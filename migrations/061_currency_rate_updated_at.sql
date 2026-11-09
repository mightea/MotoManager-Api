-- When a currency's conversionFactor was last fetched or edited, so the admin
-- page can show how old the rates are and the server can refresh them daily.
-- Until now a rate was only ever set when the currency was created (or edited
-- by hand), so createdAt is the best available backfill. Normalised to
-- RFC 3339 UTC like the timestamps the API writes; createdAt values written by
-- SQLite's CURRENT_TIMESTAMP ("YYYY-MM-DD HH:MM:SS") are UTC already.
ALTER TABLE currencies ADD COLUMN rateUpdatedAt TEXT;

UPDATE currencies
SET rateUpdatedAt = COALESCE(strftime('%Y-%m-%dT%H:%M:%SZ', createdAt), createdAt);
