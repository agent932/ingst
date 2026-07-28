//! The single source of truth for which file extensions Ingst treats as media,
//! and what can be done with each one.
//!
//! Everything that has to answer "is this a photo?", "can we read a capture
//! date out of it?" or "can we show a thumbnail for it?" goes through this
//! table. Adding support for a new camera format is one row here and nothing
//! else on the Rust side.
//!
//! The frontend keeps a small mirror of the video/audio lists in
//! `src/pages/ReviewPage.tsx` (`getFileType`) for picking a placeholder icon —
//! see the note there before editing this table.

use std::path::Path;

/// Which of the three library buckets a file lands in.
///
/// The string form is what crosses the IPC boundary as `ScannedFile.file_type`,
/// so these spellings are part of the frontend contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Video,
    Photo,
    Audio,
}

impl MediaKind {
    pub fn as_str(self) -> &'static str {
        match self {
            MediaKind::Video => "video",
            MediaKind::Photo => "photo",
            MediaKind::Audio => "audio",
        }
    }
}

/// Where a file's capture date and device name can be read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataSource {
    /// No reader for this format — the scanner falls back to the filename.
    Unsupported,
    /// EXIF, read in-process.
    Exif,
    /// Container-level tags. Which reader supplies them is a platform detail;
    /// see `ingest::video`.
    Container,
}

/// How a preview thumbnail is produced, if at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbnailSource {
    /// No thumbnail — the UI falls back to a file-type icon.
    Unsupported,
    /// Decoded in-process with the `image` crate.
    Image,
    /// A frame decoded out of the video. See `ingest::video` for the backend.
    VideoFrame,
}

/// One supported extension and everything Ingst knows about it.
pub struct MediaFormat {
    /// Lower-case, without the leading dot.
    pub extension: &'static str,
    pub kind: MediaKind,
    pub metadata: MetadataSource,
    pub thumbnail: ThumbnailSource,
}

/// Every extension the app will ingest.
///
/// An extension missing from this table is not merely unorganised — it is
/// invisible. The scanner skips it, so it is silently left behind on a card the
/// user is about to format. That makes breadth here a data-safety property, not
/// a convenience, and it is why formats are listed even when nothing can read
/// their metadata: filing a clip by its filename timestamp beats not copying it.
///
/// `Exif` is optimistic by design. Most raw formats are TIFF containers the EXIF
/// reader can open, and the ones it cannot cost a single failed file open before
/// falling back to the filename stamp and the device inferred from neighbouring
/// files. Claiming `Unsupported` where it might have worked would lose real
/// metadata; claiming `Exif` where it does not costs almost nothing.
pub const MEDIA_FORMATS: &[MediaFormat] = &[
    // ── Video ────────────────────────────────────────────────────────────────
    MediaFormat { extension: "mp4",  kind: MediaKind::Video, metadata: MetadataSource::Container,   thumbnail: ThumbnailSource::VideoFrame },
    MediaFormat { extension: "mov",  kind: MediaKind::Video, metadata: MetadataSource::Container,   thumbnail: ThumbnailSource::VideoFrame },
    MediaFormat { extension: "m4v",  kind: MediaKind::Video, metadata: MetadataSource::Container,   thumbnail: ThumbnailSource::VideoFrame },
    MediaFormat { extension: "avi",  kind: MediaKind::Video, metadata: MetadataSource::Container,   thumbnail: ThumbnailSource::VideoFrame },
    MediaFormat { extension: "mkv",  kind: MediaKind::Video, metadata: MetadataSource::Container,   thumbnail: ThumbnailSource::VideoFrame },
    MediaFormat { extension: "webm", kind: MediaKind::Video, metadata: MetadataSource::Container,   thumbnail: ThumbnailSource::VideoFrame },
    // Broadcast and cinema: Sony XDCAM, Canon XF, Panasonic P2.
    MediaFormat { extension: "mxf",  kind: MediaKind::Video, metadata: MetadataSource::Container,   thumbnail: ThumbnailSource::VideoFrame },
    // AVCHD, still produced by Sony and Panasonic camcorders.
    MediaFormat { extension: "mts",  kind: MediaKind::Video, metadata: MetadataSource::Container,   thumbnail: ThumbnailSource::VideoFrame },
    MediaFormat { extension: "m2ts", kind: MediaKind::Video, metadata: MetadataSource::Container,   thumbnail: ThumbnailSource::VideoFrame },
    // Insta360 360° video — an MP4 variant, so the container reader handles it.
    MediaFormat { extension: "insv", kind: MediaKind::Video, metadata: MetadataSource::Container,   thumbnail: ThumbnailSource::VideoFrame },
    MediaFormat { extension: "3gp",  kind: MediaKind::Video, metadata: MetadataSource::Container,   thumbnail: ThumbnailSource::VideoFrame },
    // Camera raw video. Both need vendor SDKs, so neither metadata nor a frame
    // is available — but they are unmistakably video, and they are the most
    // expensive files on the card to lose.
    MediaFormat { extension: "braw", kind: MediaKind::Video, metadata: MetadataSource::Unsupported, thumbnail: ThumbnailSource::Unsupported },
    MediaFormat { extension: "r3d",  kind: MediaKind::Video, metadata: MetadataSource::Unsupported, thumbnail: ThumbnailSource::Unsupported },
    MediaFormat { extension: "crm",  kind: MediaKind::Video, metadata: MetadataSource::Unsupported, thumbnail: ThumbnailSource::Unsupported },

    // ── Photo ────────────────────────────────────────────────────────────────
    MediaFormat { extension: "jpg",  kind: MediaKind::Photo, metadata: MetadataSource::Exif,        thumbnail: ThumbnailSource::Image },
    MediaFormat { extension: "jpeg", kind: MediaKind::Photo, metadata: MetadataSource::Exif,        thumbnail: ThumbnailSource::Image },
    MediaFormat { extension: "png",  kind: MediaKind::Photo, metadata: MetadataSource::Exif,        thumbnail: ThumbnailSource::Image },
    MediaFormat { extension: "tif",  kind: MediaKind::Photo, metadata: MetadataSource::Exif,        thumbnail: ThumbnailSource::Unsupported },
    MediaFormat { extension: "tiff", kind: MediaKind::Photo, metadata: MetadataSource::Exif,        thumbnail: ThumbnailSource::Unsupported },
    // Apple's default stills format since iOS 11 — the single most common
    // photo on any phone-shot card.
    MediaFormat { extension: "heic", kind: MediaKind::Photo, metadata: MetadataSource::Exif,        thumbnail: ThumbnailSource::Unsupported },
    MediaFormat { extension: "heif", kind: MediaKind::Photo, metadata: MetadataSource::Exif,        thumbnail: ThumbnailSource::Unsupported },
    MediaFormat { extension: "avif", kind: MediaKind::Photo, metadata: MetadataSource::Exif,        thumbnail: ThumbnailSource::Unsupported },
    MediaFormat { extension: "webp", kind: MediaKind::Photo, metadata: MetadataSource::Exif,        thumbnail: ThumbnailSource::Unsupported },
    // Adobe, Leica, DJI, and the raw format GoPro derives from.
    MediaFormat { extension: "dng",  kind: MediaKind::Photo, metadata: MetadataSource::Exif,        thumbnail: ThumbnailSource::Unsupported },
    // Manufacturer raw.
    MediaFormat { extension: "arw",  kind: MediaKind::Photo, metadata: MetadataSource::Exif,        thumbnail: ThumbnailSource::Unsupported }, // Sony
    MediaFormat { extension: "cr2",  kind: MediaKind::Photo, metadata: MetadataSource::Exif,        thumbnail: ThumbnailSource::Unsupported }, // Canon, pre-2018
    MediaFormat { extension: "cr3",  kind: MediaKind::Photo, metadata: MetadataSource::Exif,        thumbnail: ThumbnailSource::Unsupported }, // Canon, R and M series
    MediaFormat { extension: "nef",  kind: MediaKind::Photo, metadata: MetadataSource::Exif,        thumbnail: ThumbnailSource::Unsupported }, // Nikon
    MediaFormat { extension: "nrw",  kind: MediaKind::Photo, metadata: MetadataSource::Exif,        thumbnail: ThumbnailSource::Unsupported }, // Nikon compacts
    MediaFormat { extension: "rw2",  kind: MediaKind::Photo, metadata: MetadataSource::Exif,        thumbnail: ThumbnailSource::Unsupported }, // Panasonic Lumix
    MediaFormat { extension: "raf",  kind: MediaKind::Photo, metadata: MetadataSource::Exif,        thumbnail: ThumbnailSource::Unsupported }, // Fujifilm
    MediaFormat { extension: "orf",  kind: MediaKind::Photo, metadata: MetadataSource::Exif,        thumbnail: ThumbnailSource::Unsupported }, // OM System, Olympus
    MediaFormat { extension: "pef",  kind: MediaKind::Photo, metadata: MetadataSource::Exif,        thumbnail: ThumbnailSource::Unsupported }, // Pentax
    MediaFormat { extension: "srw",  kind: MediaKind::Photo, metadata: MetadataSource::Exif,        thumbnail: ThumbnailSource::Unsupported }, // Samsung
    MediaFormat { extension: "3fr",  kind: MediaKind::Photo, metadata: MetadataSource::Exif,        thumbnail: ThumbnailSource::Unsupported }, // Hasselblad
    MediaFormat { extension: "iiq",  kind: MediaKind::Photo, metadata: MetadataSource::Exif,        thumbnail: ThumbnailSource::Unsupported }, // Phase One
    MediaFormat { extension: "gpr",  kind: MediaKind::Photo, metadata: MetadataSource::Exif,        thumbnail: ThumbnailSource::Unsupported }, // GoPro raw, DNG-based
    MediaFormat { extension: "insp", kind: MediaKind::Photo, metadata: MetadataSource::Exif,        thumbnail: ThumbnailSource::Unsupported }, // Insta360 360° still

    // ── Audio ────────────────────────────────────────────────────────────────
    // External recorders and wireless mics are part of the same shoot, and are
    // routinely on the same card or dumped alongside it.
    MediaFormat { extension: "wav",  kind: MediaKind::Audio, metadata: MetadataSource::Unsupported, thumbnail: ThumbnailSource::Unsupported },
    MediaFormat { extension: "mp3",  kind: MediaKind::Audio, metadata: MetadataSource::Unsupported, thumbnail: ThumbnailSource::Unsupported },
    MediaFormat { extension: "aac",  kind: MediaKind::Audio, metadata: MetadataSource::Unsupported, thumbnail: ThumbnailSource::Unsupported },
    MediaFormat { extension: "m4a",  kind: MediaKind::Audio, metadata: MetadataSource::Unsupported, thumbnail: ThumbnailSource::Unsupported },
    MediaFormat { extension: "flac", kind: MediaKind::Audio, metadata: MetadataSource::Unsupported, thumbnail: ThumbnailSource::Unsupported },
    MediaFormat { extension: "aif",  kind: MediaKind::Audio, metadata: MetadataSource::Unsupported, thumbnail: ThumbnailSource::Unsupported },
    MediaFormat { extension: "aiff", kind: MediaKind::Audio, metadata: MetadataSource::Unsupported, thumbnail: ThumbnailSource::Unsupported },
];

/// Lower-cased extension of `path` without the dot, or `""` when it has none.
pub fn extension_of(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// Look up an already-lower-cased extension (no leading dot).
pub fn lookup(extension: &str) -> Option<&'static MediaFormat> {
    MEDIA_FORMATS.iter().find(|f| f.extension == extension)
}

/// Classify a path by its extension. `None` means "not a media file".
pub fn classify(path: &Path) -> Option<&'static MediaFormat> {
    lookup(&extension_of(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn kind_of(name: &str) -> Option<MediaKind> {
        classify(Path::new(name)).map(|f| f.kind)
    }

    /// Every device below is one a creator plausibly hands us. A format missing
    /// here is not badly filed — it is invisible, and gets left on a card that
    /// is about to be formatted. This list is the contract.
    #[test]
    fn recognises_what_real_devices_write() {
        let cases: &[(&str, MediaKind, &str)] = &[
            // Mirrorless / DSLR video
            ("C0001.MP4",              MediaKind::Video, "Sony XAVC S"),
            ("A001_01.MOV",            MediaKind::Video, "Canon, Nikon, Panasonic"),
            ("00000.MTS",              MediaKind::Video, "AVCHD camcorder"),
            ("00001.M2TS",             MediaKind::Video, "AVCHD camcorder"),
            ("A001C001.MXF",           MediaKind::Video, "Sony XDCAM / Canon XF / P2"),
            // Cinema raw
            ("A001_C001.braw",         MediaKind::Video, "Blackmagic"),
            ("A001_C001_001.R3D",      MediaKind::Video, "RED"),
            ("A001C001.CRM",           MediaKind::Video, "Canon Cinema RAW Light"),
            // Action and 360
            ("GX010001.MP4",           MediaKind::Video, "GoPro"),
            ("VID_20260723_073020.insv", MediaKind::Video, "Insta360 360"),
            ("IMG_20260723_072938.insp", MediaKind::Photo, "Insta360 360 still"),
            ("GOPR0001.GPR",           MediaKind::Photo, "GoPro raw"),
            // Drones
            ("DJI_0001.MP4",           MediaKind::Video, "DJI"),
            ("DJI_0001.DNG",           MediaKind::Photo, "DJI raw still"),
            // Stills
            ("IMG_0001.HEIC",          MediaKind::Photo, "iPhone default since iOS 11"),
            ("IMG_0001.CR3",           MediaKind::Photo, "Canon R / M series"),
            ("IMG_0001.CR2",           MediaKind::Photo, "Canon, pre-2018"),
            ("DSC00001.ARW",           MediaKind::Photo, "Sony"),
            ("DSC_0001.NEF",           MediaKind::Photo, "Nikon"),
            ("P1000001.RW2",           MediaKind::Photo, "Panasonic Lumix"),
            ("DSCF0001.RAF",           MediaKind::Photo, "Fujifilm"),
            ("P7230001.ORF",           MediaKind::Photo, "OM System"),
            ("IMGP0001.PEF",           MediaKind::Photo, "Pentax"),
            // Sound
            ("ZOOM0001.WAV",           MediaKind::Audio, "field recorder"),
            ("REC001.m4a",             MediaKind::Audio, "wireless mic / phone"),
            ("track.flac",             MediaKind::Audio, "recorder"),
        ];

        for (name, expected, device) in cases {
            assert_eq!(
                kind_of(name),
                Some(*expected),
                "{} ({}) must be recognised as {:?}",
                name, device, expected
            );
        }
    }

    /// Camera raw *video* must not be filed as stills. Both were previously
    /// classified as photos, which made the scan counts wrong and drew a photo
    /// icon next to a cinema clip.
    #[test]
    fn cinema_raw_video_is_video_not_photo() {
        for name in ["a.braw", "a.r3d", "a.crm"] {
            assert_eq!(kind_of(name), Some(MediaKind::Video), "{} is video", name);
        }
    }

    /// Case comes off the card however the camera wrote it.
    #[test]
    fn classification_ignores_extension_case() {
        assert_eq!(kind_of("A.MP4"), kind_of("a.mp4"));
        assert_eq!(kind_of("A.HEIC"), kind_of("a.heic"));
    }

    /// Non-media must stay out: the user asked for their edit files to be left
    /// alone, and proxies belong to the camera, not the library.
    #[test]
    fn leaves_non_media_alone() {
        for name in ["edit.psd", "clip.lrv", "clip.LRF", "clip.thm", "notes.txt", "index.json"] {
            assert_eq!(kind_of(name), None, "{} must not be ingested", name);
        }
    }

    #[test]
    fn no_duplicate_extensions() {
        let mut seen = std::collections::HashSet::new();
        for f in MEDIA_FORMATS {
            assert!(seen.insert(f.extension), "duplicate entry for {}", f.extension);
            assert_eq!(f.extension, f.extension.to_lowercase(), "table must be lowercase");
        }
    }
}
