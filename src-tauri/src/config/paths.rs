use std::path::{Path, PathBuf};

pub const LIBRARY_DIR_NAME: &str = "R5R Library";

/// Compose `<library_root>/R5R Library/<CHANNEL_UPPERCASE>/`.
pub fn install_dir(library_root: &Path, channel_name: &str) -> PathBuf {
    library_root
        .join(LIBRARY_DIR_NAME)
        .join(channel_name.to_uppercase())
}

/// True if the path's components match `.../R5R Library/<something>/`.
pub fn looks_like_channel_dir(p: &Path) -> bool {
    let comps: Vec<_> = p
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();
    if comps.len() < 2 {
        return false;
    }
    comps[comps.len() - 2].eq_ignore_ascii_case(LIBRARY_DIR_NAME)
}
