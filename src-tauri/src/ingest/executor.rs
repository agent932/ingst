use crate::{IngestOptions, IngestPlan, IngestResult, ProgressEvent};
use crate::ingest::logging::create_log_entry;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use std::sync::mpsc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

/// Maximum number of files copied concurrently.
const MAX_PARALLEL: usize = 4;

pub async fn execute_plan<F, P>(
    app: &AppHandle,
    plan: &IngestPlan,
    options: &IngestOptions,
    is_cancelled: F,
    is_paused: P,
) -> Result<IngestResult, Box<dyn std::error::Error + Send + Sync>>
where
    F: Fn() -> bool + Send + Sync,
    P: Fn() -> bool + Send + Sync,
{
    let start_time = Instant::now();

    // Shared atomic counters updated by each task.
    let success_count  = Arc::new(AtomicUsize::new(0));
    let skipped_count  = Arc::new(AtomicUsize::new(0));
    let error_count    = Arc::new(AtomicUsize::new(0));
    let bytes_copied   = Arc::new(AtomicU64::new(0));
    let completed_count = Arc::new(AtomicUsize::new(0));

    let errors: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let log_entries: Arc<Mutex<Vec<crate::ingest::logging::LogEntry>>> =
        Arc::new(Mutex::new(Vec::new()));

    let total       = plan.operations.len();
    let total_bytes = plan.total_size;

    let semaphore = Arc::new(Semaphore::new(MAX_PARALLEL));

    // Progress channel — receiver runs on its own task.
    let (progress_tx, progress_rx) = mpsc::channel::<ProgressEvent>();

    let app_for_receiver = app.clone();
    let _receiver_handle = tokio::spawn(async move {
        while let Ok(progress) = progress_rx.recv() {
            app_for_receiver.emit("ingest-progress", &progress).ok();
        }
    });

    // Emit initial progress.
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

    let mut join_set: JoinSet<()> = JoinSet::new();

    'outer: for operation in plan.operations.iter() {
        // Acquire a semaphore slot with periodic cancel/pause checks.
        let permit = loop {
            if is_cancelled() {
                break 'outer;
            }
            while is_paused() && !is_cancelled() {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            if is_cancelled() {
                break 'outer;
            }

            match tokio::time::timeout(
                Duration::from_millis(100),
                semaphore.clone().acquire_owned(),
            )
            .await
            {
                Ok(Ok(permit)) => break permit,
                Ok(Err(_))    => break 'outer, // semaphore closed (shouldn't happen)
                Err(_)        => continue,      // timeout — re-check cancel/pause
            }
        };

        // Clone everything the task needs.
        let op               = operation.clone();
        let bytes_copied_c   = bytes_copied.clone();
        let success_count_c  = success_count.clone();
        let skipped_count_c  = skipped_count.clone();
        let error_count_c    = error_count.clone();
        let completed_count_c = completed_count.clone();
        let log_entries_c    = log_entries.clone();
        let errors_c         = errors.clone();
        let progress_tx_c    = progress_tx.clone();

        join_set.spawn(async move {
            let _permit = permit; // released when this scope exits

            // Emit start-of-file progress.
            let done_so_far = completed_count_c.load(Ordering::SeqCst);
            let _ = progress_tx_c.send(ProgressEvent {
                current_file:       op.source_path.clone(),
                current_index:      done_so_far,
                total,
                bytes_copied:       bytes_copied_c.load(Ordering::SeqCst),
                total_bytes,
                current_file_bytes: 0,
                current_file_total: 0,
                elapsed_secs:       start_time.elapsed().as_secs(),
                status:             "processing".to_string(),
            });

            // Ok(true)  → bytes were actually transferred
            // Ok(false) → intentionally skipped, nothing written
            let result: Result<bool, Box<dyn std::error::Error + Send + Sync>> =
                match op.action.as_str() {
                    "skip" => Ok(false),
                    "copy" => {
                        let source  = op.source_path.clone();
                        let dest    = op.dest_path.clone();
                        let tracker = Arc::new(AtomicU64::new(0));
                        let flag    = Arc::new(AtomicBool::new(false));
                        let source2 = source.clone();
                        let dest2   = dest.clone();

                        match tokio::task::spawn_blocking(move || {
                            copy_file_with_progress(&source, &dest, tracker, flag)?;
                            verify_copy(&source2, &dest2)
                        })
                        .await
                        {
                            Ok(inner) => inner.map(|()| true),
                            Err(e)    => Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
                        }
                    }
                    "move" => {
                        let source  = op.source_path.clone();
                        let dest    = op.dest_path.clone();
                        let tracker = Arc::new(AtomicU64::new(0));
                        let flag    = Arc::new(AtomicBool::new(false));

                        match tokio::task::spawn_blocking(move || {
                            move_file_with_progress(&source, &dest, tracker, flag)
                        })
                        .await
                        {
                            Ok(inner) => inner.map(|()| true),
                            Err(e)    => Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
                        }
                    }
                    _ => Ok(false),
                };

            match result {
                Ok(true) => {
                    success_count_c.fetch_add(1, Ordering::SeqCst);
                    bytes_copied_c.fetch_add(op.size, Ordering::SeqCst);
                    log_entries_c
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push(create_log_entry(
                            &op.source_path,
                            &op.dest_path,
                            op.size,
                            op.hash.clone(),
                            op.capture_date.clone(),
                            op.device_name.clone(),
                            "success",
                            None,
                        ));
                }
                Ok(false) => {
                    skipped_count_c.fetch_add(1, Ordering::SeqCst);
                    log_entries_c
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push(create_log_entry(
                            &op.source_path,
                            &op.dest_path,
                            op.size,
                            op.hash.clone(),
                            op.capture_date.clone(),
                            op.device_name.clone(),
                            "skipped",
                            None,
                        ));
                }
                Err(e) => {
                    error_count_c.fetch_add(1, Ordering::SeqCst);
                    let msg = format!("{}: {}", op.source_path, e);
                    errors_c
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push(msg.clone());
                    log::error!("Error processing file: {}", msg);
                    log_entries_c
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push(create_log_entry(
                            &op.source_path,
                            &op.dest_path,
                            op.size,
                            op.hash.clone(),
                            op.capture_date.clone(),
                            op.device_name.clone(),
                            "error",
                            Some(e.to_string()),
                        ));
                }
            }

            // Emit completion progress for this file.
            let done = completed_count_c.fetch_add(1, Ordering::SeqCst) + 1;
            let _ = progress_tx_c.send(ProgressEvent {
                current_file:       op.source_path.clone(),
                current_index:      done,
                total,
                bytes_copied:       bytes_copied_c.load(Ordering::SeqCst),
                total_bytes,
                current_file_bytes: 0,
                current_file_total: 0,
                elapsed_secs:       start_time.elapsed().as_secs(),
                status:             "processing".to_string(),
            });
        });
    }

    // Wait for all in-flight tasks to finish.
    while join_set.join_next().await.is_some() {}

    let log_path = crate::ingest::logging::save_log(
        &options.dest_root,
        &plan
            .operations
            .iter()
            .map(|o| o.source_path.clone())
            .collect::<Vec<_>>(),
        options,
        &log_entries.lock().unwrap_or_else(|e| e.into_inner()),
    )?;

    let final_bytes = bytes_copied.load(Ordering::SeqCst);

    app.emit(
        "ingest-progress",
        &ProgressEvent {
            current_file:       "Complete".to_string(),
            current_index:      total,
            total,
            bytes_copied:       final_bytes,
            total_bytes,
            current_file_bytes: 0,
            current_file_total: 0,
            elapsed_secs:       start_time.elapsed().as_secs(),
            status:             "complete".to_string(),
        },
    )
    .ok();

    let success = success_count.load(Ordering::SeqCst);
    let skipped = skipped_count.load(Ordering::SeqCst);
    let errors_n = error_count.load(Ordering::SeqCst);
    let final_errors = errors.lock().unwrap_or_else(|e| e.into_inner()).clone();

    log::info!(
        "Ingest complete: {} success, {} skipped, {} errors",
        success,
        skipped,
        errors_n
    );

    Ok(IngestResult {
        success_count: success,
        skipped_count: skipped,
        error_count:   errors_n,
        errors:        final_errors,
        log_path,
    })
}

/// Verify a copy by comparing SHA256 of source and destination.
/// Removes the destination file if hashes don't match.
fn verify_copy(source: &str, dest: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let src_hash = crate::utils::hashing::full_hash(source)?;
    let dst_hash = crate::utils::hashing::full_hash(dest)?;
    if src_hash != dst_hash {
        let _ = std::fs::remove_file(dest);
        return Err(format!(
            "Checksum mismatch after copy — source: {} dest: {}",
            src_hash, dst_hash
        )
        .into());
    }
    Ok(())
}

fn copy_file_with_progress(
    source: &str,
    dest: &str,
    bytes_tracker: Arc<AtomicU64>,
    _emit_flag: Arc<AtomicBool>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let source_path = Path::new(source);
    let dest_path   = Path::new(dest);

    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut bytes_written: u64 = 0;
    let chunk_size: usize = 4 * 1024 * 1024; // 4 MB

    let mut source_file = std::fs::File::open(source_path)?;
    let mut dest_file   = std::fs::File::create(dest_path)?;

    let mut buffer = vec![0u8; chunk_size];

    loop {
        let bytes_read = std::io::Read::read(&mut source_file, &mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        std::io::Write::write_all(&mut dest_file, &buffer[..bytes_read])?;
        bytes_written += bytes_read as u64;
        bytes_tracker.store(bytes_written, Ordering::SeqCst);
    }

    Ok(())
}

fn move_file_with_progress(
    source: &str,
    dest: &str,
    bytes_tracker: Arc<AtomicU64>,
    emit_flag: Arc<AtomicBool>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let source_path = Path::new(source);
    let dest_path   = Path::new(dest);

    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Try rename first (fast, same volume).
    if std::fs::rename(source_path, dest_path).is_ok() {
        let size = dest_path.metadata().map(|m| m.len()).unwrap_or(0);
        bytes_tracker.store(size, Ordering::SeqCst);
        return Ok(());
    }

    // Fall back to copy + delete with progress.
    copy_file_with_progress(source, dest, bytes_tracker, emit_flag)?;
    // Verify before deleting the original — if corrupt, source is preserved.
    verify_copy(source, dest)?;
    std::fs::remove_file(source_path)?;

    Ok(())
}
