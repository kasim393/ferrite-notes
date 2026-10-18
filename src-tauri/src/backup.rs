use std::fs;
use std::path::Path;

use rusqlite::Connection;

use crate::error::AppResult;
use crate::util::today_utc;

const KEEP: usize = 7;

/// Writes `backups/notes-YYYY-MM-DD.db` once per day and keeps the last 7.
pub fn run_daily(db_path: &Path, data_dir: &Path) -> AppResult<()> {
    let dir = data_dir.join("backups");
    fs::create_dir_all(&dir)?;

    let target = dir.join(format!("notes-{}.db", today_utc()));
    if !target.exists() {
        let conn = Connection::open(db_path)?;
        conn.execute("VACUUM INTO ?1", [target.to_string_lossy()])?;
    }

    let mut backups: Vec<_> = fs::read_dir(&dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("notes-") && n.ends_with(".db"))
        })
        .collect();
    backups.sort();
    let excess = backups.len().saturating_sub(KEEP);
    for old in &backups[..excess] {
        fs::remove_file(old)?;
    }
    Ok(())
}
