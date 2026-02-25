use std::path::Path;
use std::time::SystemTime;

pub async fn extract_metadata(
    file_path: &Path,
    fallback_time: SystemTime,
) -> (Option<String>, Option<String>) {
    let path = file_path.to_path_buf();
    
    tokio::task::spawn_blocking(move || {
        extract_metadata_sync(&path, fallback_time)
    }).await.unwrap_or((None, None))
}

pub fn extract_metadata_sync(
    file_path: &Path,
    _fallback_time: SystemTime,
) -> (Option<String>, Option<String>) {
    let extension = file_path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    
    if ["jpg", "jpeg", "png", "arw", "cr2", "nef", "dng"].contains(&extension.as_str()) {
        return extract_photo_metadata_sync(file_path);
    }
    
    if ["mp4", "mov"].contains(&extension.as_str()) {
        return extract_video_metadata_sync(file_path);
    }
    
    (None, None)
}

fn extract_photo_metadata_sync(
    file_path: &Path,
) -> (Option<String>, Option<String>) {
    let file = match std::fs::File::open(file_path) {
        Ok(f) => f,
        Err(_) => return (None, None),
    };
    
    let mut bufreader = std::io::BufReader::new(&file);
    
    let exif_reader = exif::Reader::new();
    let exif = match exif_reader.read_from_container(&mut bufreader) {
        Ok(e) => e,
        Err(_) => return (None, None),
    };
    
    let capture_date = exif
        .get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
        .or_else(|| exif.get_field(exif::Tag::DateTime, exif::In::PRIMARY))
        .and_then(|f| {
            if let exif::Value::Ascii(ref vec) = f.value {
                let s = vec.iter()
                    .filter_map(|v| std::str::from_utf8(v).ok())
                    .collect::<String>();
                parse_exif_datetime(&s)
            } else {
                None
            }
        });
    
    let device_name = exif
        .get_field(exif::Tag::Model, exif::In::PRIMARY)
        .or_else(|| exif.get_field(exif::Tag::Make, exif::In::PRIMARY))
        .map(|f| f.display_value().to_string());
    
    (capture_date, device_name)
}

fn extract_video_metadata_sync(
    file_path: &Path,
) -> (Option<String>, Option<String>) {
    use std::process::Command;
    
    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("quiet")
        .arg("-print_format")
        .arg("json")
        .arg("-show_format")
        .arg(file_path)
        .output();
    
    if let Ok(output) = output {
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
            let tags = json.get("format")
                .and_then(|f| f.get("tags"));
            
            let capture_date = tags
                .and_then(|t| t.get("creation_time"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            
            let device_name = tags
                .and_then(|t| t.get("com.apple.quicktools.creator.0"))
                .or_else(|| tags.and_then(|t| t.get("com.apple.quicktools.make")))
                .or_else(|| tags.and_then(|t| t.get("com.apple.quicktools.model")))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            
            return (capture_date, device_name);
        }
    }
    
    (None, None)
}

fn parse_exif_datetime(s: &str) -> Option<String> {
    let parts: Vec<&str> = s.split(' ').collect();
    if parts.len() != 2 {
        return None;
    }
    
    let date_part = parts[0].replace(':', "-");
    // let time_part = parts[1];
    
    Some(format!("T{}:00", date_part))
}
