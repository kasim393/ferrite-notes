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
pub fn portable_dir() -> Option<PathBuf> {
    let dir = exe_dir()?;
    dir.join("portable").is_file().then(|| dir.join("data"))
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
