use std::path::Path;

/// Render a path for display with forward-slash separators on every platform.
///
/// Windows `Path::display()` emits native backslashes, and when a user-typed
/// forward-slash root (e.g. `D:/R5Reloaded`) is joined with additional segments,
/// the result is mixed (`D:/R5Reloaded\R5R Library\LIVE`). Run every path that
/// leaves the backend for the UI (or gets persisted into settings and later
/// shown) through this helper so the display is consistently `/`-separated.
pub fn display_slash(path: impl AsRef<Path>) -> String {
    path.as_ref().display().to_string().replace('\\', "/")
}

/// Normalize an already-stringified path (e.g. a manifest-relative entry path
/// that uses backslashes) to forward slashes. Use when you don't have a `Path`
/// on hand.
pub fn normalize_slashes(s: &str) -> String {
    s.replace('\\', "/")
}
