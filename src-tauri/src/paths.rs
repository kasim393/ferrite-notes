use std::fs;
use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};

/// Folder the app was launched from. For an AppImage this is the folder
/// holding the `.AppImage` file, not its temporary mount point.
fn exe_dir() -> Option<PathBuf> {
    if let Some(appimage) = std::env::var_os("APPIMAGE") {
        return Path::new(&appimage).parent().map(Path::to_path_buf);
    }
    std::env::current_exe()
        .ok()?
        .parent()
        .map(Path::to_path_buf)
}

/// `Some(<exe_dir>/data)` when a `portable` marker file sits next to the app.
/// `portable.txt` counts too: Windows hides extensions, so a marker made with
/// "New → Text Document" is easy to get wrong.
pub fn portable_dir() -> Option<PathBuf> {
    let dir = exe_dir()?;
    ["portable", "portable.txt"]
        .iter()
        .any(|marker| dir.join(marker).is_file())
        .then(|| dir.join("data"))
}

/// Where exports go: next to the app in portable mode, else the Downloads folder.
pub fn export_dir(app: &AppHandle, data_dir: &Path) -> PathBuf {
    if portable_dir().is_some() {
        if let Some(dir) = exe_dir() {
            return dir;
        }
    }
    app.path().download_dir().unwrap_or_else(|_| data_dir.to_path_buf())
}

pub fn data_dir(app: &AppHandle) -> AppResult<PathBuf> {
    let dir = match portable_dir() {
        Some(dir) => dir,
        None => app
            .path()
            .app_data_dir()
            .map_err(|e| AppError::Other(format!("Cannot find app data folder: {e}")))?,
    };
    fs::create_dir_all(&dir)?;
    Ok(dir)
}
