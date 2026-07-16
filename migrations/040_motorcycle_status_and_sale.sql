-- Lifecycle status for a motorcycle plus optional sale details.
--   status: 'active' | 'archived' | 'sold'   (source of truth)
-- The legacy `isArchived` column is kept and derived from status
-- (archived/sold => 1) so existing clients (iOS reads it via SELECT *) keep
-- working; it will be dropped once every client adopts `status`.
ALTER TABLE motorcycles ADD COLUMN status TEXT NOT NULL DEFAULT 'active';
UPDATE motorcycles SET status = 'archived' WHERE isArchived = 1;

-- Sale details, mirroring the purchase-price columns (price + normalized mirror
-- + currency). buyerName is a free-text buyer; a structured cross-user transfer
-- is a separate future feature.
ALTER TABLE motorcycles ADD COLUMN soldDate TEXT;
ALTER TABLE motorcycles ADD COLUMN salePrice REAL;
ALTER TABLE motorcycles ADD COLUMN normalizedSalePrice REAL;
ALTER TABLE motorcycles ADD COLUMN saleCurrencyCode TEXT;
ALTER TABLE motorcycles ADD COLUMN buyerName TEXT;
