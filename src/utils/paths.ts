// Display helpers for paths that came back from the backend.
//
// These receive native paths: POSIX on macOS and Linux, backslash-separated on
// Windows. Splitting on '/' alone left every Windows path unsplit, so the UI
// showed a full path where it meant to show a filename, and SummaryPage's
// "Open Log Folder" derived a directory that does not exist.
//
// Splitting on both separators unconditionally is not the answer either: on
// macOS and Linux a backslash is a legal character *inside* a filename, so
// `My\File.mp4` would be truncated to `File.mp4`. So the convention is detected
// per path, and a backslash only counts as a separator on a path that is
// recognisably a Windows one.

/** Whether `path` uses Windows conventions: a drive letter, or a UNC share. */
function isWindowsPath(path: string): boolean {
  return /^[A-Za-z]:[\\/]/.test(path) || path.startsWith('\\\\');
}

/** Index of the last separator in `path`, or -1 if it has none. */
function lastSeparator(path: string): number {
  const slash = path.lastIndexOf('/');
  if (!isWindowsPath(path)) {
    return slash;
  }
  return Math.max(slash, path.lastIndexOf('\\'));
}

/**
 * Last segment of `path` — the filename, for showing a file by name rather
 * than in full. Returns `path` unchanged when it has no separator.
 */
export function getFileName(path: string): string {
  const cut = lastSeparator(path);
  if (cut === -1) return path;
  return path.slice(cut + 1) || path;
}

/**
 * Everything before the last segment — the containing directory.
 *
 * Used to turn a log file path into the folder to reveal, so it has to keep
 * whichever separators the platform actually uses.
 */
export function getDirName(path: string): string {
  const cut = lastSeparator(path);
  if (cut <= 0) return path;
  return path.slice(0, cut);
}

/**
 * Name of the directory containing `path`, without the rest of the path.
 * Shown under the current filename during an ingest for a sense of location.
 */
export function getFolderName(path: string): string {
  const dir = getDirName(path);
  if (dir === path) return '';
  return getFileName(dir);
}
