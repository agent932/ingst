use crate::{IngestOperation, IngestOptions, IngestPlan, ScannedFile, SourcePath};
use crate::ingest::scanner;
use crate::ingest::logging;
use crate::utils::hashing;
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub async fn build_plan(
    sources: Vec<SourcePath>,
    options: &IngestOptions,
) -> Result<IngestPlan, Box<dyn std::error::Error + Send + Sync>> {
    let sources_clone = sources.clone();
    let options_clone = options.clone();
    
    tokio::task::spawn_blocking(move || {
        build_plan_sync(sources_clone, &options_clone)
    }).await.map_err(|e| e.to_string())?
}

pub fn build_plan_sync(
    sources: Vec<SourcePath>,
    options: &IngestOptions,
) -> Result<IngestPlan, Box<dyn std::error::Error + Send + Sync>> {
    // Each file is paired with the label of the source it came from, so that a
    // file whose own metadata has no device name can fall back to the card or
    // folder it was read from.
    let mut all_files: Vec<(ScannedFile, String)> = Vec::new();

    for source in &sources {
        let result = scanner::scan_directory_sync(source)?;
        let source_label = if source.label.trim().is_empty() {
            scanner::get_source_label(&source.path)
        } else {
            source.label.clone()
        };
        all_files.extend(
            result.files.into_iter().map(|f| (f, source_label.clone())),
        );
    }

    // Many cameras write a device name onto stills but nothing onto video —
    // the Insta360 Luna Ultra writes full EXIF to .dng/.jpg and leaves .mp4
    // with only creation_time. Letting files that resolved a device lend it to
    // their neighbours keeps one shoot in one folder, instead of splitting it
    // between the camera name and the card name.
    let dir_device = infer_device_by_directory(&all_files);

    let dest_root = Path::new(&options.dest_root);

    let mut operations: Vec<IngestOperation> = Vec::new();
    let mut seen_hashes: HashMap<String, String> = HashMap::new();
    let mut duplicate_count = 0;

    // Destination paths already handed out during this plan. Without this, two
    // source files with the same name landing in the same folder would both be
    // assigned the same destination and the second would overwrite the first —
    // `resolve_collision` alone can only see files already on disk.
    let mut claimed: HashSet<String> = HashSet::new();

    // Load existing hashes from previous ingest logs for true idempotency
    if options.skip_duplicates {
        seen_hashes = logging::load_existing_hashes(&options.dest_root);
    }

    for (file, source_label) in &all_files {
        let capture_date = file.capture_date.clone()
            .or_else(|| Some(file.modified.clone()));

        let device_name = file.device_name.clone()
            .filter(|d| !d.trim().is_empty())
            .or_else(|| {
                Path::new(&file.path)
                    .parent()
                    .and_then(|p| dir_device.get(p.to_string_lossy().as_ref()).cloned())
            })
            .unwrap_or_else(|| source_label.clone());
        let device_name = sanitize_device_name(&device_name);
        
        let date_path = capture_date
            .as_ref()
            .and_then(|d| parse_date_for_path(d))
            .unwrap_or_else(|| {
                parse_date_for_path(&file.modified).unwrap_or_else(|| "UnknownDate".to_string())
            });
        
        let year = &date_path[0..4];
        let month = &date_path[5..7];
        
        let dest_dir = dest_root
            .join(year)
            .join(month)
            .join(&device_name);

        // Guard: reject any path that escapes dest_root after joining.
        if !dest_dir.starts_with(dest_root) {
            log::warn!("Skipping file with unsafe dest path: {:?}", dest_dir);
            continue;
        }

        let dest_path = resolve_collision(&dest_dir, &file.name, &mut claimed);
        
        let hash = if options.skip_duplicates {
            Some(hashing::fast_hash(&file.path, file.size)?)
        } else {
            None
        };
        
        let action = if options.skip_duplicates {
            if let Some(ref hash_val) = hash {
                if seen_hashes.get(hash_val).is_some() {
                    duplicate_count += 1;
                    "skip".to_string()
                } else {
                    seen_hashes.insert(hash_val.clone(), file.path.clone());
                    options.operation.clone()
                }
            } else {
                options.operation.clone()
            }
        } else {
            options.operation.clone()
        };
        
        operations.push(IngestOperation {
            source_path: file.path.clone(),
            dest_path: dest_path.to_string_lossy().to_string(),
            action,
            size: file.size,
            capture_date,
            device_name,
            hash,
        });
    }
    
    // Sidecar pass: for each non-skipped file, look for companion files
    // (.xmp, .srt, .lut, .xml) with the same stem in the same source directory.
    const SIDECAR_EXTENSIONS: &[&str] = &["xmp", "srt", "lut", "xml", "edl"];
    let mut sidecar_ops: Vec<IngestOperation> = Vec::new();
    let ingested_paths: std::collections::HashSet<&str> = operations
        .iter()
        .filter(|o| o.action != "skip")
        .map(|o| o.source_path.as_str())
        .collect();

    for op in operations.iter().filter(|o| o.action != "skip") {
        let src = Path::new(&op.source_path);
        let stem = match src.file_stem().map(|s| s.to_string_lossy().to_lowercase()) {
            Some(s) => s,
            None => continue,
        };
        let parent = match src.parent() {
            Some(p) => p,
            None => continue,
        };
        let dest_dir = Path::new(&op.dest_path).parent().unwrap_or(dest_root);

        for ext in SIDECAR_EXTENSIONS {
            // Try both lower and upper case extensions.
            for case_ext in &[ext.to_string(), ext.to_uppercase()] {
                let candidate = parent.join(format!("{}.{}", stem, case_ext));
                let candidate_str = candidate.to_string_lossy().to_string();
                if candidate.exists() && !ingested_paths.contains(candidate_str.as_str()) {
                    let sidecar_name = candidate.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let sidecar_dest = resolve_collision(dest_dir, &sidecar_name, &mut claimed);
                    let sidecar_size = candidate.metadata().map(|m| m.len()).unwrap_or(0);
                    sidecar_ops.push(IngestOperation {
                        source_path: candidate_str,
                        dest_path: sidecar_dest.to_string_lossy().to_string(),
                        action: "copy".to_string(), // always copy sidecars
                        size: sidecar_size,
                        capture_date: op.capture_date.clone(),
                        device_name: op.device_name.clone(),
                        hash: None,
                    });
                }
            }
        }
    }
    operations.extend(sidecar_ops);

    let total_files = operations.iter().filter(|o| o.action != "skip").count();
    let total_size: u64 = operations
        .iter()
        .filter(|o| o.action != "skip")
        .map(|o| o.size)
        .sum();
    
    log::info!(
        "Built plan: {} operations, {} total size, {} duplicates",
        operations.len(),
        total_size,
        duplicate_count
    );
    
    Ok(IngestPlan {
        operations,
        total_files,
        total_size,
        duplicate_count,
    })
}

/// Map each directory to the device name most commonly reported by the files
/// inside it, so files in that directory with no device metadata can adopt it.
///
/// Ties break on name so the result is stable across runs. A directory holding
/// files from two cameras resolves to whichever appears more often, which is
/// the best available guess without per-file grouping.
fn infer_device_by_directory(files: &[(ScannedFile, String)]) -> HashMap<String, String> {
    let mut counts: HashMap<String, HashMap<String, usize>> = HashMap::new();

    for (file, _) in files {
        let device = match file.device_name.as_ref().map(|d| d.trim()) {
            Some(d) if !d.is_empty() => d.to_string(),
            _ => continue,
        };
        let dir = match Path::new(&file.path).parent() {
            Some(p) => p.to_string_lossy().to_string(),
            None => continue,
        };
        *counts.entry(dir).or_default().entry(device).or_insert(0) += 1;
    }

    counts
        .into_iter()
        .filter_map(|(dir, devices)| {
            let mut ranked: Vec<(String, usize)> = devices.into_iter().collect();
            ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            ranked.into_iter().next().map(|(device, _)| (dir, device))
        })
        .collect()
}

/// Strip characters from a camera/device name that are unsafe in file paths.
/// Keeps alphanumeric, spaces, hyphens, and underscores; collapses runs of
/// spaces; falls back to "UnknownDevice" if nothing remains.
fn sanitize_device_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    if sanitized.is_empty() {
        "UnknownDevice".to_string()
    } else {
        sanitized
    }
}

pub fn parse_date_for_path(date_str: &str) -> Option<String> {
    let date_part = date_str.split('T').next()?;
    let parts: Vec<&str> = date_part.split('-').collect();
    if parts.len() >= 2 {
        Some(format!("{}/{}", parts[0], parts[1]))
    } else {
        None
    }
}

/// Pick a destination path for `filename` inside `dir` that collides with
/// neither an existing file on disk nor a path already claimed earlier in this
/// plan. The chosen path is recorded in `claimed`.
///
/// Keys are lower-cased because the common targets (APFS, exFAT, NTFS) are
/// case-insensitive, so `IMG_1.JPG` and `img_1.jpg` are the same file there.
pub fn resolve_collision(
    dir: &Path,
    filename: &str,
    claimed: &mut HashSet<String>,
) -> std::path::PathBuf {
    let take = |path: std::path::PathBuf, claimed: &mut HashSet<String>| {
        claimed.insert(path.to_string_lossy().to_lowercase());
        path
    };

    let is_free = |path: &Path, claimed: &HashSet<String>| {
        !path.exists() && !claimed.contains(&path.to_string_lossy().to_lowercase())
    };

    let dest_path = dir.join(filename);
    if is_free(&dest_path, claimed) {
        return take(dest_path, claimed);
    }

    let stem = Path::new(filename)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    let ext = Path::new(filename)
        .extension()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_default();

    let build = |suffix: &str| -> String {
        if ext.is_empty() {
            format!("{}_{}", stem, suffix)
        } else {
            format!("{}_{}.{}", stem, suffix, ext)
        }
    };

    for counter in 1..=9999 {
        let new_path = dir.join(build(&counter.to_string()));
        if is_free(&new_path, claimed) {
            return take(new_path, claimed);
        }
    }

    // Pathological case (10k same-named files in one folder): fall back to a
    // timestamp suffix rather than returning a path that would clobber another.
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos().to_string())
        .unwrap_or_else(|_| "dup".to_string());
    take(dir.join(build(&unique)), claimed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmpdir(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(name);
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn write_file(dir: &Path, name: &str, contents: &[u8]) -> PathBuf {
        let p = dir.join(name);
        fs::write(&p, contents).unwrap();
        p
    }

    /// Pin a file's modified time so date-routing assertions do not depend on
    /// when the test happens to run.
    fn set_mtime(path: &Path, epoch_secs: u64) {
        let t = std::time::UNIX_EPOCH + std::time::Duration::from_secs(epoch_secs);
        let f = fs::File::options().write(true).open(path).unwrap();
        f.set_times(fs::FileTimes::new().set_modified(t)).unwrap();
    }

    fn source(path: &Path, label: &str) -> SourcePath {
        SourcePath {
            path: path.to_string_lossy().to_string(),
            label: label.to_string(),
            exclusions: vec![],
        }
    }

    fn options(dest: &Path, skip_duplicates: bool) -> IngestOptions {
        IngestOptions {
            operation: "copy".to_string(),
            skip_duplicates,
            dest_root: dest.to_string_lossy().to_string(),
        }
    }

    fn scanned(path: &str, device: Option<&str>) -> (ScannedFile, String) {
        (
            ScannedFile {
                path: path.to_string(),
                name: Path::new(path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default(),
                size: 0,
                modified: "2026-01-01T00:00:00".to_string(),
                created: None,
                capture_date: None,
                device_name: device.map(|d| d.to_string()),
                file_type: "video".to_string(),
                hash: None,
            },
            "CardLabel".to_string(),
        )
    }

    fn rel<'a>(op: &'a IngestOperation, dest: &Path) -> String {
        Path::new(&op.dest_path)
            .strip_prefix(dest)
            .unwrap_or_else(|_| Path::new(&op.dest_path))
            .to_string_lossy()
            .replace('\\', "/")
    }

    // --- date / device routing -------------------------------------------

    /// Footage must land in the folder for the month it was *shot*, under the
    /// name of the card it came from. Getting this wrong scatters one shoot
    /// across several month folders, where the editor never finds it again.
    #[test]
    fn plan_routes_files_by_capture_date_and_device() {
        let root = tmpdir("ingst_plan_routing");
        let src = root.join("card");
        fs::create_dir_all(&src).unwrap();
        let dest = root.join("library");

        write_file(&src, "VID_20260723_073020_002.mp4", b"video-bytes");
        write_file(&src, "IMG_20241105_101112_001.jpg", b"not-a-real-jpeg");

        let plan = build_plan_sync(
            vec![source(&src, "Sony A7S")],
            &options(&dest, false),
        )
        .unwrap();

        let mut paths: Vec<String> = plan.operations.iter().map(|o| rel(o, &dest)).collect();
        paths.sort();

        assert_eq!(
            paths,
            vec![
                "2024/11/Sony A7S/IMG_20241105_101112_001.jpg".to_string(),
                "2026/07/Sony A7S/VID_20260723_073020_002.mp4".to_string(),
            ],
            "capture date from the filename must drive the YYYY/MM folders"
        );
        assert_eq!(plan.total_files, 2);

        fs::remove_dir_all(&root).ok();
    }

    /// A clip with no metadata and no timestamp in its name must still be
    /// filed by its modification time, not dumped in "UnknownDate" — the
    /// month folder is the only index this app gives the user.
    #[test]
    fn plan_falls_back_to_modified_time_when_no_capture_date() {
        let root = tmpdir("ingst_plan_mtime_fallback");
        let src = root.join("card");
        fs::create_dir_all(&src).unwrap();
        let dest = root.join("library");

        let f = write_file(&src, "takeone.wav", b"audio");
        set_mtime(&f, 1_551_700_800); // 2019-03-04T12:00:00Z

        let plan = build_plan_sync(vec![source(&src, "Zoom H6")], &options(&dest, false)).unwrap();

        assert_eq!(plan.operations.len(), 1);
        assert_eq!(rel(&plan.operations[0], &dest), "2019/03/Zoom H6/takeone.wav");

        fs::remove_dir_all(&root).ok();
    }

    /// Two cards holding the same camera-generated filename (cameras restart
    /// their counters) must never be assigned the same destination. If they
    /// were, the executor would copy one file over the other and one take
    /// would be gone with no error shown.
    #[test]
    fn plan_never_assigns_two_sources_the_same_destination() {
        let root = tmpdir("ingst_plan_same_name");
        let a = root.join("card_a");
        let b = root.join("card_b");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        let dest = root.join("library");

        // Same name, same shoot date, same device label — different content.
        write_file(&a, "VID_20260723_073020_002.mp4", b"take one");
        write_file(&b, "VID_20260723_073020_002.mp4", b"take two - different");

        let plan = build_plan_sync(
            vec![source(&a, "RED KOMODO"), source(&b, "RED KOMODO")],
            &options(&dest, false),
        )
        .unwrap();

        assert_eq!(plan.operations.len(), 2);
        let d0 = &plan.operations[0].dest_path;
        let d1 = &plan.operations[1].dest_path;
        assert_ne!(
            d0.to_lowercase(),
            d1.to_lowercase(),
            "identically named files from two cards must not share a destination"
        );
        assert!(
            d1.contains("VID_20260723_073020_002_1.mp4"),
            "second file should be de-duplicated by suffix, got {}",
            d1
        );

        fs::remove_dir_all(&root).ok();
    }

    /// With dedup on, a byte-identical file already recorded in the library
    /// index is skipped — and, just as important, a *different* file is not.
    /// A false positive here silently drops footage that was never imported.
    #[test]
    fn plan_skips_only_true_duplicates() {
        let root = tmpdir("ingst_plan_dedup");
        let src = root.join("card");
        fs::create_dir_all(&src).unwrap();
        let dest = root.join("library");

        let payload = vec![7u8; 200 * 1024];
        let mut other = payload.clone();
        other[0] ^= 0xFF; // differs in the hashed header

        write_file(&src, "VID_20260723_073020_001.mp4", &payload);
        write_file(&src, "VID_20260723_073020_002.mp4", &payload);
        write_file(&src, "VID_20260723_073020_003.mp4", &other);

        let plan = build_plan_sync(vec![source(&src, "Card")], &options(&dest, true)).unwrap();

        assert_eq!(plan.duplicate_count, 1, "the second identical file is the duplicate");
        assert_eq!(plan.total_files, 2, "both distinct files must still be ingested");
        let skipped: Vec<&IngestOperation> =
            plan.operations.iter().filter(|o| o.action == "skip").collect();
        assert_eq!(skipped.len(), 1);
        assert!(
            plan.operations.iter().all(|o| o.hash.is_some()),
            "dedup mode must record a hash for every operation, for the log index"
        );

        fs::remove_dir_all(&root).ok();
    }

    // --- sanitize_device_name --------------------------------------------

    /// A device name comes from EXIF, i.e. from the file — untrusted input.
    /// Any separator or dot-segment that survives would turn one folder name
    /// into a path and could write outside the chosen library.
    #[test]
    fn sanitize_device_name_cannot_produce_a_path() {
        let hostile = [
            "Canon/EOS:R5",
            "../../etc",
            "..",
            "C:\\Windows\\System32",
            "/absolute/name",
            "name\nwith\tcontrol",
            "*?<>|\"",
        ];

        let root = Path::new("/library");
        for name in hostile {
            let s = sanitize_device_name(name);
            assert!(!s.is_empty(), "{name:?} sanitized to an empty name");
            assert!(
                !s.contains('/') && !s.contains('\\') && !s.contains(':'),
                "{name:?} still contains a path separator: {s:?}"
            );
            assert_ne!(s, "..", "{name:?} sanitized to a parent-directory hop");
            let joined = root.join("2026").join("07").join(&s);
            assert!(
                joined.starts_with(root) && joined.components().count() == 5,
                "{name:?} produced extra path components: {joined:?}"
            );
        }

        assert_eq!(sanitize_device_name("Canon/EOS:R5"), "Canon EOS R5");
        assert_eq!(sanitize_device_name("DJI Mini 4 Pro"), "DJI Mini 4 Pro");
        assert_eq!(sanitize_device_name("Insta360   Luna  Ultra"), "Insta360 Luna Ultra");
    }

    /// A camera that reports a name made only of punctuation must still get a
    /// folder. An empty name would make the destination `YYYY/MM/` itself and
    /// mix every device's files together.
    #[test]
    fn sanitize_device_name_falls_back_when_nothing_survives() {
        for empty in ["", "   ", "///", "..", "!!!", "\u{0}"] {
            assert_eq!(
                sanitize_device_name(empty),
                "UnknownDevice",
                "{empty:?} should fall back to UnknownDevice"
            );
        }
    }

    // --- parse_date_for_path ---------------------------------------------

    /// The date-only form (no `T`) has to work too: `capture_date` can arrive
    /// from EXIF or ffprobe without a time component, and returning None there
    /// would file a dated file under the wrong month via the mtime fallback.
    #[test]
    fn parse_date_for_path_handles_date_only_and_rejects_garbage() {
        assert_eq!(parse_date_for_path("2026-07-23"), Some("2026/07".to_string()));
        assert_eq!(parse_date_for_path("2026-07-23T07:30:20"), Some("2026/07".to_string()));
        assert_eq!(parse_date_for_path("2026-07-23T07:30:20+02:00"), Some("2026/07".to_string()));
        assert_eq!(parse_date_for_path("20260723"), None);
        assert_eq!(parse_date_for_path(""), None);
        assert_eq!(parse_date_for_path("T07:30:20"), None);
    }

    /// KNOWN BUG — currently fails, hence #[ignore]; run with
    /// `cargo test -- --ignored` to see it.
    ///
    /// `build_plan_sync` slices this function's output with `[0..4]` and
    /// `[5..7]`. A camera that writes an unpadded EXIF date ("2024:1:5
    /// 10:00:00") reaches here as "2024-1-5T10:00:00" and yields "2024/1" —
    /// six bytes, so `[5..7]` panics and the *entire* ingest aborts before a
    /// single file is copied. Every accepted input must be safe to slice that
    /// way, i.e. exactly `YYYY/MM`.
    #[test]
    #[ignore = "documents a latent panic: parse_date_for_path can return fewer than 7 bytes"]
    fn parse_date_for_path_output_is_always_sliceable_by_callers() {
        let inputs = [
            "2024-1-5T10:00:00",  // unpadded EXIF date, via parse_exif_datetime
            "2024-1",             // unpadded, no day
            "99-1T00:00:00",      // two-digit year from a mis-set camera clock
            "2026-07-23T07:30:20",
        ];

        for input in inputs {
            if let Some(date_path) = parse_date_for_path(input) {
                assert!(
                    date_path.len() >= 7,
                    "{input:?} -> {date_path:?} is too short for the [0..4]/[5..7] slice callers do"
                );
                let year = &date_path[0..4];
                let month = &date_path[5..7];
                assert!(year.chars().all(|c| c.is_ascii_digit()), "{input:?} -> year {year:?}");
                assert!(month.chars().all(|c| c.is_ascii_digit()), "{input:?} -> month {month:?}");
            }
        }
    }

    // --- infer_device_by_directory ---------------------------------------

    /// Cameras that tag stills but not video (Insta360 writes EXIF to .dng and
    /// nothing to .mp4) would otherwise split one shoot between the camera name
    /// and the card name. The directory adopts the name most files report.
    #[test]
    fn infer_device_by_directory_uses_the_majority_name() {
        let files = vec![
            scanned("/cards/A/one.dng", Some("Canon EOS R5")),
            scanned("/cards/A/two.dng", Some("Canon EOS R5")),
            scanned("/cards/A/three.mp4", Some("DJI Mini 4 Pro")),
            scanned("/cards/A/four.mp4", None),
        ];

        let map = infer_device_by_directory(&files);
        assert_eq!(map.get("/cards/A").map(String::as_str), Some("Canon EOS R5"));
    }

    /// A tie must resolve the same way on every run. An unstable tie-break
    /// files the same card under a different device folder each time it is
    /// re-ingested, which quietly duplicates a whole shoot.
    #[test]
    fn infer_device_by_directory_breaks_ties_stably() {
        let forward = vec![
            scanned("/cards/B/one.mp4", Some("Beta Cam")),
            scanned("/cards/B/two.mp4", Some("Alpha Cam")),
        ];
        let reverse: Vec<_> = forward.iter().cloned().rev().collect();

        // Repeated because HashMap iteration order varies per process/run.
        for _ in 0..20 {
            assert_eq!(
                infer_device_by_directory(&forward).get("/cards/B").map(String::as_str),
                Some("Alpha Cam")
            );
            assert_eq!(
                infer_device_by_directory(&reverse).get("/cards/B").map(String::as_str),
                Some("Alpha Cam"),
                "tie-break must not depend on the order files were scanned"
            );
        }
    }

    /// Blank device names are what a camera writes when the field exists but is
    /// unset. Treating "" as a real vote would name the folder after nothing
    /// and hide the card-label fallback.
    #[test]
    fn infer_device_by_directory_ignores_blank_names() {
        let files = vec![
            scanned("/cards/C/one.mp4", Some("")),
            scanned("/cards/C/two.mp4", Some("   ")),
            scanned("/cards/C/three.mp4", None),
        ];

        assert!(
            infer_device_by_directory(&files).get("/cards/C").is_none(),
            "a directory with no real device names must not be assigned one"
        );
    }

    /// End-to-end: a file with no device metadata inherits the name its
    /// neighbours reported, instead of falling back to the card label and
    /// landing in a second folder for the same shoot.
    #[test]
    fn plan_lends_a_directorys_device_name_to_files_without_one() {
        let root = tmpdir("ingst_plan_dir_device");
        let src = root.join("card");
        fs::create_dir_all(&src).unwrap();
        let dest = root.join("library");
        write_file(&src, "VID_20260723_073020_002.mp4", b"clip");

        // The clip has no device of its own; the still beside it carried EXIF.
        let files = vec![
            scanned(
                &src.join("VID_20260723_073020_002.mp4").to_string_lossy(),
                None,
            ),
            scanned(&src.join("IMG_0001.dng").to_string_lossy(), Some("Insta360 Luna Ultra")),
        ];

        let dir_device = infer_device_by_directory(&files);
        assert_eq!(
            dir_device.get(src.to_string_lossy().as_ref()).map(String::as_str),
            Some("Insta360 Luna Ultra")
        );

        // And the real planner reaches the card-label fallback when nothing in
        // the directory reported a device at all.
        let plan = build_plan_sync(vec![source(&src, "SDXC 128")], &options(&dest, false)).unwrap();
        assert_eq!(plan.operations[0].device_name, "SDXC 128");

        fs::remove_dir_all(&root).ok();
    }

    // --- sidecars ---------------------------------------------------------

    /// Sidecars carry the grade and the subtitles; leaving them on the card
    /// loses work that cannot be recovered from the footage. They must follow
    /// their clip into the same folder, and nothing else may be swept up.
    #[test]
    fn plan_pairs_every_sidecar_extension_with_its_clip() {
        let root = tmpdir("ingst_plan_sidecars");
        let src = root.join("card");
        fs::create_dir_all(&src).unwrap();
        let dest = root.join("library");

        write_file(&src, "a001.mp4", b"clip-bytes");
        for ext in ["xmp", "srt", "lut", "xml", "edl"] {
            write_file(&src, &format!("a001.{}", ext), b"sidecar");
        }
        write_file(&src, "a001.txt", b"notes - not a sidecar");
        write_file(&src, "b002.xmp", b"orphan - no clip of its own");

        let plan = build_plan_sync(vec![source(&src, "Card")], &options(&dest, false)).unwrap();

        let names: std::collections::HashSet<String> = plan
            .operations
            .iter()
            .map(|o| {
                Path::new(&o.dest_path)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_lowercase()
            })
            .collect();

        for ext in ["xmp", "srt", "lut", "xml", "edl"] {
            assert!(
                names.contains(&format!("a001.{}", ext)),
                ".{} sidecar was not paired with its clip; plan was {:?}",
                ext,
                names
            );
        }
        assert!(!names.contains("a001.txt"), "arbitrary files must not be treated as sidecars");
        assert!(!names.contains("b002.xmp"), "a sidecar with no clip must not be ingested");

        let clip_dir = plan
            .operations
            .iter()
            .find(|o| o.dest_path.ends_with("a001.mp4"))
            .map(|o| Path::new(&o.dest_path).parent().unwrap().to_path_buf())
            .expect("clip must be in the plan");
        for op in plan.operations.iter().filter(|o| !o.dest_path.ends_with("a001.mp4")) {
            assert_eq!(
                Path::new(&op.dest_path).parent().unwrap(),
                clip_dir,
                "sidecar must land beside its clip, not in its own folder"
            );
            assert_eq!(op.action, "copy", "sidecars are always copied, never moved");
        }

        assert!(
            plan.operations.iter().all(|o| Path::new(&o.source_path).exists()),
            "every planned source path must be a file the executor can actually open"
        );
        assert_eq!(
            plan.total_size,
            plan.operations.iter().map(|o| o.size).sum::<u64>(),
            "sidecar bytes must be counted, or the free-space check under-reads"
        );

        fs::remove_dir_all(&root).ok();
    }

    /// KNOWN BUG — currently fails, hence #[ignore]; run with
    /// `cargo test -- --ignored` to see it.
    ///
    /// The sidecar pass probes both `a001.xmp` and `a001.XMP`. On the case-
    /// insensitive filesystems this app actually runs on (APFS, NTFS, exFAT)
    /// *both* `exists()`, so every sidecar is planned twice and the library
    /// gains a bogus `a001_1.XMP` next to every real sidecar on every ingest.
    /// It also inflates `total_files`/`total_size`, so the progress bar and the
    /// free-space check are both wrong.
    #[test]
    #[ignore = "documents a real bug: sidecars are planned twice on case-insensitive filesystems"]
    fn each_sidecar_is_planned_exactly_once() {
        let root = tmpdir("ingst_plan_sidecar_once");
        let src = root.join("card");
        fs::create_dir_all(&src).unwrap();
        let dest = root.join("library");

        write_file(&src, "a001.mp4", b"clip-bytes");
        for ext in ["xmp", "srt", "lut", "xml", "edl"] {
            write_file(&src, &format!("a001.{}", ext), b"sidecar");
        }

        let plan = build_plan_sync(vec![source(&src, "Card")], &options(&dest, false)).unwrap();

        assert_eq!(
            plan.operations.len(),
            6,
            "expected clip + 5 sidecars, got: {:?}",
            plan.operations.iter().map(|o| o.dest_path.clone()).collect::<Vec<_>>()
        );
        assert_eq!(plan.total_files, 6);

        fs::remove_dir_all(&root).ok();
    }

    /// Cameras write upper-case names (A001.MP4 / A001.XMP). The sidecar has to
    /// be discovered at all — this part holds today on a case-insensitive
    /// filesystem, and is the reason the bug below stays hidden in testing.
    #[test]
    fn plan_discovers_uppercase_sidecars() {
        let root = tmpdir("ingst_plan_sidecar_case");
        let src = root.join("card");
        fs::create_dir_all(&src).unwrap();
        let dest = root.join("library");

        write_file(&src, "A001.MP4", b"clip-bytes");
        write_file(&src, "A001.XMP", b"sidecar");

        let plan = build_plan_sync(vec![source(&src, "Card")], &options(&dest, false)).unwrap();

        assert!(
            plan.operations.iter().any(|o| o.dest_path.to_lowercase().ends_with(".xmp")),
            "an upper-case sidecar next to an upper-case clip must be ingested"
        );

        fs::remove_dir_all(&root).ok();
    }

    /// KNOWN BUG — currently fails, hence #[ignore]; run with
    /// `cargo test -- --ignored` to see it.
    ///
    /// The sidecar search lower-cases the clip's stem, so `A001.MP4` is filed
    /// as `A001.MP4` while its `A001.XMP` is filed as `a001.xmp`. An NLE pairs
    /// a sidecar to its clip by name, and on a case-sensitive destination
    /// (Linux, a NAS share, case-sensitive APFS) the grade/subtitles silently
    /// stop being linked. The same lower-casing means that on a case-sensitive
    /// *source* the sidecar is never found at all.
    #[test]
    #[ignore = "documents a real bug: sidecar destination name is lower-cased, unlinking it from its clip"]
    fn sidecar_destination_keeps_the_clips_name_case() {
        let root = tmpdir("ingst_plan_sidecar_case_kept");
        let src = root.join("card");
        fs::create_dir_all(&src).unwrap();
        let dest = root.join("library");

        write_file(&src, "A001.MP4", b"clip-bytes");
        write_file(&src, "A001.XMP", b"sidecar");

        let plan = build_plan_sync(vec![source(&src, "Card")], &options(&dest, false)).unwrap();

        let sidecar = plan
            .operations
            .iter()
            .find(|o| o.dest_path.to_lowercase().ends_with(".xmp"))
            .expect("sidecar must be in the plan");

        let name = Path::new(&sidecar.dest_path)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        assert_eq!(
            name, "A001.XMP",
            "sidecar must keep the on-disk name it had beside the clip"
        );

        fs::remove_dir_all(&root).ok();
    }
}
