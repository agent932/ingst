use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::AppHandle;

static CANCEL_FLAG: AtomicBool = AtomicBool::new(false);
static PAUSE_FLAG: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcePath {
    pub path: String,
    pub label: String,
    pub exclusions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannedFile {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub modified: String,
    pub created: Option<String>,
    pub capture_date: Option<String>,
    pub device_name: Option<String>,
    pub file_type: String,
    pub hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceScanResult {
    pub source: SourcePath,
    pub files: Vec<ScannedFile>,
    pub total_size: u64,
    pub video_count: usize,
    pub photo_count: usize,
    pub audio_count: usize,
    pub other_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DestInfo {
    pub path: String,
    pub exists: bool,
    pub writable: bool,
    pub free_space: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestOptions {
    pub operation: String,
    pub skip_duplicates: bool,
    pub dest_root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestOperation {
    pub source_path: String,
    pub dest_path: String,
    pub action: String,
    pub size: u64,
    pub capture_date: Option<String>,
    pub device_name: String,
    pub hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestPlan {
    pub operations: Vec<IngestOperation>,
    pub total_files: usize,
    pub total_size: u64,
    pub duplicate_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressEvent {
    pub current_file: String,
    pub current_index: usize,
    pub total: usize,
    pub bytes_copied: u64,
    pub total_bytes: u64,
    pub current_file_bytes: u64,
    pub current_file_total: u64,
    pub elapsed_secs: u64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestResult {
    pub success_count: usize,
    pub skipped_count: usize,
    pub error_count: usize,
    pub errors: Vec<String>,
    pub log_path: String,
    /// True when the user stopped the run before every operation was reached.
    pub cancelled: bool,
    /// Operations never started, non-zero only when `cancelled`.
    pub remaining_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub last_destination: Option<String>,
    pub default_operation: String,
    pub skip_duplicates_default: bool,
    pub theme: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            last_destination: None,
            default_operation: "copy".to_string(),
            skip_duplicates_default: true,
            theme: "system".to_string(),
        }
    }
}

#[tauri::command]
pub async fn scan_source(source: SourcePath) -> Result<SourceScanResult, String> {
    log::info!("Scanning source: {}", source.path);
    crate::ingest::scanner::scan_directory(&source).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_dest_info(path: String) -> Result<DestInfo, String> {
    log::info!("Getting destination info: {}", path);
    crate::ingest::scanner::get_destination_info(&path).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn build_ingest_plan(
    sources: Vec<SourcePath>,
    options: IngestOptions,
) -> Result<IngestPlan, String> {
    log::info!("Building ingest plan for {} sources", sources.len());
    crate::ingest::plan::build_plan(sources, &options).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn execute_ingest(
    app: AppHandle,
    plan: IngestPlan,
    options: IngestOptions,
) -> Result<IngestResult, String> {
    CANCEL_FLAG.store(false, Ordering::SeqCst);
    PAUSE_FLAG.store(false, Ordering::SeqCst);
    log::info!("Executing ingest plan with {} operations", plan.operations.len());

    let result = crate::ingest::executor::execute_plan(
        &app,
        &plan,
        &options,
        || CANCEL_FLAG.load(Ordering::SeqCst),
        || PAUSE_FLAG.load(Ordering::SeqCst),
    ).await;

    CANCEL_FLAG.store(false, Ordering::SeqCst);
    PAUSE_FLAG.store(false, Ordering::SeqCst);
    result.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cancel_ingest() -> Result<(), String> {
    log::info!("Cancel requested");
    CANCEL_FLAG.store(true, Ordering::SeqCst);
    PAUSE_FLAG.store(false, Ordering::SeqCst); // unblock any pause so the loop can exit
    Ok(())
}

#[tauri::command]
pub fn pause_ingest() -> Result<(), String> {
    log::info!("Pause requested");
    PAUSE_FLAG.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub fn resume_ingest() -> Result<(), String> {
    log::info!("Resume requested");
    PAUSE_FLAG.store(false, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub fn get_settings() -> Result<Settings, String> {
    let config_dir = dirs::config_dir()
        .ok_or("Could not find config directory")?
        .join("ingst");
    
    let settings_file = config_dir.join("settings.json");
    
    if settings_file.exists() {
        let content = std::fs::read_to_string(&settings_file)
            .map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| e.to_string())
    } else {
        Ok(Settings::default())
    }
}

#[tauri::command]
pub fn save_settings(settings: Settings) -> Result<(), String> {
    let config_dir = dirs::config_dir()
        .ok_or("Could not find config directory")?
        .join("ingst");
    
    std::fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
    
    let settings_file = config_dir.join("settings.json");
    let content = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    std::fs::write(&settings_file, content).map_err(|e| e.to_string())?;
    
    log::info!("Settings saved to {:?}", settings_file);
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountedVolume {
    pub path: String,
    pub name: String,
    pub is_removable: bool,
    pub total_space: u64,
    pub free_space: u64,
}

/// Returns a base64 `data:image/jpeg;base64,…` thumbnail, or `null` for formats
/// with no thumbnail source. See `ingest::formats::MEDIA_FORMATS` for which is
/// which.
#[tauri::command]
pub async fn get_thumbnail(path: String) -> Result<Option<String>, String> {
    tokio::task::spawn_blocking(move || generate_thumbnail(&path))
        .await
        .map_err(|e| e.to_string())?
}

fn generate_thumbnail(path: &str) -> Result<Option<String>, String> {
    use crate::ingest::formats::{self, ThumbnailSource};

    let format = formats::classify(std::path::Path::new(path));

    match format.map(|f| f.thumbnail) {
        Some(ThumbnailSource::Image) => generate_photo_thumbnail(path),
        Some(ThumbnailSource::VideoFrame) => generate_video_thumbnail(path),
        // RAW stills, audio, and non-media files get the type icon instead.
        _ => Ok(None),
    }
}

fn generate_photo_thumbnail(path: &str) -> Result<Option<String>, String> {
    use base64::{engine::general_purpose, Engine as _};
    use std::io::Cursor;

    let img = image::open(path).map_err(|e| e.to_string())?;
    let thumb = img.thumbnail(160, 160);

    let mut buf: Vec<u8> = Vec::new();
    thumb
        .write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Jpeg)
        .map_err(|e| e.to_string())?;

    let b64 = general_purpose::STANDARD.encode(&buf);
    Ok(Some(format!("data:image/jpeg;base64,{}", b64)))
}

/// Longest side of a generated thumbnail, in pixels.
const THUMBNAIL_MAX_DIM: u32 = 160;

fn generate_video_thumbnail(path: &str) -> Result<Option<String>, String> {
    use base64::{engine::general_purpose, Engine as _};

    // No frame is an ordinary outcome, not an error: the clip may be shorter
    // than the seek point, or the backend may not decode this codec. The UI
    // falls back to a file-type icon.
    let frame = crate::ingest::video::backend()
        .thumbnail(std::path::Path::new(path), THUMBNAIL_MAX_DIM);

    Ok(frame.map(|jpeg| {
        format!(
            "data:image/jpeg;base64,{}",
            general_purpose::STANDARD.encode(&jpeg)
        )
    }))
}

/// Extract the mount point from a `df -k` line.
///
/// The mount point is the 9th field but may contain spaces — "/Volumes/SD Card"
/// is an ordinary volume name. Splitting the whole line on whitespace and taking
/// field 8 truncates it to "/Volumes/SD", which then scans as an empty source.
/// Skip the first 8 fields and take the remainder of the line verbatim.
#[cfg(target_os = "macos")]
fn mount_point_from_df_line(line: &str) -> Option<&str> {
    let mut rest = line;

    for _ in 0..8 {
        let trimmed = rest.trim_start();
        let end = trimmed.find(char::is_whitespace)?;
        rest = &trimmed[end..];
    }

    let mount_point = rest.trim_start();
    if mount_point.is_empty() {
        None
    } else {
        Some(mount_point)
    }
}

#[tauri::command]
pub fn get_mounted_volumes() -> Result<Vec<MountedVolume>, String> {
    #[cfg(target_os = "macos")]
    {
        use std::path::PathBuf;
        use std::process::Command;

        let mut volumes = Vec::new();

        // Use df to get mounted volumes
        let df_output = Command::new("df")
            .arg("-k")
            .output()
            .map_err(|e| e.to_string())?;
        
        let df_str = String::from_utf8_lossy(&df_output.stdout);
        
        for line in df_str.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 9 {
                let mount_point = match mount_point_from_df_line(line) {
                    Some(mp) => mp,
                    None => continue,
                };

                // Check if it's a removable/volume path
                if mount_point.starts_with("/Volumes/") || mount_point == "/" {
                    let path = PathBuf::from(mount_point);
                    let name = path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "Root".to_string());
                    
                    // Check if removable (simplified check for /Volumes/*)
                    let is_removable = mount_point.starts_with("/Volumes/");
                    
                    // Get space info
                    let total_space: u64 = parts[1].parse().unwrap_or(0) * 1024;
                    let free_space: u64 = parts[3].parse().unwrap_or(0) * 1024;
                    
                    // Skip system volumes
                    if !name.starts_with("Macintosh HD") && !name.is_empty() {
                        volumes.push(MountedVolume {
                            path: mount_point.to_string(),
                            name,
                            is_removable,
                            total_space,
                            free_space,
                        });
                    }
                }
            }
        }
        
        // Remove duplicates based on path
        volumes.sort_by(|a, b| a.path.cmp(&b.path));
        volumes.dedup_by(|a, b| a.path == b.path);
        
        log::info!("Found {} mounted volumes", volumes.len());
        Ok(volumes)
    }
    
    #[cfg(target_os = "windows")]
    {
        // Windows implementation using drive letters
        use std::process::Command;
        
        let output = Command::new("wmic")
            .arg("logicaldisk")
            .arg("get")
            .arg("DeviceID,Size,FreeSpace,VolumeName")
            .output()
            .map_err(|e| e.to_string())?;
        
        let output_str = String::from_utf8_lossy(&output.stdout);
        let mut volumes = Vec::new();
        
        for line in output_str.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let device_id = parts[0];
                if device_id.contains(':') {
                    let free_space: u64 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
                    let total_space: u64 = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
                    let name = parts.get(3).map(|s| s.to_string()).unwrap_or_else(|| device_id.to_string());
                    
                    volumes.push(MountedVolume {
                        path: device_id.to_string(),
                        name,
                        is_removable: true, // Simplified
                        total_space,
                        free_space,
                    });
                }
            }
        }
        
        Ok(volumes)
    }
}
