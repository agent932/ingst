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
