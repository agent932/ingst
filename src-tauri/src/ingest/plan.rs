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
    let mut all_files: Vec<ScannedFile> = Vec::new();
    
    for source in &sources {
        let result = scanner::scan_directory(source).await?;
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
            .or_else(|| Some(source_label_from_path(&file.path, &sources)))
            .unwrap_or_else(|| "UnknownDevice".to_string());
        
let date_path = capture_date
        .as_ref()
        .and_then(|d| parse_date_for_path(d))
        .unwrap_or_else(|| {
            let fallback = &file.modified;
            parse_date_for_path(fallback).unwrap_or_else(|| "UnknownDate".to_string())
        });
        
        let year = &date_path[0..4];
        let month = &date_path[5..7];
        
        let dest_dir = dest_root
            .join(year)
            .join(month)
            .join(&device_name);
        
        let dest_path = resolve_collision(&dest_dir, &file.name);
        
        let hash = if options.skip_duplicates {
            Some(hashing::fast_hash(&file.path, file.size)?)
        } else {
            None
        };
        
        let action = if options.skip_duplicates {
            if let Some(ref hash_val) = hash {
                if let Some(existing) = seen_hashes.get(hash_val) {
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

fn source_label_from_path(path: &str, sources: &[SourcePath]) -> String {
    for source in sources {
        if path.starts_with(&source.path) {
            return source.label.clone();
        }
    }
    "UnknownDevice".to_string()
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
