-- Server `rev` of the version this note's local edits are based on (0 = never synced).
ALTER TABLE notes ADD COLUMN server_rev INTEGER NOT NULL DEFAULT 0;
-- `deleted` now also uses 2 = purged: a tombstone (body cleared) kept until the
-- purge has been pushed to the sync server, then removed.
