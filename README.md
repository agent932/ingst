# Ingst - Media Ingestion Desktop Application

A polished desktop app that helps creators ingest footage from cameras, SD cards, drones, phones, and folders into a clean library structure. Built with Tauri v2 + React + TypeScript.

## Features

- **Multi-source ingestion**: Add folders, SD cards, or drives as sources
- **Smart organization**: Organize files by date (YYYY/MM) and device name
- **Copy or Move**: Choose between copying (keeps originals) or moving (transfers files)
- **Verified transfers**: Every copy is checksummed against its source and only
  moved into place once it matches, so a file in your library is never a partial
  or corrupt one
- **Duplicate detection**: Files already ingested are skipped, confirmed by
  whole-file hash rather than a sample
- **Metadata extraction**: Reads EXIF for photos and container metadata for
  video, falling back to camera filename conventions and to the device that
  tagged neighbouring files
- **Sidecar handling**: `.xmp`, `.srt`, `.lut`, `.xml` and `.edl` companions
  travel with their clip, keeping its filename
- **Progress tracking**: Live byte progress, speed, and time estimates, through
  both the copy and the verification pass
- **Detailed logging**: JSON logs saved to your library for auditing

## Installation

### macOS

**Option 1: DMG Installer**
1. Download the `.dmg` or `.app.tar.gz` for your Mac from the latest release
2. If using DMG: Open and drag `Ingst.app` to Applications
3. If using .tar.gz: Extract and move `Ingst.app` to Applications
4. **If you see "damaged" error**: 
   - Right-click the app → "Open" → Click "Open" in the dialog
   - Or run: `xattr -cr /Applications/Ingst.app`

**Option 2: Build from Source**
```bash
# Install Rust if not already installed
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
cd ingst
npm install
npm run tauri build
```

The built app will be at: `src-tauri/target/release/bundle/macos/Ingst.app`

### Windows

**Option 1: Build from Source**
```bash
# Install Rust if not already installed
winget install Rustlang.Rust.MSVC

# Clone and build
cd ingst
npm install
npm run tauri build
```

The built .exe will be at: `src-tauri/target/release/ingst.exe`

## Development

```bash
# Install dependencies
npm install

# Run in development mode
npm run tauri dev

# Build for production
npm run tauri build
```

## Project Structure

```
ingst/
├── src/                    # React frontend
│   ├── components/         # UI components
│   ├── pages/              # Wizard step pages
│   ├── store/              # Zustand state management
│   └── utils/              # Formatting and path helpers
├── src-tauri/              # Rust backend
│   ├── src/
│   │   ├── commands.rs     # Tauri commands
│   │   ├── ingest/         # Ingest engine
│   │   │   ├── formats.rs  # Supported media formats (single source of truth)
│   │   │   ├── scanner.rs  # File scanning
│   │   │   ├── metadata.rs # Metadata extraction
│   │   │   ├── plan.rs     # Build ingest plan
│   │   │   ├── executor.rs # Execute operations
│   │   │   └── logging.rs  # Logs and the duplicate index
│   │   └── utils/          # Hashing and path helpers
│   ├── binaries/           # ffmpeg/ffprobe sidecars (fetched, not committed)
│   └── tauri.conf.json     # Tauri configuration
└── README.md
```

Adding support for a new camera format means one line in
`src-tauri/src/ingest/formats.rs`.

## How files are verified

Copies are written to a `<name>.part` sidecar, hashed, and renamed into place
only once the hash matches the source. Rename is atomic, so a file sitting at
its final path has always been transferred completely and checked. If an ingest
is interrupted, only a `.part` file is left behind, which later runs ignore.

Moves across volumes take the same route and unlink the original only after the
copy verifies. Moves within a volume are a plain rename.

The source hash is computed while copying rather than by re-reading the file
afterwards, so verification costs one extra read of the destination rather than
a second pass over the card.

## Supported File Types

- **Video**: .mp4, .mov, .mxf, .avi, .mkv
- **Photo**: .jpg, .jpeg, .png, .braw, .r3d, .arw, .cr2, .nef, .dng
- **Audio**: .wav, .mp3, .aac

## Organization Structure

Files are organized as:
```
<DEST_ROOT>/<YYYY>/<MM>/<DEVICE_NAME>/
```

Example:
```
/MediaLibrary/
├── 2024/
│   ├── 01/
│   │   └── iPhone15/
│   │       └── video.mp4
│   └── 03/
│       └── SonyA7IV/
│           └── photo.arw
```

## Settings

- **Theme**: Light, Dark, or System
- **Default operation**: Copy or Move
- **Skip duplicates**: Enabled by default

Settings are stored in:
- macOS: `~/Library/Application Support/ingst/settings.json`
- Windows: `%APPDATA%/ingst/settings.json`

## Post-MVP Backlog

- Drag and drop folders or cards onto the window
- Mirror to two destinations at once (working drive + backup)
- MHL / checksum manifest export for camera-to-post handoff
- Resume an interrupted ingest, and retry only failed files
- Per-source device name override
- Custom folder naming tokens (`{YYYY}/{MM}/{DD}/{device}/{project}`)
- Per-device rules (GoPro chapters, DJI folders)
- Proxy generation / transcode presets
- Integration hooks (DaVinci Resolve / Premiere folder templates)
- Watch folder automation

## License

MIT
