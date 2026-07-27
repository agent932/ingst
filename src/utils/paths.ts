// Display helpers for paths that came back from the backend.

/**
 * Last path segment of `path`, for showing a file by name rather than in full.
 *
 * Splits on '/' only. That is correct on macOS and Linux, where a backslash is
 * a legal character inside a filename, and is what every caller has always
 * done. Windows paths are not handled here.
 */
export function getFileName(path: string): string {
  return path.split('/').pop() || path;
}
