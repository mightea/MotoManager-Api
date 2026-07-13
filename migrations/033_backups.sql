-- Backup run history, surfaced to admins in the webapp so a backup that silently
-- fails is visible instead of lost. One row per attempt (scheduled or manual).
-- `status`: 'running' while in-flight, then 'success' or 'failed'. A row left in
-- 'running' after a crash is reset to 'failed' on the next startup (see main.rs).
CREATE TABLE backups (
    id         INTEGER PRIMARY KEY,
    startedAt  TEXT    NOT NULL,
    finishedAt TEXT,
    status     TEXT    NOT NULL,   -- 'running' | 'success' | 'failed'
    trigger    TEXT    NOT NULL,   -- 'scheduled' | 'manual'
    sizeBytes  INTEGER,            -- archive size on success
    filePath   TEXT,               -- basename of the .tar.gz on success
    error      TEXT                -- failure message when status = 'failed'
);

-- History is queried newest-first and the scheduler looks up the last success.
CREATE INDEX idx_backups_startedAt ON backups (startedAt DESC);
