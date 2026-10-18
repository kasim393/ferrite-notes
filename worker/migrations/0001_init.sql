CREATE TABLE notes (
  id          TEXT PRIMARY KEY,
  body        TEXT NOT NULL,
  pinned      INTEGER NOT NULL,
  deleted     INTEGER NOT NULL,   -- 0 = live, 1 = in trash, 2 = purged (tombstone, body cleared)
  updated_at  INTEGER NOT NULL,   -- client edit time (conflict resolution)
  rev         INTEGER NOT NULL    -- server sequence number (sync cursor)
);
CREATE INDEX idx_rev ON notes(rev);

CREATE TABLE counter (id INTEGER PRIMARY KEY CHECK (id = 1), rev INTEGER NOT NULL);
INSERT INTO counter (id, rev) VALUES (1, 0);
