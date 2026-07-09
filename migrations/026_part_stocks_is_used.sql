-- Used/salvaged flag on physical stock entries (e.g. a part pulled from a
-- donor motorcycle). Follows the isPublic boolean convention from 012.
ALTER TABLE partStocks ADD COLUMN isUsed INTEGER NOT NULL DEFAULT 0;
