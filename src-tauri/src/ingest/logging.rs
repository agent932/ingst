use crate::IngestOptions;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub source_path: String,
    pub dest_path: String,
    pub size: u64,
    /// Sampled fingerprint from `fast_hash` — a lookup key, not proof of equality.
    pub hash: Option<String>,
    /// SHA-256 of the whole file, computed during the copy. Absent on entries
    /// written before this was recorded, on skips, and on same-volume moves,
    /// which rename without reading the bytes.
    #[serde(default)]
    pub full_hash: Option<String>,
    pub capture_datetime: Option<String>,
    pub device_name: String,
    pub status: String,
    pub error: Option<String>,
}

/// What the dedup index remembers about a file that has been ingested.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexEntry {
    /// A file known to carry this fingerprint: the copy in the library for
    /// persisted entries, or the source file for duplicates found within a
    /// single run, where nothing has been written yet.
    pub path: String,
    #[serde(default)]
    pub full_hash: Option<String>,
}

/// On-disk shapes the index has had. The original stored a bare
/// `hash -> dest_path` string map; reading it lets existing libraries keep
/// their dedup history instead of re-importing everything once.
#[derive(Deserialize)]
#[serde(untagged)]
enum StoredIndex {
    Current(HashMap<String, IndexEntry>),
    Legacy(HashMap<String, String>),
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
        timestamp: Utc::now().to_rfc3339(),
        source_path: source_path.to_string(),
        dest_path: dest_path.to_string(),
        size,
        hash,
        full_hash: None,
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

    // Persist hashes *before* rotating, so rotation can never discard the only
    // record of a previously-ingested file.
    if let Err(e) = update_hash_index(dest_root, entries) {
        log::warn!("Failed to update dedup index: {}", e);
    }

    rotate_logs(&log_dir);


    Ok(log_file.to_string_lossy().to_string())
}

fn index_path(dest_root: &str) -> PathBuf {
    Path::new(dest_root).join(".ingst").join("index.json")
}

fn read_index(path: &Path) -> HashMap<String, IndexEntry> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };

    match serde_json::from_str::<StoredIndex>(&content) {
        Ok(StoredIndex::Current(map)) => map,
        Ok(StoredIndex::Legacy(map)) => map
            .into_iter()
            .map(|(hash, path)| (hash, IndexEntry { path, full_hash: None }))
            .collect(),
        Err(e) => {
            log::warn!("Could not read dedup index at {:?}: {}", path, e);
            HashMap::new()
        }
    }
}

/// Merge this run's hashes into the persistent dedup index.
///
/// The index lives outside the rotating logs on purpose: `rotate_logs` keeps
/// only the newest MAX_LOGS files, so an index derived from logs alone would
/// silently forget older ingests and re-import those files as new.
pub fn update_hash_index(
    dest_root: &str,
    entries: &[LogEntry],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let path = index_path(dest_root);
    let mut index = read_index(&path);

    for entry in entries {
        if entry.status == "success" || entry.status == "skipped" {
            if let Some(hash) = &entry.hash {
                // A skip records no full hash, so keep whichever one is already
                // stored rather than blanking it and forcing later runs back
                // onto the slower confirm-by-reading-the-library path.
                let retained = index.get(hash).and_then(|e| e.full_hash.clone());
                index.insert(
                    hash.clone(),
                    IndexEntry {
                        path: entry.dest_path.clone(),
                        full_hash: entry.full_hash.clone().or(retained),
                    },
                );
            }
        }
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Write-then-rename so an interrupted write can't truncate the index.
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string(&index)?)?;
    fs::rename(&tmp, &path)?;

    log::info!("Dedup index now holds {} hashes", index.len());
    Ok(())
}

/// Keep only the most recent MAX_LOGS log files; delete the rest.
fn rotate_logs(log_dir: &Path) {
    const MAX_LOGS: usize = 50;

    let mut log_files: Vec<_> = match fs::read_dir(log_dir) {
        Ok(entries) => entries
            .flatten()
            .filter(|e| {
                e.path().extension().map(|x| x == "json").unwrap_or(false)
            })
            .collect(),
        Err(_) => return,
    };

    if log_files.len() <= MAX_LOGS {
        return;
    }

    // Sort by file name (which starts with ingst_YYYYMMDD_HHMMSS, so lexicographic = chronological).
    log_files.sort_by_key(|e| e.file_name());

    let to_delete = log_files.len() - MAX_LOGS;
    for entry in log_files.iter().take(to_delete) {
        if let Err(e) = fs::remove_file(entry.path()) {
            log::warn!("Failed to delete old log {:?}: {}", entry.path(), e);
        } else {
            log::info!("Rotated old log: {:?}", entry.path());
        }
    }
}

pub fn load_existing_hashes(dest_root: &str) -> HashMap<String, IndexEntry> {
    let mut hashes = read_index(&index_path(dest_root));
    let from_index = hashes.len();

    // Also fold in whatever the surviving logs know. This migrates libraries
    // created before the index existed, and costs nothing once it is populated.
    let log_dir = Path::new(dest_root)
        .join(".ingst")
        .join("logs");

    if log_dir.exists() {
        if let Ok(entries) = fs::read_dir(&log_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "json").unwrap_or(false) {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(log) = serde_json::from_str::<IngestLog>(&content) {
                            for entry in log.entries {
                                if entry.status == "success" || entry.status == "skipped" {
                                    if let Some(hash) = entry.hash {
                                        hashes.entry(hash).or_insert(IndexEntry {
                                            path: entry.dest_path,
                                            full_hash: entry.full_hash,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    log::info!(
        "Loaded {} existing hashes ({} from index, {} recovered from logs)",
        hashes.len(),
        from_index,
        hashes.len() - from_index
    );
    hashes
}
