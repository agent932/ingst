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

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(name);
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn entry(status: &str, hash: Option<&str>, dest: &str) -> LogEntry {
        LogEntry {
            timestamp: "2026-07-23T07:30:20+00:00".to_string(),
            source_path: format!("/Volumes/CARD/{}", dest),
            dest_path: format!("/library/2026/07/Card/{}", dest),
            size: 1024,
            hash: hash.map(|h| h.to_string()),
            full_hash: None,
            capture_datetime: Some("2026-07-23T07:30:20".to_string()),
            device_name: "Card".to_string(),
            status: status.to_string(),
            error: None,
        }
    }

    /// The index is the only memory the app has of what it already imported.
    /// Recording a *failed* copy would make the next run skip that file as a
    /// duplicate — the footage never lands in the library, and the user
    /// reformats the card believing it did.
    #[test]
    fn hash_index_records_only_files_that_are_really_in_the_library() {
        let root = tmpdir("ingst_log_index_status");
        let dest_root = root.to_string_lossy().to_string();

        let entries = vec![
            entry("success", Some("hash-copied"), "a.mp4"),
            entry("skipped", Some("hash-already-there"), "b.mp4"),
            entry("error", Some("hash-failed"), "c.mp4"),
            entry("success", None, "d.mp4"),
        ];

        update_hash_index(&dest_root, &entries).unwrap();
        let loaded = load_existing_hashes(&dest_root);

        assert!(loaded.contains_key("hash-copied"), "a copied file must be remembered");
        assert!(
            loaded.contains_key("hash-already-there"),
            "a file skipped as an existing duplicate is still in the library"
        );
        assert!(
            !loaded.contains_key("hash-failed"),
            "a failed copy must NOT be remembered, or the retry gets skipped"
        );
        assert_eq!(loaded.len(), 2);

        assert!(
            !root.join(".ingst").join("index.json.tmp").exists(),
            "the write-then-rename temp file must not survive"
        );

        fs::remove_dir_all(&root).ok();
    }

    /// Each ingest must add to the index, not replace it. If run two wiped the
    /// memory of run one, every earlier file would be re-imported on the next
    /// pass and the library would fill with duplicates.
    #[test]
    fn hash_index_merges_across_runs() {
        let root = tmpdir("ingst_log_index_merge");
        let dest_root = root.to_string_lossy().to_string();

        update_hash_index(&dest_root, &[entry("success", Some("run-one"), "a.mp4")]).unwrap();
        update_hash_index(&dest_root, &[entry("success", Some("run-two"), "b.mp4")]).unwrap();

        let loaded = load_existing_hashes(&dest_root);
        assert!(loaded.contains_key("run-one"), "the earlier run was forgotten");
        assert!(loaded.contains_key("run-two"));

        fs::remove_dir_all(&root).ok();
    }

    /// Libraries created before the index existed only have logs. Those hashes
    /// must still be recovered, otherwise the first run after an upgrade
    /// re-copies the entire library.
    #[test]
    fn hashes_are_recovered_from_logs_when_the_index_is_missing() {
        let root = tmpdir("ingst_log_recover");
        let dest_root = root.to_string_lossy().to_string();
        let log_dir = root.join(".ingst").join("logs");
        fs::create_dir_all(&log_dir).unwrap();

        let log = IngestLog {
            app_version: "0.0.1".to_string(),
            timestamp: "2026-07-23T07:30:20+00:00".to_string(),
            sources: vec!["/Volumes/CARD".to_string()],
            options: IngestOptionsLog {
                operation: "copy".to_string(),
                skip_duplicates: true,
                dest_root: dest_root.clone(),
            },
            entries: vec![
                entry("success", Some("only-in-the-log"), "old.mp4"),
                entry("error", Some("failed-in-the-log"), "bad.mp4"),
            ],
        };
        fs::write(
            log_dir.join("ingst_20260101_000000.json"),
            serde_json::to_string(&log).unwrap(),
        )
        .unwrap();

        assert!(
            !root.join(".ingst").join("index.json").exists(),
            "precondition: no index yet"
        );

        let loaded = load_existing_hashes(&dest_root);
        assert!(
            loaded.contains_key("only-in-the-log"),
            "pre-index ingests must still be recognised as already imported"
        );
        assert!(
            !loaded.contains_key("failed-in-the-log"),
            "a failed entry in an old log is not proof the file is in the library"
        );

        fs::remove_dir_all(&root).ok();
    }

    /// A hash present in both places must keep the index's value: the index
    /// stores where the file landed in the library, the log fallback only the
    /// card path it came from, and the card is gone by the next run.
    #[test]
    fn the_index_wins_over_a_log_for_the_same_hash() {
        let root = tmpdir("ingst_log_index_precedence");
        let dest_root = root.to_string_lossy().to_string();
        let log_dir = root.join(".ingst").join("logs");
        fs::create_dir_all(&log_dir).unwrap();

        let shared = entry("success", Some("same-hash"), "clip.mp4");
        update_hash_index(&dest_root, std::slice::from_ref(&shared)).unwrap();

        let log = IngestLog {
            app_version: "0.0.1".to_string(),
            timestamp: shared.timestamp.clone(),
            sources: vec![],
            options: IngestOptionsLog {
                operation: "copy".to_string(),
                skip_duplicates: true,
                dest_root: dest_root.clone(),
            },
            entries: vec![shared.clone()],
        };
        fs::write(
            log_dir.join("ingst_20260101_000000.json"),
            serde_json::to_string(&log).unwrap(),
        )
        .unwrap();

        let loaded = load_existing_hashes(&dest_root);
        assert_eq!(
            loaded.get("same-hash").map(|e| e.path.as_str()),
            Some(shared.dest_path.as_str()),
            "the library location must not be overwritten by the source path"
        );

        fs::remove_dir_all(&root).ok();
    }

    /// Rotation must delete the *oldest* logs. Deleting the newest would throw
    /// away the record of the ingest that just happened — the only audit trail
    /// of where each file went, and the dedup fallback for pre-index libraries.
    #[test]
    fn rotate_logs_keeps_the_newest_fifty() {
        const MAX_LOGS: usize = 50; // mirrors the constant inside rotate_logs
        let log_dir = tmpdir("ingst_log_rotate");

        // 55 logs, named the way save_log names them (lexicographic = chronological).
        for i in 1..=55 {
            fs::write(
                log_dir.join(format!("ingst_20260101_{:06}.json", i)),
                "{}",
            )
            .unwrap();
        }
        fs::write(log_dir.join("notes.txt"), "not a log").unwrap();

        rotate_logs(&log_dir);

        let mut remaining: Vec<String> = fs::read_dir(&log_dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.ends_with(".json"))
            .collect();
        remaining.sort();

        assert_eq!(remaining.len(), MAX_LOGS, "should keep exactly MAX_LOGS logs");
        assert_eq!(
            remaining.first().map(String::as_str),
            Some("ingst_20260101_000006.json"),
            "the five oldest must be the ones deleted"
        );
        assert_eq!(
            remaining.last().map(String::as_str),
            Some("ingst_20260101_000055.json"),
            "the most recent log must always survive"
        );
        assert!(
            log_dir.join("notes.txt").exists(),
            "rotation must not touch files that are not logs"
        );

        fs::remove_dir_all(&log_dir).ok();
    }

    /// At or below the limit nothing may be deleted — an over-eager rotation
    /// would discard history the user still needs.
    #[test]
    fn rotate_logs_is_a_no_op_at_the_limit() {
        let log_dir = tmpdir("ingst_log_rotate_limit");
        for i in 1..=50 {
            fs::write(log_dir.join(format!("ingst_20260101_{:06}.json", i)), "{}").unwrap();
        }

        rotate_logs(&log_dir);

        assert_eq!(fs::read_dir(&log_dir).unwrap().count(), 50);

        fs::remove_dir_all(&log_dir).ok();
    }

    /// End to end: a saved log must be readable back (it is the audit trail the
    /// user is told to keep) and its hashes must already be in the index, since
    /// rotation may delete this very log later.
    #[test]
    fn save_log_writes_a_readable_log_and_updates_the_index() {
        let root = tmpdir("ingst_log_save");
        let dest_root = root.to_string_lossy().to_string();
        let options = IngestOptions {
            operation: "copy".to_string(),
            skip_duplicates: true,
            dest_root: dest_root.clone(),
        };
        let entries = vec![entry("success", Some("saved-hash"), "a.mp4")];

        let path = save_log(&dest_root, &["/Volumes/CARD".to_string()], &options, &entries).unwrap();

        let round_tripped: IngestLog =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(round_tripped.entries.len(), 1);
        assert_eq!(round_tripped.entries[0].hash.as_deref(), Some("saved-hash"));
        assert_eq!(round_tripped.sources, vec!["/Volumes/CARD".to_string()]);

        let index: HashMap<String, IndexEntry> = serde_json::from_str(
            &fs::read_to_string(root.join(".ingst").join("index.json")).unwrap(),
        )
        .unwrap();
        assert!(
            index.contains_key("saved-hash"),
            "hashes must reach the index even if this log is rotated away"
        );

        fs::remove_dir_all(&root).ok();
    }
}
