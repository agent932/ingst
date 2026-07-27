# ffmpeg / ffprobe sidecars

Place ffprobe and ffmpeg here so Ingst can:
- Extract capture dates and device names from video files (`ffprobe`)
- Generate video thumbnails in the Review screen (`ffmpeg`)

**The binaries must be statically linked.** Release builds fetch pinned,
checksummed static builds from
[eugeneware/ffmpeg-static](https://github.com/eugeneware/ffmpeg-static);
see `.github/workflows/release.yml`.

> Do **not** copy Homebrew's ffmpeg here. It is dynamically linked against
> `/opt/homebrew/Cellar/ffmpeg/<version>/lib/*.dylib`, so the copied executable
> cannot launch on any machine without that exact Homebrew install. Releases
> built that way shipped with video thumbnails and metadata silently disabled.
> CI now runs `otool -L` and fails the build if a sidecar references anything
> outside `/usr/lib` or `/System/Library`.

## Fetching them manually

```sh
REL=b6.1.1
BASE="https://github.com/eugeneware/ffmpeg-static/releases/download/$REL"

# macOS (Apple Silicon)
curl -fsSL "$BASE/ffmpeg-darwin-arm64"  -o src-tauri/binaries/ffmpeg-aarch64-apple-darwin
curl -fsSL "$BASE/ffprobe-darwin-arm64" -o src-tauri/binaries/ffprobe-aarch64-apple-darwin
chmod +x src-tauri/binaries/*-apple-darwin
# arm64 macOS will not exec an unsigned binary
codesign --force --sign - src-tauri/binaries/ffmpeg-aarch64-apple-darwin
codesign --force --sign - src-tauri/binaries/ffprobe-aarch64-apple-darwin

# Windows (x86_64) — rename with a .exe suffix
curl -fsSL "$BASE/ffmpeg-win32-x64"  -o src-tauri/binaries/ffmpeg-x86_64-pc-windows-msvc.exe
curl -fsSL "$BASE/ffprobe-win32-x64" -o src-tauri/binaries/ffprobe-x86_64-pc-windows-msvc.exe
```

Use `ffmpeg-darwin-x64` / `ffprobe-darwin-x64` and the
`x86_64-apple-darwin` suffix for Intel Macs.

Verify before bundling:

```sh
otool -L src-tauri/binaries/ffprobe-aarch64-apple-darwin   # system paths only
./src-tauri/binaries/ffprobe-aarch64-apple-darwin -version
```

The binaries are gitignored — they are fetched at release time, not committed.

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
