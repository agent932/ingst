# Changelog

## v0.2.0 (unreleased)

A correctness release. Every fix below was verified against real camera media,
mostly an Insta360 Luna Ultra card.

### Fixed — data loss

- **Same-named files no longer overwrite each other.** Destination names were
  checked against the disk, but at plan time nothing has been written yet, so
  two files with the same name bound for the same folder were given the same
  destination and the second replaced the first. Camera folder rollover
  (`100CANON/IMG_0001.JPG` and `101CANON/IMG_0001.JPG`) hit this every time.
  Destinations are now reserved as the plan is built.
- **Duplicates are confirmed before being skipped.** The duplicate check
  sampled a file's head, tail and length; two files agreeing on those but
  differing in between were treated as identical and the second was never
  copied. A match is now confirmed against a whole-file hash.
- **Files of 64–128 KiB were fingerprinted from their first 64 KiB alone**,
  making the rest of the file — including its last byte — invisible to the
  duplicate check.
- **Interrupted copies no longer leave corrupt files.** Copies were written
  straight to their final path, so a crash or an unplugged drive left a
  truncated file under a real media name that nothing marked as bad, and that
  the next ingest duplicated rather than repaired. Copies now go to a `.part`
  sidecar and are renamed into place only after verifying.

### Fixed — organisation

- **Video no longer lands in a folder named after the file.** The QuickTime
  metadata key was misspelled, so no video ever resolved a device name, and the
  fallback used the file's own path — every clip got its own folder. Files now
  inherit a device name from others in the same directory, which is how a
  camera that tags stills but not video (like the Luna Ultra) keeps one shoot
  in one folder.
- **Stills and clips from the same moment file together.** Container
  timestamps are UTC while EXIF is local, so an evening shoot near a month
  boundary put video in the following month. Container timestamps are now
  converted to local time, and camera filename stamps are read as a fallback.
- **EXIF make and model are combined**, so an ambiguous "Luna Ultra" becomes
  "Insta360 Luna Ultra" without turning "Canon EOS R5" into "Canon Canon
  EOS R5".
- **Sidecars are planned once, keeping their clip's filename case.** On
  case-insensitive filesystems every companion file was planned twice, the
  second landing as `a001_1.xmp`, and names were lower-cased so an NLE could no
  longer pair `A001.MP4` with `a001.xmp`. On a case-sensitive source they were
  missed entirely and left on the card.
- **macOS AppleDouble stubs are no longer ingested as media.** `._IMG_0001.DNG`
  carries a real extension and was being copied in as a 4 KB photo.

### Fixed — counts, progress and state

- **Skipped files are no longer counted as successes**, which had inflated the
  ingested total, pushed the byte progress past 100%, and written two log
  entries per skip.
- **Progress updates during a transfer.** Events were funnelled through a
  blocking channel read inside an async task, which parked a worker; the
  interface sat on "Starting…" for an entire ingest while files copied
  normally underneath.
- **Checksum verification is visible**, with live hashing progress, so a large
  file no longer looks stalled while it is being verified.
- **Cancelling reports that it cancelled**, along with how many files were
  never started, instead of claiming the ingest completed.
- **The ingest no longer re-runs** when returning to that step.
- **Settings are loaded at startup.** They were saved after every ingest and
  never read back, so theme, default operation and last destination were
  discarded on every launch.

### Fixed — devices and packaging

- **Volumes whose name contains a space are detected.** `/Volumes/SD Card` was
  truncated to `/Volumes/SD` and scanned as empty.
- **Released macOS builds ship a working ffmpeg.** The bundled binaries were
  copied from Homebrew and dynamically linked against paths that do not exist
  on a user's machine, so every release silently lost video thumbnails and
  metadata. Builds now fetch statically linked binaries pinned by tag and
  checksum, and CI fails if a sidecar is not self-contained.
- **The duplicate index survives log rotation.** It was rebuilt from the last
  50 logs, so on the 51st ingest the oldest files were re-imported as new.

### Changed

- Verification hashes the source while copying rather than re-reading it,
  cutting roughly a third of the I/O per file.
- Media formats are defined in one registry instead of four hardcoded lists.
- Source scanning skips hidden and system directories, and per-source
  exclusions now work.

### Known issues

- `parse_date_for_path` can return fewer than seven characters for an unpadded
  month, which panics the plan. One malformed file aborts the whole ingest.

## v0.1.0 (2026-02-24)
- Initial release
- Multi-source ingestion (folders, SD cards, drives)
- Smart organization by date (YYYY/MM) and device
- Copy or Move operations
- Duplicate detection with fast hashing
- Metadata extraction (EXIF for photos)
- Real-time progress tracking
- Light/Dark/System theme support
- SD card auto-detection
- JSON logging to .ingst/logs/
