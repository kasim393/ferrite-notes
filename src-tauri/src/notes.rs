use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::util::now_ms;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: String,
    pub body: String,
    pub pinned: bool,
    pub deleted: bool,
    pub updated_at: i64,
}

/// What the note list shows. Never carries the full body.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteSummary {
    pub id: String,
    pub title: String,
    /// Plain text; for search results matches are wrapped in \u{2} … \u{3}.
    pub snippet: String,
    pub pinned: bool,
    pub deleted: bool,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ListFilter {
    Notes,
    Trash,
}

const TITLE_LEN: usize = 100;
const SNIPPET_LEN: usize = 80;
/// Enough leading text to build a title and snippet without loading whole notes.
const HEAD_CHARS: i64 = 400;

fn note_from_row(row: &Row) -> rusqlite::Result<Note> {
    Ok(Note {
        id: row.get(0)?,
        body: row.get(1)?,
        pinned: row.get(2)?,
        deleted: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

/// Columns: id, head, pinned, deleted, updated_at[, fts snippet].
fn summary_from_row(row: &Row, fts_snippet: bool) -> rusqlite::Result<NoteSummary> {
    let head: String = row.get(1)?;
    let (title, mut snippet) = title_and_snippet(&head);
    if fts_snippet {
        snippet = row
            .get::<_, String>(5)?
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
    }
    Ok(NoteSummary {
        id: row.get(0)?,
        title,
        snippet,
        pinned: row.get(2)?,
        deleted: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

/// First non-empty line is the title (leading `#` stripped); the rest is the snippet.
pub fn title_and_snippet(body: &str) -> (String, String) {
    let mut lines = body.lines().map(str::trim).filter(|l| !l.is_empty());
    let title = lines
        .next()
        .map(|l| l.trim_start_matches('#').trim())
        .unwrap_or_default();
    let rest = lines.collect::<Vec<_>>().join(" ");
    (truncate(title, TITLE_LEN), truncate(&rest, SNIPPET_LEN))
}

fn truncate(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((i, _)) => format!("{}…", &s[..i]),
        None => s.to_string(),
    }
}

pub fn list(conn: &Connection, filter: ListFilter) -> AppResult<Vec<NoteSummary>> {
    let deleted = matches!(filter, ListFilter::Trash);
    let mut stmt = conn.prepare_cached(
        "SELECT id, substr(body, 1, ?2), pinned, deleted, updated_at
         FROM notes WHERE deleted = ?1
         ORDER BY pinned DESC, updated_at DESC",
    )?;
    let rows = stmt.query_map(params![deleted, HEAD_CHARS], |r| summary_from_row(r, false))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn get(conn: &Connection, id: &str) -> AppResult<Note> {
    conn.prepare_cached(
        "SELECT id, body, pinned, deleted, updated_at FROM notes WHERE id = ?1 AND deleted < 2",
    )?
    .query_row([id], note_from_row)
    .optional()?
    .ok_or(AppError::NotFound)
}

pub fn create(conn: &Connection) -> AppResult<Note> {
    let note = Note {
        id: uuid::Uuid::now_v7().to_string(),
        body: String::new(),
        pinned: false,
        deleted: false,
        updated_at: now_ms(),
    };
    conn.prepare_cached("INSERT INTO notes (id, body, updated_at) VALUES (?1, '', ?2)")?
        .execute(params![note.id, note.updated_at])?;
    Ok(note)
}

/// Saves the body if it changed. Returns the note's `updated_at`.
/// `updated_at` never moves backwards, even if the clock does (sync relies on it).
pub fn save(conn: &Connection, id: &str, body: &str) -> AppResult<i64> {
    let updated: Option<i64> = conn
        .prepare_cached(
            "UPDATE notes SET body = ?2, updated_at = MAX(?3, updated_at + 1), dirty = 1
             WHERE id = ?1 AND deleted < 2 AND body <> ?2 RETURNING updated_at",
        )?
        .query_row(params![id, body, now_ms()], |r| r.get(0))
        .optional()?;
    match updated {
        Some(t) => Ok(t),
        None => conn
            .prepare_cached("SELECT updated_at FROM notes WHERE id = ?1 AND deleted < 2")?
            .query_row([id], |r| r.get(0))
            .optional()?
            .ok_or(AppError::NotFound),
    }
}

fn set_flag(conn: &Connection, id: &str, column: &str, value: bool) -> AppResult<()> {
    // `column` is always one of our own literals, never user input.
    let sql = format!(
        "UPDATE notes SET {column} = ?2, updated_at = MAX(?3, updated_at + 1), dirty = 1
         WHERE id = ?1 AND deleted < 2"
    );
    match conn
        .prepare_cached(&sql)?
        .execute(params![id, value, now_ms()])?
    {
        0 => Err(AppError::NotFound),
        _ => Ok(()),
    }
}

pub fn set_pinned(conn: &Connection, id: &str, pinned: bool) -> AppResult<()> {
    set_flag(conn, id, "pinned", pinned)
}

pub fn set_deleted(conn: &Connection, id: &str, deleted: bool) -> AppResult<()> {
    set_flag(conn, id, "deleted", deleted)
}

/// Permanently removes trashed notes. With sync on, each becomes a tombstone
/// (body cleared, `deleted = 2`) until the purge reaches the server.
pub fn purge_trash(conn: &Connection, sync_on: bool) -> AppResult<()> {
    purge_trashed_before(conn, i64::MAX, sync_on)
}

/// Removes notes that have sat in the trash longer than `max_age_ms`.
pub fn purge_old_trash(conn: &Connection, max_age_ms: i64, sync_on: bool) -> AppResult<()> {
    purge_trashed_before(conn, now_ms() - max_age_ms, sync_on)
}

fn purge_trashed_before(conn: &Connection, cutoff: i64, sync_on: bool) -> AppResult<()> {
    if sync_on {
        conn.execute(
            "UPDATE notes SET deleted = 2, body = '', dirty = 1, updated_at = MAX(?2, updated_at + 1)
             WHERE deleted = 1 AND updated_at < ?1",
            params![cutoff, now_ms()],
        )?;
    } else {
        conn.execute(
            "DELETE FROM notes WHERE deleted = 1 AND updated_at < ?1",
            [cutoff],
        )?;
    }
    Ok(())
}

/// Drops tombstones that no longer need to reach the server.
pub fn clean_up_purged(conn: &Connection, sync_on: bool) -> AppResult<()> {
    let filter = if sync_on {
        "deleted = 2 AND dirty = 0"
    } else {
        "deleted = 2"
    };
    conn.execute(&format!("DELETE FROM notes WHERE {filter}"), [])?;
    Ok(())
}

/// Removes a note that was never written in, so stray "new note" presses
/// don't leave empty notes behind. Returns true if it was removed.
pub fn discard_if_empty(conn: &Connection, id: &str, sync_on: bool) -> AppResult<bool> {
    let blank: Option<bool> = conn
        .query_row(
            "SELECT server_rev > 0 FROM notes
             WHERE id = ?1 AND deleted < 2 AND trim(body, ' ' || char(9, 10, 13)) = ''",
            [id],
            |r| r.get(0),
        )
        .optional()?;
    let Some(synced) = blank else {
        return Ok(false);
    };
    if sync_on && synced {
        // The server already has it: leave a tombstone so other devices drop it too.
        conn.execute(
            "UPDATE notes SET deleted = 2, body = '', dirty = 1, updated_at = MAX(?2, updated_at + 1)
             WHERE id = ?1",
            params![id, now_ms()],
        )?;
    } else {
        conn.execute("DELETE FROM notes WHERE id = ?1", [id])?;
    }
    Ok(true)
}

pub fn toggle_task(conn: &Connection, id: &str, line_no: usize) -> AppResult<Note> {
    let tx = conn.unchecked_transaction()?;
    let note = get(&tx, id)?;
    let body = toggle_line(&note.body, line_no).ok_or(AppError::NotATask)?;
    save(&tx, id, &body)?;
    let note = get(&tx, id)?;
    tx.commit()?;
    Ok(note)
}

pub fn search(conn: &Connection, query: &str) -> AppResult<Vec<NoteSummary>> {
    let fts = fts_query(query);
    if fts.is_empty() {
        return list(conn, ListFilter::Notes);
    }
    let mut stmt = conn.prepare_cached(
        "SELECT n.id, substr(n.body, 1, ?2), n.pinned, n.deleted, n.updated_at,
                snippet(notes_fts, 0, char(2), char(3), '…', 12)
         FROM notes_fts JOIN notes n ON n.rid = notes_fts.rowid
         WHERE notes_fts MATCH ?1 AND n.deleted = 0
         ORDER BY rank LIMIT 200",
    )?;
    let rows = stmt.query_map(params![fts, HEAD_CHARS], |r| summary_from_row(r, true))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Turns free text into a safe FTS5 query: every word becomes a quoted prefix term.
pub fn fts_query(input: &str) -> String {
    input
        .split_whitespace()
        .filter(|w| w.chars().any(char::is_alphanumeric))
        .map(|w| format!("\"{}\"*", w.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" ")
}

/// If `line` is a markdown task (`- [ ]`, `* [x]`, `1. [ ]` …), returns the byte
/// index of the character between the brackets and whether it is checked.
pub fn task_marker(line: &str) -> Option<(usize, bool)> {
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();
    let after_bullet = match trimmed.strip_prefix(['-', '*', '+']) {
        Some(rest) => rest,
        None => {
            let digits = trimmed.bytes().take_while(u8::is_ascii_digit).count();
            if digits == 0 {
                return None;
            }
            trimmed[digits..].strip_prefix(['.', ')'])?
        }
    };
    let bullet_len = trimmed.len() - after_bullet.len();
    let b = after_bullet.strip_prefix(' ')?.as_bytes();
    if b.len() < 3 || b[0] != b'[' || b[2] != b']' {
        return None;
    }
    let checked = match b[1] {
        b' ' => false,
        b'x' | b'X' => true,
        _ => return None,
    };
    if b.len() > 3 && !b[3].is_ascii_whitespace() {
        return None;
    }
    Some((indent + bullet_len + 2, checked))
}

/// Flips `[ ]` ↔ `[x]` on 0-based line `line_no`. `None` if that line isn't a task.
pub fn toggle_line(body: &str, line_no: usize) -> Option<String> {
    let mut out = String::with_capacity(body.len());
    let mut found = false;
    for (i, line) in body.split_inclusive('\n').enumerate() {
        if i == line_no {
            let (at, checked) = task_marker(line)?;
            out.push_str(&line[..at]);
            out.push(if checked { ' ' } else { 'x' });
            out.push_str(&line[at + 1..]);
            found = true;
        } else {
            out.push_str(line);
        }
    }
    found.then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    #[test]
    fn toggles_tasks() {
        let body = "Bugs\n- [x] Fix tax\n- [ ] Safari\r\n  - [ ] nested\n1. [ ] numbered";
        assert_eq!(
            toggle_line(body, 1).unwrap(),
            "Bugs\n- [ ] Fix tax\n- [ ] Safari\r\n  - [ ] nested\n1. [ ] numbered"
        );
        assert_eq!(
            toggle_line(body, 2).unwrap(),
            "Bugs\n- [x] Fix tax\n- [x] Safari\r\n  - [ ] nested\n1. [ ] numbered"
        );
        assert!(toggle_line(body, 3).unwrap().contains("  - [x] nested"));
        assert!(toggle_line(body, 4).unwrap().ends_with("1. [x] numbered"));
        assert!(toggle_line(body, 0).is_none());
        assert!(toggle_line(body, 99).is_none());
    }

    #[test]
    fn rejects_non_tasks() {
        assert!(task_marker("- [ ]x").is_none());
        assert!(task_marker("-[ ] a").is_none());
        assert!(task_marker("- [y] a").is_none());
        assert!(task_marker("[ ] a").is_none());
        assert_eq!(task_marker("- [ ]"), Some((3, false)));
        assert_eq!(task_marker("* [X] done"), Some((3, true)));
    }

    #[test]
    fn titles_and_snippets() {
        assert_eq!(
            title_and_snippet("\n\n# Hello  \nworld\n\nmore"),
            ("Hello".into(), "world more".into())
        );
        assert_eq!(title_and_snippet(""), (String::new(), String::new()));
        let long = "x".repeat(150);
        assert_eq!(title_and_snippet(&long).0.chars().count(), TITLE_LEN + 1);
    }

    #[test]
    fn fts_queries_are_quoted() {
        assert_eq!(fts_query("foo  ba\"r"), "\"foo\"* \"ba\"\"r\"*");
        assert_eq!(fts_query(" - * "), "");
    }

    #[test]
    fn crud_search_and_trash() {
        let conn = open_in_memory();
        let a = create(&conn).unwrap();
        let b = create(&conn).unwrap();
        save(&conn, &a.id, "Groceries\n- [ ] milk\n- [ ] bread").unwrap();
        save(&conn, &b.id, "Bugs\n- [ ] Safari date picker").unwrap();

        let hits = search(&conn, "saf").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "Bugs");
        assert!(hits[0].snippet.contains('\u{2}'));

        let note = toggle_task(&conn, &a.id, 2).unwrap();
        assert_eq!(note.body, "Groceries\n- [ ] milk\n- [x] bread");
        assert!(matches!(
            toggle_task(&conn, &a.id, 0),
            Err(AppError::NotATask)
        ));

        set_pinned(&conn, &b.id, true).unwrap();
        assert_eq!(list(&conn, ListFilter::Notes).unwrap()[0].id, b.id);

        set_deleted(&conn, &b.id, true).unwrap();
        assert!(search(&conn, "safari").unwrap().is_empty());
        assert_eq!(list(&conn, ListFilter::Trash).unwrap().len(), 1);
        purge_trash(&conn, false).unwrap();
        assert!(list(&conn, ListFilter::Trash).unwrap().is_empty());
        assert!(search(&conn, "safari").unwrap().is_empty());
    }

    #[test]
    fn save_keeps_time_monotonic_and_skips_noop() {
        let conn = open_in_memory();
        let n = create(&conn).unwrap();
        let t1 = save(&conn, &n.id, "a").unwrap();
        let t2 = save(&conn, &n.id, "a").unwrap();
        assert_eq!(t1, t2);
        let t3 = save(&conn, &n.id, "b").unwrap();
        assert!(t3 > t1);
        assert!(matches!(save(&conn, "nope", "x"), Err(AppError::NotFound)));
    }

    #[test]
    fn discards_only_empty_notes() {
        let conn = open_in_memory();
        let empty = create(&conn).unwrap();
        save(&conn, &empty.id, " \n\t").unwrap();
        let full = create(&conn).unwrap();
        save(&conn, &full.id, "keep").unwrap();
        assert!(discard_if_empty(&conn, &empty.id, false).unwrap());
        assert!(!discard_if_empty(&conn, &full.id, false).unwrap());
    }
}
