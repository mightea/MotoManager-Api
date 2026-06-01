-- Locations: add type, coordinates, timestamps.
-- The closed type taxonomy is enforced in code via the LocationType Rust enum.
--
-- Note on defaults: SQLite forbids non-constant DEFAULTs (e.g. CURRENT_TIMESTAMP)
-- when adding columns to an existing table, so timestamps are added as nullable
-- and then backfilled. Handlers explicitly provide values for new rows.

ALTER TABLE locations ADD COLUMN type TEXT NOT NULL DEFAULT 'other';
ALTER TABLE locations ADD COLUMN latitude REAL;
ALTER TABLE locations ADD COLUMN longitude REAL;
ALTER TABLE locations ADD COLUMN createdAt TEXT;
ALTER TABLE locations ADD COLUMN updatedAt TEXT;

UPDATE locations
SET createdAt = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE createdAt IS NULL;

CREATE INDEX IF NOT EXISTS idx_locations_userId_type ON locations(userId, type);

-- Backfill: create one Location per (user, free-text name) found in maintenanceRecords
-- where no locationId is set yet. Type defaults to 'maintenance_shop' since the data
-- originated from maintenance entries.
INSERT INTO locations (userId, name, type, latitude, longitude, countryCode, createdAt)
SELECT DISTINCT
    m.userId,
    COALESCE(NULLIF(TRIM(mr.locationName), ''),
             NULLIF(TRIM(mr.inspectionLocation), ''),
             'Unknown'),
    'maintenance_shop',
    mr.latitude,
    mr.longitude,
    'CH',
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM maintenanceRecords mr
JOIN motorcycles m ON mr.motorcycleId = m.id
WHERE mr.locationId IS NULL
  AND (mr.locationName IS NOT NULL
       OR mr.inspectionLocation IS NOT NULL
       OR mr.latitude IS NOT NULL)
  AND NOT EXISTS (
      SELECT 1 FROM locations l
      WHERE l.userId = m.userId
        AND l.type = 'maintenance_shop'
        AND l.name = COALESCE(NULLIF(TRIM(mr.locationName), ''),
                              NULLIF(TRIM(mr.inspectionLocation), ''),
                              'Unknown')
  );

-- Backfill maintenanceRecords.locationId by matching name + user.
UPDATE maintenanceRecords
SET locationId = (
    SELECT l.id
    FROM locations l
    JOIN motorcycles m ON m.id = maintenanceRecords.motorcycleId
    WHERE l.userId = m.userId
      AND l.type = 'maintenance_shop'
      AND l.name = COALESCE(NULLIF(TRIM(maintenanceRecords.locationName), ''),
                            NULLIF(TRIM(maintenanceRecords.inspectionLocation), ''),
                            'Unknown')
    LIMIT 1
)
WHERE locationId IS NULL
  AND (locationName IS NOT NULL
       OR inspectionLocation IS NOT NULL
       OR latitude IS NOT NULL);

-- Drop dead columns: location is now a first-class typed entity referenced by locationId.
ALTER TABLE maintenanceRecords DROP COLUMN inspectionLocation;
ALTER TABLE maintenanceRecords DROP COLUMN locationName;
ALTER TABLE maintenanceRecords DROP COLUMN latitude;
ALTER TABLE maintenanceRecords DROP COLUMN longitude;
