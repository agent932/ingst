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
    
    if ["mp4", "mov", "mxf", "avi", "mkv"].contains(&extension.as_str()) {
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
    
    let make = exif
        .get_field(exif::Tag::Make, exif::In::PRIMARY)
        .map(|f| clean_exif_string(&f.display_value().to_string()))
        .filter(|s| !s.is_empty());

    let model = exif
        .get_field(exif::Tag::Model, exif::In::PRIMARY)
        .map(|f| clean_exif_string(&f.display_value().to_string()))
        .filter(|s| !s.is_empty());

    (capture_date, combine_make_model(make, model))
}

fn clean_exif_string(s: &str) -> String {
    s.trim().trim_matches('"').trim().to_string()
}

/// Build a folder-friendly device name from EXIF Make and Model.
///
/// Some makers repeat the brand in Model ("Canon" / "Canon EOS R5"), others do
/// not ("Insta360" / "Luna Ultra"). Prefixing only when the brand is absent
/// avoids both "Canon Canon EOS R5" and a bare, ambiguous "Luna Ultra".
fn combine_make_model(make: Option<String>, model: Option<String>) -> Option<String> {
    match (make, model) {
        (Some(make), Some(model)) => {
            if model.to_lowercase().contains(&make.to_lowercase()) {
                Some(model)
            } else {
                Some(format!("{} {}", make, model))
            }
        }
        (Some(make), None) => Some(make),
        (None, Some(model)) => Some(model),
        (None, None) => None,
    }
}

fn extract_video_metadata_sync(
    file_path: &Path,
) -> (Option<String>, Option<String>) {
    use std::process::Command;

    let output = Command::new(crate::utils::paths::ffprobe_path())
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
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(normalize_to_local);

            // QuickTime metadata keys are `com.apple.quicktime.*`. Cameras that
            // write plain MP4 tags (DJI, GoPro, Android) use the bare keys.
            let device_name = tags
                .and_then(|t| {
                    t.get("com.apple.quicktime.model")
                        .or_else(|| t.get("com.apple.quicktime.make"))
                        .or_else(|| t.get("model"))
                        .or_else(|| t.get("make"))
                        .or_else(|| t.get("device_model"))
                })
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());

            return (capture_date, device_name);
        }
    }
    
    (None, None)
}

/// Convert a timezone-qualified timestamp to local wall-clock time.
///
/// Container metadata (`creation_time` from ffprobe) is UTC; EXIF
/// `DateTimeOriginal` carries no zone and is already local. Left unconverted, a
/// clip and a still shot in the same minute can land in different YYYY/MM
/// folders — an evening shoot on the 31st puts video in the following month.
/// Timestamps without a zone are returned unchanged.
fn normalize_to_local(ts: &str) -> String {
    use chrono::{DateTime, Local};

    match DateTime::parse_from_rfc3339(ts) {
        Ok(dt) => dt
            .with_timezone(&Local)
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string(),
        Err(_) => ts.to_string(),
    }
}

/// Recover a capture time from camera filename conventions, e.g.
/// `VID_20260723_073020_002.mp4` or `IMG_20260723_072938_001.jpg`.
///
/// Used only when a file carries no usable metadata. These stamps are local
/// wall-clock time, which is what belongs in the folder structure.
pub fn timestamp_from_filename(file_name: &str) -> Option<String> {
    let chars: Vec<char> = file_name.chars().collect();
    let n = chars.len();

    for i in 0..n {
        // Must start at a digit-run boundary so serial numbers do not match.
        if i > 0 && chars[i - 1].is_ascii_digit() {
            continue;
        }
        if i + 8 > n || !chars[i..i + 8].iter().all(|c| c.is_ascii_digit()) {
            continue;
        }

        let date: String = chars[i..i + 8].iter().collect();
        let year: i32 = date[0..4].parse().unwrap_or(0);
        let month: u32 = date[4..6].parse().unwrap_or(0);
        let day: u32 = date[6..8].parse().unwrap_or(0);

        if !(1990..=2100).contains(&year) || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
            continue;
        }

        // Optional HHMMSS, allowing one separator between date and time.
        let mut j = i + 8;
        if j < n && !chars[j].is_ascii_digit() {
            j += 1;
        }
        if j + 6 <= n && chars[j..j + 6].iter().all(|c| c.is_ascii_digit()) {
            let time: String = chars[j..j + 6].iter().collect();
            let hour: u32 = time[0..2].parse().unwrap_or(99);
            let min: u32 = time[2..4].parse().unwrap_or(99);
            let sec: u32 = time[4..6].parse().unwrap_or(99);
            if hour < 24 && min < 60 && sec < 60 {
                return Some(format!(
                    "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
                    year, month, day, hour, min, sec
                ));
            }
        }

        return Some(format!("{:04}-{:02}-{:02}T00:00:00", year, month, day));
    }

    None
}

fn parse_exif_datetime(s: &str) -> Option<String> {
    let parts: Vec<&str> = s.split(' ').collect();
    if parts.len() != 2 {
        return None;
    }
    
    let date_part = parts[0].replace(':', "-");
    let time_part = parts[1];

    Some(format!("{}T{}", date_part, time_part))
}
