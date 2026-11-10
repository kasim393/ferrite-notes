use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::sync::SyncConfig;

pub const LOOKS: &[&str] = &["macos", "gaming", "terminal", "retro", "eink"];
pub const DEFAULT_LOOK: &str = "macos";

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Light/dark mode: "system" | "light" | "dark".
    pub theme: String,
    /// The visual theme: one of `LOOKS`.
    pub look: String,
    pub sync_url: Option<String>,
    pub sync_token: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            look: DEFAULT_LOOK.into(),
            sync_url: None,
            sync_token: None,
        }
    }
}

// Hand-written so the token can never end up in a log.
impl std::fmt::Debug for Settings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Settings")
            .field("theme", &self.theme)
            .field("look", &self.look)
            .field("sync_url", &self.sync_url)
            .field(
                "sync_token",
                &self.sync_token.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

impl Settings {
    /// Trims fields, turns blanks into `None`, and checks the sync URL.
    pub fn normalized(mut self) -> AppResult<Self> {
        // Unknown values (an older or newer app wrote them) fall back to the defaults.
        if !["system", "light", "dark"].contains(&self.theme.as_str()) {
            self.theme = "system".into();
        }
        if !LOOKS.contains(&self.look.as_str()) {
            self.look = DEFAULT_LOOK.into();
        }
        let clean = |v: Option<String>| v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        self.sync_token = clean(self.sync_token);
        self.sync_url = clean(self.sync_url).map(|u| {
            let u = u.trim_end_matches('/');
            u.strip_suffix("/sync").unwrap_or(u).to_string()
        });
        if let Some(url) = &self.sync_url {
            let local = ["http://localhost", "http://127.0.0.1"]
                .iter()
                .any(|p| url.starts_with(p));
            if !url.starts_with("https://") && !local {
                return Err(AppError::Other(
                    "The sync URL must start with https://".into(),
                ));
            }
        }
        Ok(self)
    }

    /// `Some` when both the URL and token are set.
    pub fn sync_config(&self) -> Option<SyncConfig> {
        Some(SyncConfig {
            endpoint: format!("{}/sync", self.sync_url.as_ref()?),
            token: self.sync_token.clone()?,
        })
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
