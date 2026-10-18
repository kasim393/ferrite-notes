-- `rid` is an explicit INTEGER PRIMARY KEY so FTS5 rowids stay stable
-- (implicit rowids may be renumbered by VACUUM).
CREATE TABLE notes (
  rid         INTEGER PRIMARY KEY,
  id          TEXT NOT NULL UNIQUE,           -- UUID v7 (time-sortable)
  body        TEXT NOT NULL DEFAULT '',
  pinned      INTEGER NOT NULL DEFAULT 0,
  deleted     INTEGER NOT NULL DEFAULT 0,     -- soft delete (needed for sync)
  updated_at  INTEGER NOT NULL,               -- ms epoch, set on every edit
  dirty       INTEGER NOT NULL DEFAULT 1      -- 1 = not yet pushed to server
);
CREATE INDEX idx_notes_list ON notes(deleted, pinned DESC, updated_at DESC);

CREATE VIRTUAL TABLE notes_fts USING fts5(
  body, content='notes', content_rowid='rid', tokenize='unicode61'
);

CREATE TRIGGER notes_ai AFTER INSERT ON notes BEGIN
  INSERT INTO notes_fts(rowid, body) VALUES (new.rid, new.body);
END;
CREATE TRIGGER notes_ad AFTER DELETE ON notes BEGIN
  INSERT INTO notes_fts(notes_fts, rowid, body) VALUES ('delete', old.rid, old.body);
END;
CREATE TRIGGER notes_au AFTER UPDATE OF body ON notes BEGIN
  INSERT INTO notes_fts(notes_fts, rowid, body) VALUES ('delete', old.rid, old.body);
  INSERT INTO notes_fts(rowid, body) VALUES (new.rid, new.body);
END;

CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT);
