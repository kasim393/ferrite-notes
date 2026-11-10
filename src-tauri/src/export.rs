//! "Export all notes": a zip of plain `.md` files, readable in any text
//! editor without this app. Your data is never locked in.

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{BufWriter, Cursor, Seek, Write};
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipWriter};

use crate::error::{AppError, AppResult};
use crate::notes::title_and_snippet;
use crate::util::{now_ms, today_utc, utc_parts};

const MAX_NAME_CHARS: usize = 80;

/// `ferrite-notes-YYYY-MM-DD`, the export's name without `.zip`.
pub fn export_stem() -> String {
    format!("ferrite-notes-{}", today_utc())
}

/// Writes `ferrite-notes-YYYY-MM-DD.zip` into `dir` and returns its path.
/// Live notes go at the top level, trashed ones under `Trash/`.
pub fn export_zip(conn: &Connection, dir: &Path) -> AppResult<PathBuf> {
    fs::create_dir_all(dir)?;
    let target = unused_path(dir, &export_stem(), "zip");
    let tmp = target.with_extension("zip.part");

    let result = File::create(&tmp)
        .map_err(AppError::from)
        .and_then(|file| write_zip(conn, BufWriter::new(file)).map(|_| ()));
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result?;
    fs::rename(&tmp, &target)?;
    Ok(target)
}

/// The same zip, in memory, for targets that aren't plain paths (Android's
/// `content://` locations from the "Save as" picker).
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn export_bytes(conn: &Connection) -> AppResult<Vec<u8>> {
    Ok(write_zip(conn, Cursor::new(Vec::new()))?.into_inner())
}

fn write_zip<W: Write + Seek>(conn: &Connection, out: W) -> AppResult<W> {
    let mut zip = ZipWriter::new(out);
    let mut used = HashSet::new();

    let mut stmt = conn.prepare(
        "SELECT body, deleted, updated_at FROM notes WHERE deleted < 2
         ORDER BY deleted, pinned DESC, updated_at DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, bool>(1)?, r.get::<_, i64>(2)?))
    })?;
    for row in rows {
        let (body, trashed, updated_at) = row?;
        let folder = if trashed { "Trash/" } else { "" };
        let name = unique_name(&mut used, folder, &file_stem(&title_and_snippet(&body).0));
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .last_modified_time(zip_time(updated_at));
        zip.start_file(name, options).map_err(zip_error)?;
        zip.write_all(body.as_bytes())?;
    }
    let mut out = zip.finish().map_err(zip_error)?;
    out.flush()?;
    Ok(out)
}

fn zip_error(e: zip::result::ZipError) -> AppError {
    AppError::Other(format!("Could not write the export: {e}"))
}

fn zip_time(ms: i64) -> DateTime {
    let (y, mo, d, h, mi, s) = utc_parts(if ms > 0 { ms } else { now_ms() });
    // Zip can't store years before 1980; fall back to its default time then.
    DateTime::from_date_and_time(y as u16, mo as u8, d as u8, h as u8, mi as u8, s as u8)
        .unwrap_or_default()
}

/// A note title made safe as a file name on Windows, macOS, and Linux.
pub fn file_stem(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| if c.is_control() || r#"<>:"/\|?*"#.contains(c) { '_' } else { c })
        .take(MAX_NAME_CHARS)
        .collect();
    let trimmed = cleaned.trim().trim_end_matches(['.', ' ']).trim_start_matches('.').trim();
    if trimmed.is_empty() {
        return "Untitled".into();
    }
    let reserved = ["CON", "PRN", "AUX", "NUL"].contains(&trimmed.to_ascii_uppercase().as_str())
        || (trimmed.len() == 4
            && ["COM", "LPT"].contains(&trimmed[..3].to_ascii_uppercase().as_str())
            && trimmed.as_bytes()[3].is_ascii_digit());
    if reserved {
        format!("_{trimmed}")
    } else {
        trimmed.to_string()
    }
}

/// `folder/stem.md`, or `folder/stem (2).md` … if that name is taken (case-insensitively).
fn unique_name(used: &mut HashSet<String>, folder: &str, stem: &str) -> String {
    let mut n = 1;
    loop {
        let name = match n {
            1 => format!("{folder}{stem}.md"),
            _ => format!("{folder}{stem} ({n}).md"),
        };
        if used.insert(name.to_lowercase()) {
            return name;
        }
        n += 1;
    }
}

fn unused_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let mut n = 1;
    loop {
        let path = match n {
            1 => dir.join(format!("{stem}.{ext}")),
            _ => dir.join(format!("{stem}-{n}.{ext}")),
        };
        if !path.exists() {
            return path;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;
    use crate::notes;
    use std::io::Read;

    #[test]
    fn file_names_are_safe() {
        assert_eq!(file_stem("Bugs: billing / tax?"), "Bugs_ billing _ tax_");
        assert_eq!(file_stem("  ...  "), "Untitled");
        assert_eq!(file_stem("con"), "_con");
        assert_eq!(file_stem("COM1"), "_COM1");
        assert_eq!(file_stem("Notes."), "Notes");
        assert_eq!(file_stem(&"x".repeat(200)).chars().count(), MAX_NAME_CHARS);
    }

    #[test]
    fn exports_every_note_as_markdown() {
        let conn = open_in_memory();
        for body in ["Groceries\n- [ ] milk", "Groceries\nsecond", "", "Old idea"] {
            let n = notes::create(&conn).unwrap();
            notes::save(&conn, &n.id, body).unwrap();
            if body == "Old idea" {
                notes::set_deleted(&conn, &n.id, true).unwrap();
            }
        }
        let dir = std::env::temp_dir().join(format!("ferrite-export-test-{}", uuid::Uuid::now_v7()));
        let path = export_zip(&conn, &dir).unwrap();

        let mut zip = zip::ZipArchive::new(File::open(&path).unwrap()).unwrap();
        let mut names: Vec<String> = (0..zip.len())
            .map(|i| zip.by_index(i).unwrap().name().unwrap().to_string())
            .collect();
        names.sort();
        assert_eq!(names, ["Groceries (2).md", "Groceries.md", "Trash/Old idea.md", "Untitled.md"]);

        let mut text = String::new();
        zip.by_name("Trash/Old idea.md").unwrap().read_to_string(&mut text).unwrap();
        assert_eq!(text, "Old idea");
        fs::remove_dir_all(dir).unwrap();
    }
}
