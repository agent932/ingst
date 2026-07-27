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
    /// Container tags, read with the bundled `ffprobe` sidecar.
    Ffprobe,
}

/// How a preview thumbnail is produced, if at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbnailSource {
    /// No thumbnail — the UI falls back to a file-type icon.
    Unsupported,
    /// Decoded in-process with the `image` crate.
    Image,
    /// One frame grabbed with the bundled `ffmpeg` sidecar.
    Ffmpeg,
}

/// One supported extension and everything Ingst knows about it.
pub struct MediaFormat {
    /// Lower-case, without the leading dot.
    pub extension: &'static str,
    pub kind: MediaKind,
    pub metadata: MetadataSource,
    pub thumbnail: ThumbnailSource,
}

/// Every extension Ingst will ingest. Anything absent counts as "other" and is
/// left on the card.
///
/// Note the two RAW formats that read as photos but support neither EXIF nor
/// thumbnailing: `braw` and `r3d` are proprietary containers that neither the
/// `exif` crate nor the `image` crate can open.
pub const MEDIA_FORMATS: &[MediaFormat] = &[
    // Video — container tags via ffprobe, thumbnail frame via ffmpeg.
    MediaFormat { extension: "mp4",  kind: MediaKind::Video, metadata: MetadataSource::Ffprobe,     thumbnail: ThumbnailSource::Ffmpeg },
    MediaFormat { extension: "mov",  kind: MediaKind::Video, metadata: MetadataSource::Ffprobe,     thumbnail: ThumbnailSource::Ffmpeg },
    MediaFormat { extension: "mxf",  kind: MediaKind::Video, metadata: MetadataSource::Ffprobe,     thumbnail: ThumbnailSource::Ffmpeg },
    MediaFormat { extension: "avi",  kind: MediaKind::Video, metadata: MetadataSource::Ffprobe,     thumbnail: ThumbnailSource::Ffmpeg },
    MediaFormat { extension: "mkv",  kind: MediaKind::Video, metadata: MetadataSource::Ffprobe,     thumbnail: ThumbnailSource::Ffmpeg },

    // Photo — EXIF in-process; only the web-ish formats can be decoded for a thumbnail.
    MediaFormat { extension: "jpg",  kind: MediaKind::Photo, metadata: MetadataSource::Exif,        thumbnail: ThumbnailSource::Image },
    MediaFormat { extension: "jpeg", kind: MediaKind::Photo, metadata: MetadataSource::Exif,        thumbnail: ThumbnailSource::Image },
    MediaFormat { extension: "png",  kind: MediaKind::Photo, metadata: MetadataSource::Exif,        thumbnail: ThumbnailSource::Image },
    MediaFormat { extension: "arw",  kind: MediaKind::Photo, metadata: MetadataSource::Exif,        thumbnail: ThumbnailSource::Unsupported },
    MediaFormat { extension: "cr2",  kind: MediaKind::Photo, metadata: MetadataSource::Exif,        thumbnail: ThumbnailSource::Unsupported },
    MediaFormat { extension: "nef",  kind: MediaKind::Photo, metadata: MetadataSource::Exif,        thumbnail: ThumbnailSource::Unsupported },
    MediaFormat { extension: "dng",  kind: MediaKind::Photo, metadata: MetadataSource::Exif,        thumbnail: ThumbnailSource::Unsupported },
    MediaFormat { extension: "braw", kind: MediaKind::Photo, metadata: MetadataSource::Unsupported, thumbnail: ThumbnailSource::Unsupported },
    MediaFormat { extension: "r3d",  kind: MediaKind::Photo, metadata: MetadataSource::Unsupported, thumbnail: ThumbnailSource::Unsupported },

    // Audio — no metadata reader and no thumbnail today.
    MediaFormat { extension: "wav",  kind: MediaKind::Audio, metadata: MetadataSource::Unsupported, thumbnail: ThumbnailSource::Unsupported },
    MediaFormat { extension: "mp3",  kind: MediaKind::Audio, metadata: MetadataSource::Unsupported, thumbnail: ThumbnailSource::Unsupported },
    MediaFormat { extension: "aac",  kind: MediaKind::Audio, metadata: MetadataSource::Unsupported, thumbnail: ThumbnailSource::Unsupported },
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
