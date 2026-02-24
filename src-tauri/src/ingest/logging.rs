use crate::IngestOptions;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub source_path: String,
    pub dest_path: String,
    pub size: u64,
    pub hash: Option<String>,
    pub capture_datetime: Option<String>,
    pub device_name: String,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestLog {
    pub app_version: String,
    pub timestamp: String,
    pub sources: Vec<String>,
    pub options: IngestOptionsLog,
    pub entries: Vec<LogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestOptionsLog {
    pub operation: String,
    pub skip_duplicates: bool,
    pub dest_root: String,
}

pub fn create_log_entry(
    source_path: &str,
    dest_path: &str,
    size: u64,
    hash: Option<String>,
    capture_datetime: Option<String>,
    device_name: String,
    status: &str,
    error: Option<String>,
) -> LogEntry {
    LogEntry {
        source_path: source_path.to_string(),
        dest_path: dest_path.to_string(),
        size,
        hash,
        capture_datetime,
        device_name,
        status: status.to_string(),
        error,
    }
}

pub fn save_log(
    dest_root: &str,
    sources: &[String],
    options: &IngestOptions,
    entries: &[LogEntry],
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
    
    let log_dir = Path::new(dest_root)
        .join(".ingst")
        .join("logs");
    
    fs::create_dir_all(&log_dir)?;
    
    let log_file = log_dir.join(format!("ingst_{}.json", timestamp));
    
    let log = IngestLog {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: Utc::now().to_rfc3339(),
        sources: sources.to_vec(),
        options: IngestOptionsLog {
            operation: options.operation.clone(),
            skip_duplicates: options.skip_duplicates,
            dest_root: options.dest_root.clone(),
        },
        entries: entries.to_vec(),
    };
    
    let json = serde_json::to_string_pretty(&log)?;
    fs::write(&log_file, json)?;
    
    log::info!("Log saved to {:?}", log_file);
    
    Ok(log_file.to_string_lossy().to_string())
}

pub fn load_existing_hashes(dest_root: &str) -> HashMap<String, String> {
    let mut hashes = HashMap::new();
    
    let log_dir = Path::new(dest_root)
        .join(".ingst")
        .join("logs");
    
    if !log_dir.exists() {
        return hashes;
    }
    
    if let Ok(entries) = fs::read_dir(&log_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(log) = serde_json::from_str::<IngestLog>(&content) {
                        for entry in log.entries {
                            if entry.status == "success" || entry.status == "skipped" {
                                if let Some(hash) = entry.hash {
                                    // Use hash as key, store source path
                                    hashes.insert(hash, entry.source_path);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    log::info!("Loaded {} existing hashes from logs", hashes.len());
    hashes
}
