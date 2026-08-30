//! Data/config directory resolution (~/.local/share/lantern-mix, ~/.config/lantern-mix).

use std::path::PathBuf;

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

/// `$XDG_CONFIG_HOME/lantern-mix` or `~/.config/lantern-mix`.
pub fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home().join(".config"))
        .join("lantern-mix")
}

/// `$XDG_DATA_HOME/lantern-mix` or `~/.local/share/lantern-mix`.
pub fn data_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home().join(".local/share"))
        .join("lantern-mix")
}
