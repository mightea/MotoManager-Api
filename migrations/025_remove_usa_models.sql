-- Remove USA-market catalog models from the seed — this garage is ECE-only.
-- Guarded: anything referenced by parts or motorcycles (or with children)
-- survives. Serie-level type-code lists keep the US codes, so a US-market
-- VIN still resolves to the surrounding Serie/ECE Modell.
DELETE FROM modelSeries
WHERE userId IS NULL
  AND name LIKE '%(USA,%'
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat)
  AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL)
  AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);
