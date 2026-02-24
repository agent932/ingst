use crate::{IngestOptions, IngestPlan, IngestResult, ProgressEvent};
use crate::ingest::logging::create_log_entry;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::{AppHandle, Emitter};

pub async fn execute_plan<F>(
    app: &AppHandle,
    plan: &IngestPlan,
    options: &IngestOptions,
    is_cancelled: F,
) -> Result<IngestResult, Box<dyn std::error::Error + Send + Sync>>
where
    F: Fn() -> bool + Send + Sync,
{
    let start_time = Instant::now();
    let mut success_count = 0;
    let mut skipped_count = 0;
    let mut error_count = 0;
    let mut errors: Vec<String> = Vec::new();
    let mut bytes_copied: u64 = 0;
    
    let log_entries = Arc::new(Mutex::new(Vec::new()));
    
    let total = plan.operations.len();
    let total_bytes = plan.total_size;
    
    for (index, operation) in plan.operations.iter().enumerate() {
        if is_cancelled() {
            log::info!("Ingest cancelled by user");
            break;
        }
        
        let progress = ProgressEvent {
            current_file: operation.source_path.clone(),
            current_index: index + 1,
            total,
            bytes_copied,
            total_bytes,
            elapsed_secs: start_time.elapsed().as_secs(),
            status: "processing".to_string(),
        };
        
        app.emit("ingest-progress", &progress).ok();
        
        let result = match operation.action.as_str() {
            "skip" => {
                skipped_count += 1;
                log_entries.lock().unwrap().push(create_log_entry(
                    &operation.source_path,
                    &operation.dest_path,
                    operation.size,
                    operation.hash.clone(),
                    operation.capture_date.clone(),
                    operation.device_name.clone(),
                    "skipped",
                    None,
                ));
                Ok(())
            }
            "copy" => copy_file(&operation.source_path, &operation.dest_path),
            "move" => move_file(&operation.source_path, &operation.dest_path),
            _ => Ok(()),
        };
        
        match result {
            Ok(()) => {
                success_count += 1;
                bytes_copied += operation.size;
                
                log_entries.lock().unwrap().push(create_log_entry(
                    &operation.source_path,
                    &operation.dest_path,
                    operation.size,
                    operation.hash.clone(),
                    operation.capture_date.clone(),
                    operation.device_name.clone(),
                    "success",
                    None,
                ));
            }
            Err(e) => {
                error_count += 1;
                let error_msg = format!("{}: {}", operation.source_path, e);
                errors.push(error_msg.clone());
                log::error!("Error processing file: {}", error_msg);
                
                log_entries.lock().unwrap().push(create_log_entry(
                    &operation.source_path,
                    &operation.dest_path,
                    operation.size,
                    operation.hash.clone(),
                    operation.capture_date.clone(),
                    operation.device_name.clone(),
                    "error",
                    Some(e.to_string()),
                ));
            }
        }
    }
    
    let log_path = crate::ingest::logging::save_log(
        &options.dest_root,
        &plan.operations.iter().map(|o| o.source_path.clone()).collect::<Vec<_>>(),
        options,
        &log_entries.lock().unwrap(),
    )?;
    
    let final_progress = ProgressEvent {
        current_file: "Complete".to_string(),
        current_index: total,
        total,
        bytes_copied,
        total_bytes,
        elapsed_secs: start_time.elapsed().as_secs(),
        status: "complete".to_string(),
    };
    
    app.emit("ingest-progress", &final_progress).ok();
    
    log::info!(
        "Ingest complete: {} success, {} skipped, {} errors",
        success_count,
        skipped_count,
        error_count
    );
    
    Ok(IngestResult {
        success_count,
        skipped_count,
        error_count,
        errors,
        log_path,
    })
}

fn copy_file(source: &str, dest: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let source_path = Path::new(source);
    let dest_path = Path::new(dest);
    
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    
    std::fs::copy(source_path, dest_path)?;
    
    Ok(())
}

fn move_file(source: &str, dest: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let source_path = Path::new(source);
    let dest_path = Path::new(dest);
    
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    
    std::fs::rename(source_path, dest_path).or_else(|_| {
        copy_file(source, dest)?;
        std::fs::remove_file(source_path)?;
        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    })?;
    
    Ok(())
}
