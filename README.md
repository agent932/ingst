# Ingst - Media Ingestion Desktop Application

A polished desktop app that helps creators ingest footage from cameras, SD cards, drones, phones, and folders into a clean library structure. Built with Tauri v2 + React + TypeScript.

## Features

- **Multi-source ingestion**: Add folders, SD cards, or drives as sources
- **Smart organization**: Organize files by date (YYYY/MM) and device name
- **Copy or Move**: Choose between copying (keeps originals) or moving (transfers files)
- **Duplicate detection**: Fast hash-based skip duplicates option
- **Metadata extraction**: Reads EXIF for photos, QuickTime metadata for videos
- **Progress tracking**: Real-time progress with speed and time estimates
- **Detailed logging**: JSON logs saved to your library for auditing

## Installation

### macOS

**Option 1: DMG Installer**
1. Download `Ingst_0.1.0_aarch64.dmg` or `Ingst_aarch64.app.tar.gz` from the releases
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
│   └── lib/                # Tauri command wrappers
├── src-tauri/              # Rust backend
│   ├── src/
│   │   ├── commands.rs     # Tauri commands
│   │   ├── ingest/         # Ingest engine
│   │   │   ├── scanner.rs  # File scanning
│   │   │   ├── metadata.rs # Metadata extraction
│   │   │   ├── plan.rs    # Build ingest plan
│   │   │   ├── executor.rs # Execute operations
│   │   │   └── logging.rs # Log management
│   │   └── utils/          # Helper utilities
│   └── tauri.conf.json     # Tauri configuration
├── SPEC.md                 # Detailed specification
└── README.md
```

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
- macOS: `~/.config/ingst/settings.json`
- Windows: `%APPDATA%/ingst/settings.json`

## Post-MVP Backlog

- Project templates ("Video Project", "Client Shoot")
- Per-device rules (GoPro chapters, DJI folders)
- Proxy generation / transcode presets
- Checksum verification mode
- Integration hooks (DaVinci Resolve / Premiere folder templates)
- Watch folder automation

## License

MIT
