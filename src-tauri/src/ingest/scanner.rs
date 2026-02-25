use crate::{ScannedFile, SourcePath, SourceScanResult};
use chrono::{DateTime, Utc};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::SystemTime;
use walkdir::WalkDir;

const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mov", "mxf", "avi", "mkv"];
const AUDIO_EXTENSIONS: &[&str] = &["wav", "mp3", "aac"];
const PHOTO_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "braw", "r3d", "arw", "cr2", "nef", "dng"];

pub async fn scan_directory(source: &SourcePath) -> Result<SourceScanResult, Box<dyn std::error::Error + Send + Sync>> {
    let source = source.clone();
    
    tokio::task::spawn_blocking(move || {
        scan_directory_sync(&source)
    }).await.map_err(|e| e.to_string())?
}

pub fn scan_directory_sync(source: &SourcePath) -> Result<SourceScanResult, Box<dyn std::error::Error + Send + Sync>> {
    let path = Path::new(&source.path);
    
    let mut files: Vec<ScannedFile> = Vec::new();
    let mut video_count = 0;
    let mut photo_count = 0;
    let mut audio_count = 0;
    let mut other_count = 0;
    let mut total_size: u64 = 0;
    
    let exclusions: Vec<String> = source.exclusions.iter()
        .map(|e| e.to_lowercase())
        .collect();
    
    for entry in WalkDir::new(path)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_dir() {
            let dir_name = entry.file_name().to_string_lossy().to_lowercase();
            if exclusions.iter().any(|e| dir_name.contains(e)) {
                continue;
            }
            continue;
        }
        
        let file_path = entry.path();
        
        if !file_path.is_file() {
            continue;
        }
        
        let extension = file_path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        
        let file_type = if VIDEO_EXTENSIONS.contains(&extension.as_str()) {
            video_count += 1;
            "video"
        } else if PHOTO_EXTENSIONS.contains(&extension.as_str()) {
            photo_count += 1;
            "photo"
        } else if AUDIO_EXTENSIONS.contains(&extension.as_str()) {
            audio_count += 1;
            "audio"
        } else {
            other_count += 1;
            continue;
        };
        
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let size = metadata.len();
        total_size += size;
        
        let modified = metadata
            .modified()
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let modified_str = format_datetime(modified);
        
        let created = metadata
            .created()
            .ok()
            .map(format_datetime);
        
        let (capture_date, device_name) = crate::ingest::metadata::extract_metadata_sync(
            file_path,
            modified,
        );
        
        let file_name = file_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        
        files.push(ScannedFile {
            path: file_path.to_string_lossy().to_string(),
            name: file_name,
            size,
            modified: modified_str,
            created,
            capture_date,
            device_name,
            file_type: file_type.to_string(),
            hash: None,
        });
    }
    
    log::info!("Scanned {} files from {}", files.len(), source.path);
    
    Ok(SourceScanResult {
        source: source.clone(),
        files,
        total_size,
        video_count,
        photo_count,
        audio_count,
        other_count,
    })
}

pub async fn get_destination_info(path: &str) -> Result<crate::DestInfo, Box<dyn std::error::Error + Send + Sync>> {
    let path = path.to_string();
    
    tokio::task::spawn_blocking(move || {
        get_destination_info_sync(&path)
    }).await.map_err(|e| e.to_string())?
}

fn get_destination_info_sync(path: &str) -> Result<crate::DestInfo, Box<dyn std::error::Error + Send + Sync>> {
    let path = Path::new(path);
    let exists = path.exists();
    
    let writable = if exists {
        std::fs::metadata(path)
            .map(|m| m.permissions().readonly())
            .unwrap_or(true) == false
    } else {
        if let Some(parent) = path.parent() {
            parent.exists() && std::fs::metadata(parent)
                .map(|m| m.permissions().readonly())
                .unwrap_or(true) == false
        } else {
            false
        }
    };
    
    let free_space = get_free_space(path).unwrap_or(0);
    
    Ok(crate::DestInfo {
        path: path.to_string_lossy().to_string(),
        exists,
        writable,
        free_space,
    })
}

fn get_free_space(path: &Path) -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let output = Command::new("df")
            .arg("-k")
            .arg(path)
            .output()
            .ok()?;
        
        let output_str = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = output_str.lines().collect();
        
        if lines.len() >= 2 {
            let parts: Vec<&str> = lines[1].split_whitespace().collect();
            if parts.len() >= 4 {
                return parts[3].parse::<u64>().ok().map(|v| v * 1024);
            }
        }
        None
    }
    
    #[cfg(target_os = "windows")]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        
        let path_wide: Vec<u16> = OsStr::new(path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        
        let mut free_bytes: u64 = 0;
        let mut total_bytes: u64 = 0;
        let mut total_free_bytes: u64 = 0;
        
        unsafe {
            #[link(name = "kernel32")]
            extern "system" {
                fn GetDiskFreeSpaceExW(
                    lpDirectoryName: *const u16,
                    lpFreeBytesAvailableToCaller: *mut u64,
                    lpTotalNumberOfBytes: *mut u64,
                    lpTotalNumberOfFreeBytes: *mut u64,
                ) -> i32;
            }
            
            if GetDiskFreeSpaceExW(
                path_wide.as_ptr(),
                &mut free_bytes,
                &mut total_bytes,
                &mut total_free_bytes,
            ) != 0 {
                return Some(free_bytes);
            }
        }
        None
    }
    
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

fn format_datetime(time: SystemTime) -> String {
    let datetime: DateTime<Utc> = time.into();
    datetime.format("%Y-%m-%dT%H:%M:%S").to_string()
}

pub fn get_source_label(path: &str) -> String {
    let path = Path::new(path);
    
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let output = Command::new("diskutil")
            .arg("info")
            .arg("-plist")
            .arg(path)
            .output();
        
        if let Ok(output) = output {
            if let Ok(plist) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                if let Some(name) = plist.get("VolumeName").and_then(|v| v.as_str()) {
                    return name.to_string();
                }
            }
        }
    }
    
    #[cfg(target_os = "windows")]
    {
        let name = path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if !name.is_empty() {
            return name;
        }
    }
    
    path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "UnknownSource".to_string())
}
