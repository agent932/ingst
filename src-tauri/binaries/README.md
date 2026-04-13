# ffmpeg / ffprobe sidecars

Place ffprobe and ffmpeg here so Ingst can:
- Extract capture dates and device names from video files (`ffprobe`)
- Generate video thumbnails in the Review screen (`ffmpeg`)

Both binaries are included in every standard ffmpeg distribution.

## Windows (x86_64)

1. Download the latest ffmpeg static build from https://www.gyan.dev/ffmpeg/builds/
   (grab the "release essentials" zip)
2. Extract and copy into this directory:
   - `bin/ffprobe.exe` → `ffprobe-x86_64-pc-windows-msvc.exe`
   - `bin/ffmpeg.exe`  → `ffmpeg-x86_64-pc-windows-msvc.exe`

## macOS (Apple Silicon)

```sh
brew install ffmpeg
cp $(which ffprobe) src-tauri/binaries/ffprobe-aarch64-apple-darwin
cp $(which ffmpeg)  src-tauri/binaries/ffmpeg-aarch64-apple-darwin
```
Use `x86_64-apple-darwin` on Intel Macs.

## How it works

`sidecar_path()` in `src/utils/paths.rs` checks for the triple-suffixed binary
next to the running executable first, then falls back to whatever is on PATH.
During development, having ffmpeg/ffprobe on PATH is sufficient.

## Production bundling

Once both binaries are present, add to `tauri.conf.json` under `bundle`:
```json
"externalBin": ["binaries/ffprobe", "binaries/ffmpeg"]
```
Tauri will then include them in the installer automatically.
