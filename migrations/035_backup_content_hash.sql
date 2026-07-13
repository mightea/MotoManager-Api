-- Content fingerprint of a backup (SHA-256 of the VACUUM INTO snapshot plus a
-- stat-listing of the upload dirs). Lets a scheduled run detect that nothing has
-- changed since the last successful backup and record a lightweight 'skipped'
-- row instead of writing an identical archive.
ALTER TABLE backups ADD COLUMN contentHash TEXT;
