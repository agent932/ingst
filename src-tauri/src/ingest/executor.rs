use crate::{IngestOptions, IngestPlan, IngestResult, ProgressEvent};
use crate::ingest::logging::create_log_entry;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::{AppHandle, Emitter};
use std::sync::mpsc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

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
    let errors: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let mut bytes_copied: u64 = 0;
    
    let log_entries: Arc<Mutex<Vec<crate::ingest::logging::LogEntry>>> = Arc::new(Mutex::new(Vec::new()));
    
    let total = plan.operations.len();
    let total_bytes = plan.total_size;
    
    // Atomic for tracking current file progress
    let current_file_bytes: Arc<AtomicU64> = Arc::new(AtomicU64::new(0));
    let should_emit_progress: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    
    // Create channel for progress updates
    let (progress_tx, progress_rx) = mpsc::channel::<ProgressEvent>();
    
    // Spawn a task to receive progress updates and emit them
    let app_for_receiver = app.clone();
    let _receiver_handle = tokio::spawn(async move {
        while let Ok(progress) = progress_rx.recv() {
            app_for_receiver.emit("ingest-progress", &progress).ok();
        }
    });
    
    // Emit initial progress
    let _ = progress_tx.send(ProgressEvent {
        current_file: "Starting...".to_string(),
        current_index: 0,
        total,
        bytes_copied: 0,
        total_bytes,
        current_file_bytes: 0,
        current_file_total: 0,
        elapsed_secs: 0,
        status: "starting".to_string(),
    });
    
    for (index, operation) in plan.operations.iter().enumerate() {
        if is_cancelled() {
            log::info!("Ingest cancelled by user");
            break;
        }
        
        // Reset current file bytes
        current_file_bytes.store(0, Ordering::SeqCst);
        should_emit_progress.store(false, Ordering::SeqCst);
        
        // Emit progress at start of each file
        let _ = progress_tx.send(ProgressEvent {
            current_file: operation.source_path.clone(),
            current_index: index + 1,
            total,
            bytes_copied,
            total_bytes,
            current_file_bytes: 0,
            current_file_total: operation.size,
            elapsed_secs: start_time.elapsed().as_secs(),
            status: "processing".to_string(),
        });
        
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
            "copy" => {
                let source = operation.source_path.clone();
                let dest = operation.dest_path.clone();
                let bytes_tracker = current_file_bytes.clone();
                let emit_flag = should_emit_progress.clone();
                let file_size = operation.size;
                
                tokio::task::spawn_blocking(move || {
                    copy_file_with_progress(&source, &dest, bytes_tracker, emit_flag)
                }).await.map_err(|e| e.to_string())?
            }
            "move" => {
                let source = operation.source_path.clone();
                let dest = operation.dest_path.clone();
                let bytes_tracker = current_file_bytes.clone();
                let emit_flag = should_emit_progress.clone();
                
                tokio::task::spawn_blocking(move || {
                    move_file_with_progress(&source, &dest, bytes_tracker, emit_flag)
                }).await.map_err(|e| e.to_string())?
            }
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
                errors.lock().unwrap().push(error_msg.clone());
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
        
        // Emit progress after each file completes
        let elapsed = start_time.elapsed().as_secs();
        let _ = progress_tx.send(ProgressEvent {
            current_file: operation.source_path.clone(),
            current_index: index + 1,
            total,
            bytes_copied,
            total_bytes,
            current_file_bytes: operation.size,
            current_file_total: operation.size,
            elapsed_secs: elapsed,
            status: "processing".to_string(),
        });
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
        current_file_bytes: 0,
        current_file_total: 0,
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
    
    let final_errors = errors.lock().unwrap().clone();
    
    Ok(IngestResult {
        success_count,
        skipped_count,
        error_count,
        errors: final_errors,
        log_path,
    })
}

fn copy_file_with_progress(
    source: &str, 
    dest: &str, 
    bytes_tracker: Arc<AtomicU64>,
    _emit_flag: Arc<AtomicBool>
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let source_path = Path::new(source);
    let dest_path = Path::new(dest);
    
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    
    let total_size = source_path.metadata()?.len();
    let mut bytes_written: u64 = 0;
    let chunk_size: u64 = 64 * 1024; // 64KB chunks
    
    let mut source_file = std::fs::File::open(source_path)?;
    let mut dest_file = std::fs::File::create(dest_path)?;
    
    let mut buffer = vec![0u8; chunk_size as usize];
    
    loop {
        let bytes_read = std::io::Read::read(&mut source_file, &mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        
        std::io::Write::write_all(&mut dest_file, &buffer[..bytes_read])?;
        bytes_written += bytes_read as u64;
        
        // Track bytes for current file
        bytes_tracker.store(bytes_written, Ordering::SeqCst);
    }
    
    Ok(())
}

fn move_file_with_progress(
    source: &str, 
    dest: &str, 
    bytes_tracker: Arc<AtomicU64>,
    _emit_flag: Arc<AtomicBool>
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let source_path = Path::new(source);
    let dest_path = Path::new(dest);
    
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    
    let total_size = source_path.metadata().map(|m| m.len()).unwrap_or(0);
    
    // Try rename first (fast)
    if std::fs::rename(source_path, dest_path).is_ok() {
        bytes_tracker.store(total_size, Ordering::SeqCst);
        return Ok(());
    }
    
    // Fall back to copy + delete with progress
    copy_file_with_progress(source, dest, bytes_tracker, _emit_flag)?;
    std::fs::remove_file(source_path)?;
    
    Ok(())
}
