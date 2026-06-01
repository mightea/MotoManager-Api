-- After 007 backfilled `title` from `description`, every previously
-- existing row has `description` duplicating `title`. Null those out
-- so only meaningful (non-redundant) descriptions remain.
UPDATE issues
   SET description = NULL
 WHERE description IS NOT NULL
   AND TRIM(description) = title;
