use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension};

use crate::error::{AppError, AppResult};

/// Schema migrations; `PRAGMA user_version` = number applied.
const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/001_init.sql"),
    include_str!("../migrations/002_sync.sql"),
];

pub fn open(path: &Path) -> AppResult<Connection> {
    let conn = Connection::open(path)?;
    configure(&conn)?;
    migrate(&conn)?;
    ensure_device_id(&conn)?;
    Ok(conn)
}

fn configure(conn: &Connection) -> AppResult<()> {
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;",
    )?;
    Ok(())
}

pub fn migrate(conn: &Connection) -> AppResult<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if current > MIGRATIONS.len() as i64 {
        return Err(AppError::NewerSchema(current));
    }
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        tx.commit()?;
    }
    Ok(())
}

fn ensure_device_id(conn: &Connection) -> AppResult<()> {
    let existing: Option<String> = conn
        .query_row("SELECT value FROM meta WHERE key = 'device_id'", [], |r| {
            r.get(0)
        })
        .optional()?;
    if existing.is_none() {
        conn.execute(
            "INSERT INTO meta (key, value) VALUES ('device_id', ?1)",
            [uuid::Uuid::now_v7().to_string()],
        )?;
    }
    Ok(())
}

#[cfg(test)]
pub fn open_in_memory() -> Connection {
    let conn = Connection::open_in_memory().expect("open memory db");
    migrate(&conn).expect("migrate");
    ensure_device_id(&conn).expect("device id");
    conn
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_are_idempotent() {
        let conn = open_in_memory();
        migrate(&conn).unwrap();
        let v: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, MIGRATIONS.len() as i64);
    }

    /// Guards the trimmed SQLite build (src-tauri/.cargo/config.toml).
    #[test]
    fn sqlite_build_keeps_fts5_and_drops_unused_features() {
        let conn = open_in_memory();
        let options: Vec<String> = conn
            .prepare("PRAGMA compile_options")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(options.iter().any(|o| o == "ENABLE_FTS5"), "{options:?}");
        for dropped in ["ENABLE_FTS3", "ENABLE_RTREE", "SOUNDEX"] {
            assert!(!options.iter().any(|o| o == dropped), "{dropped} still on: {options:?}");
        }
        assert!(options.iter().any(|o| o == "OMIT_JSON"), "{options:?}");
    }

    #[test]
    fn refuses_newer_schema() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", 999).unwrap();
        assert!(matches!(migrate(&conn), Err(AppError::NewerSchema(999))));
    }
}
