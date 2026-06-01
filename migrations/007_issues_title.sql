-- Issues: introduce a mandatory `title` column and demote `description`
-- to optional. Existing rows have only `description` populated; copy it
-- across so no information is lost. Rows whose description is NULL or
-- blank fall back to a generic placeholder so the NOT NULL constraint
-- is satisfied — users can rename them later.
--
-- SQLite cannot add a NOT NULL column with a non-constant default and
-- cannot tighten a column to NOT NULL via ALTER, so the canonical
-- pattern is a table-swap.

-- 1. Stage the new column and backfill from description.
ALTER TABLE issues ADD COLUMN title TEXT;
UPDATE issues
   SET title = COALESCE(NULLIF(TRIM(description), ''), 'Mangel');

-- 2. Build the new table with the tightened schema.
CREATE TABLE issues_new (
    id           INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    motorcycleId INTEGER NOT NULL REFERENCES motorcycles(id) ON DELETE NO ACTION,
    odo          INTEGER NOT NULL,
    title        TEXT    NOT NULL,
    description  TEXT,
    priority     TEXT    NOT NULL DEFAULT 'medium',
    status       TEXT    NOT NULL DEFAULT 'new',
    date         TEXT    DEFAULT (CURRENT_DATE)
);

-- 3. Move the data over.
INSERT INTO issues_new (id, motorcycleId, odo, title, description, priority, status, date)
SELECT id, motorcycleId, odo, title, description, priority, status, date
  FROM issues;

-- 4. Swap.
DROP TABLE issues;
ALTER TABLE issues_new RENAME TO issues;
