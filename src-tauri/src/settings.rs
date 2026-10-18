use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::AppResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// "system" | "light" | "dark"
    pub theme: String,
    pub sync_url: Option<String>,
    pub sync_token: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self { theme: "system".into(), sync_url: None, sync_token: None }
    }
}

pub fn load(path: &Path) -> Settings {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Write to a temp file then rename, so a crash never leaves half a file.
pub fn save(path: &Path, settings: &Settings) -> AppResult<()> {
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(settings)?)?;
    fs::rename(&tmp, path)?;
    Ok(())
}
