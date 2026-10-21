use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct AppConfig {
    /// Directory the user picked to store their notes in (as Markdown
    /// files). `None` until first-run setup completes. Deliberately kept
    /// outside the SQLite database, which may live inside a synced folder.
    pub notes_root: Option<PathBuf>,
}

fn config_path(app_config_dir: &Path) -> PathBuf {
    app_config_dir.join("config.json")
}

pub fn load(app_config_dir: &Path) -> AppConfig {
    let path = config_path(app_config_dir);
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return AppConfig::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save(app_config_dir: &Path, config: &AppConfig) -> std::io::Result<()> {
    std::fs::create_dir_all(app_config_dir)?;
    let raw = serde_json::to_string_pretty(config).expect("AppConfig always serializes");
    std::fs::write(config_path(app_config_dir), raw)
}
