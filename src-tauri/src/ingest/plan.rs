use crate::{IngestOperation, IngestOptions, IngestPlan, ScannedFile, SourcePath};
use crate::ingest::scanner;
use crate::ingest::logging;
use crate::utils::hashing;
use std::collections::HashMap;
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
    let mut all_files: Vec<ScannedFile> = Vec::new();
    
    for source in &sources {
        let result = scanner::scan_directory_sync(source)?;
        all_files.extend(result.files);
    }
    
    let dest_root = Path::new(&options.dest_root);
    
    let mut operations: Vec<IngestOperation> = Vec::new();
    let mut seen_hashes: HashMap<String, String> = HashMap::new();
    let mut duplicate_count = 0;
    
    // Load existing hashes from previous ingest logs for true idempotency
    if options.skip_duplicates {
        seen_hashes = logging::load_existing_hashes(&options.dest_root);
    }
    
    for file in &all_files {
        let capture_date = file.capture_date.clone()
            .or_else(|| Some(file.modified.clone()));
        
        let device_name = file.device_name.clone()
            .or_else(|| Some(scanner::get_source_label(&file.path)))
            .unwrap_or_else(|| "UnknownDevice".to_string());
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

        let dest_path = resolve_collision(&dest_dir, &file.name);
        
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
                    let sidecar_dest = resolve_collision(dest_dir, &sidecar_name);
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

pub fn resolve_collision(dir: &Path, filename: &str) -> std::path::PathBuf {
    let dest_path = dir.join(filename);
    
    if !dest_path.exists() {
        return dest_path;
    }
    
    let stem = Path::new(filename)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    
    let ext = Path::new(filename)
        .extension()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_default();
    
    let mut counter = 1;
    loop {
        let new_name = if ext.is_empty() {
            format!("{}_{}", stem, counter)
        } else {
            format!("{}_{}.{}", stem, counter, ext)
        };
        
        let new_path = dir.join(&new_name);
        if !new_path.exists() {
            return new_path;
        }
        
        counter += 1;
        if counter > 9999 {
            break;
        }
    }
    
    dir.join(filename)
}
