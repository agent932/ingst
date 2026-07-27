use crate::ingest::formats::{self, MetadataSource};
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
    match formats::classify(file_path).map(|f| f.metadata) {
        Some(MetadataSource::Exif) => extract_photo_metadata_sync(file_path),
        Some(MetadataSource::Ffprobe) => extract_video_metadata_sync(file_path),
        // Audio, the proprietary RAW containers, and non-media files have no
        // reader; the caller falls back to the filename timestamp.
        _ => (None, None),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    // --- timestamp_from_filename -----------------------------------------

    /// The last resort for a card whose files carry no metadata at all. If the
    /// stamp in the name is not read, every clip on that card is filed by the
    /// date it was *copied*, so a shoot from last year lands in this month.
    #[test]
    fn timestamp_from_filename_reads_camera_stamps() {
        assert_eq!(
            timestamp_from_filename("VID_20260723_073020_002.mp4").as_deref(),
            Some("2026-07-23T07:30:20")
        );
        assert_eq!(
            timestamp_from_filename("IMG_20260723_072938_001.jpg").as_deref(),
            Some("2026-07-23T07:29:38")
        );
        // No prefix at all.
        assert_eq!(
            timestamp_from_filename("20260723_073020.mov").as_deref(),
            Some("2026-07-23T07:30:20")
        );
        // Pixel appends milliseconds to the time; the extra digits are noise.
        assert_eq!(
            timestamp_from_filename("PXL_20260723_073020123.jpg").as_deref(),
            Some("2026-07-23T07:30:20")
        );
        // Date but no time: keep the date, which is what the folder needs.
        assert_eq!(
            timestamp_from_filename("20260723.jpg").as_deref(),
            Some("2026-07-23T00:00:00")
        );
    }

    /// Serial numbers and clip counters must never be mistaken for a date.
    /// A misread here does not just mislabel one file: it invents a folder like
    /// `1120/26/` and buries the footage where nobody thinks to look.
    #[test]
    fn timestamp_from_filename_rejects_serial_numbers() {
        for name in [
            "IMG_112026072312345.jpg", // long serial that contains a date-shaped run
            "MVI_1234567890.MOV",
            "DSC_0001.JPG",
            "C0001.MP4",
            "GX010042.MP4",
            "100_0001.MOV",
            "clip.mp4",
        ] {
            assert_eq!(
                timestamp_from_filename(name),
                None,
                "{name:?} is not a timestamp and must not be read as one"
            );
        }
    }

    /// A digit run that looks like a date but cannot be one is a serial number,
    /// not a capture time. Accepting it would file footage under an impossible
    /// month — an unreachable folder for the user, and a wrong one forever.
    #[test]
    fn timestamp_from_filename_rejects_impossible_dates() {
        for name in [
            "VID_20261323_073020.mp4", // month 13
            "VID_20260732_073020.mp4", // day 32
            "VID_20260700_073020.mp4", // day 00
            "VID_20260023_073020.mp4", // month 00
            "VID_19891231_120000.mp4", // before the range cameras plausibly report
            "VID_21010101_120000.mp4", // after it
        ] {
            assert_eq!(timestamp_from_filename(name), None, "{name:?} must be rejected");
        }

        // The edges of the accepted range still work.
        assert_eq!(
            timestamp_from_filename("VID_19900101_000000.mp4").as_deref(),
            Some("1990-01-01T00:00:00")
        );
        assert_eq!(
            timestamp_from_filename("VID_21001231_235959.mp4").as_deref(),
            Some("2100-12-31T23:59:59")
        );
    }

    /// A nonsense time must not throw away a perfectly good date — the folder
    /// only needs YYYY/MM, so degrade to midnight rather than to "no date" and
    /// a fallback on the copy date.
    #[test]
    fn timestamp_from_filename_keeps_the_date_when_the_time_is_impossible() {
        assert_eq!(
            timestamp_from_filename("VID_20260723_993020.mp4").as_deref(),
            Some("2026-07-23T00:00:00")
        );
        assert_eq!(
            timestamp_from_filename("VID_20260723_076099.mp4").as_deref(),
            Some("2026-07-23T00:00:00")
        );
    }

    // --- combine_make_model ----------------------------------------------

    /// The device name becomes a folder name. Repeating the brand splits one
    /// camera's work across "Canon EOS R5" and "Canon Canon EOS R5" folders;
    /// dropping it leaves an unidentifiable "Luna Ultra".
    #[test]
    fn combine_make_model_adds_the_brand_only_when_missing() {
        let combine = |a: &str, b: &str| combine_make_model(Some(a.into()), Some(b.into()));

        assert_eq!(combine("Canon", "Canon EOS R5").as_deref(), Some("Canon EOS R5"));
        assert_eq!(
            combine("Insta360", "Luna Ultra").as_deref(),
            Some("Insta360 Luna Ultra")
        );
        // Brand match is case-insensitive: EXIF casing is not consistent.
        assert_eq!(combine("NIKON", "Nikon D850").as_deref(), Some("Nikon D850"));
        assert_eq!(combine("SONY", "ILCE-7SM3").as_deref(), Some("SONY ILCE-7SM3"));
    }

    /// Cards where only one of the two EXIF fields is present still have to
    /// yield a usable folder name rather than falling through to the card label.
    #[test]
    fn combine_make_model_handles_missing_fields() {
        assert_eq!(combine_make_model(Some("DJI".into()), None).as_deref(), Some("DJI"));
        assert_eq!(
            combine_make_model(None, Some("DC-GH6".into())).as_deref(),
            Some("DC-GH6")
        );
        assert_eq!(combine_make_model(None, None), None);
    }

    // --- parse_exif_datetime ---------------------------------------------

    /// EXIF writes `2026:07:23 07:30:20`. Only the date's colons are separators
    /// — replacing the ones in the time too produces `07-30-20`, which nothing
    /// downstream can parse, and every photo would fall back to its file date.
    #[test]
    fn parse_exif_datetime_converts_only_the_date_separators() {
        assert_eq!(
            parse_exif_datetime("2026:07:23 07:30:20").as_deref(),
            Some("2026-07-23T07:30:20")
        );
    }

    /// Malformed EXIF is common on recovered or third-party-written files.
    /// Returning a half-parsed string would put the file in a garbage folder;
    /// returning None correctly falls back to the file's modified time.
    #[test]
    fn parse_exif_datetime_rejects_malformed_input() {
        assert_eq!(parse_exif_datetime(""), None);
        assert_eq!(parse_exif_datetime("2026:07:23"), None);
        assert_eq!(parse_exif_datetime("2026:07:23 07:30:20 "), None);
        assert_eq!(parse_exif_datetime("2026:07:23  07:30:20"), None);
    }

    // --- normalize_to_local ----------------------------------------------

    /// ffprobe reports `creation_time` in UTC. A clip shot at 21:00 local on
    /// the 31st is 04:00 UTC on the 1st, so leaving it unconverted files it in
    /// the *next month's* folder, split from the stills of the same shoot.
    #[test]
    fn normalize_to_local_preserves_the_instant_and_drops_the_zone() {
        use chrono::{Local, NaiveDateTime, TimeZone, Utc};

        for (input, expected_utc) in [
            ("2026-07-23T07:30:20Z", Utc.with_ymd_and_hms(2026, 7, 23, 7, 30, 20).unwrap()),
            (
                "2026-07-23T09:30:20+02:00",
                Utc.with_ymd_and_hms(2026, 7, 23, 7, 30, 20).unwrap(),
            ),
        ] {
            let out = normalize_to_local(input);
            let naive = NaiveDateTime::parse_from_str(&out, "%Y-%m-%dT%H:%M:%S")
                .unwrap_or_else(|e| panic!("{input:?} -> {out:?} is not plain local time: {e}"));
            let as_local = Local
                .from_local_datetime(&naive)
                .single()
                .expect("test instant must be unambiguous in the local zone");
            assert_eq!(
                as_local.with_timezone(&Utc),
                expected_utc,
                "{input:?} -> {out:?} moved the moment the clip was shot"
            );
        }
    }

    /// A timestamp with no zone is already local wall-clock time (EXIF), so it
    /// must pass through untouched — shifting it by the UTC offset would move
    /// evening shoots into the following day.
    #[test]
    fn normalize_to_local_leaves_zoneless_timestamps_alone() {
        assert_eq!(normalize_to_local("2026-07-23T07:30:20"), "2026-07-23T07:30:20");
        assert_eq!(normalize_to_local("2026-07-23 07:30:20"), "2026-07-23 07:30:20");
        assert_eq!(normalize_to_local("not a timestamp"), "not a timestamp");
    }
}
