-- Re-derive tripDistance and fuelConsumption for every live fuel record from
-- the odometer gap to the previous live fill of the same motorcycle.
-- Before, only the record being written was computed (against whatever fill
-- preceded it at that moment, including soft-deleted ones), so later fills
-- went stale when an earlier fill was backfilled, corrected or deleted.
-- Changed rows get a fresh updatedAt so offline clients pull the fix.
WITH chain AS (
    SELECT
        id,
        fuelAmount,
        odo - LAG(odo) OVER (PARTITION BY motorcycleId ORDER BY odo, date, id) AS gap
    FROM maintenanceRecords
    WHERE type = 'fuel' AND deletedAt IS NULL
),
derived AS (
    SELECT
        id,
        CASE WHEN gap > 0 THEN CAST(gap AS REAL) END AS trip,
        CASE WHEN gap > 0 AND fuelAmount > 0 THEN fuelAmount * 100.0 / gap END AS consumption
    FROM chain
)
UPDATE maintenanceRecords
SET tripDistance = d.trip,
    fuelConsumption = d.consumption,
    updatedAt = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM derived d
WHERE maintenanceRecords.id = d.id
  AND (
      maintenanceRecords.tripDistance IS NOT d.trip
      OR (maintenanceRecords.fuelConsumption IS NULL) <> (d.consumption IS NULL)
      OR abs(maintenanceRecords.fuelConsumption - d.consumption) > 1e-9
  );
