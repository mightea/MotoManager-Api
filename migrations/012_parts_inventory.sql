-- Parts database / inventory.
-- modelSeries is a lookup table (global seed rows have userId NULL, user-custom
-- rows have userId set) — no sync columns, mirroring currencies.
-- parts / partStocks / partConsumptions / storageLocations are user data and
-- follow the 011 sync pattern (clientId idempotency key, updatedAt cursor,
-- deletedAt tombstone). Columns are camelCase to match the live schema.

CREATE TABLE modelSeries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    manufacturer TEXT NOT NULL DEFAULT 'BMW',
    userId INTEGER REFERENCES users(id) ON DELETE CASCADE,
    createdAt TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE UNIQUE INDEX idx_model_series_global ON modelSeries(manufacturer, name) WHERE userId IS NULL;
CREATE UNIQUE INDEX idx_model_series_user ON modelSeries(manufacturer, name, userId) WHERE userId IS NOT NULL;

INSERT INTO modelSeries (name) VALUES
    ('R 45'), ('R 65'), ('R 80'), ('R 80 GS'), ('R 100'), ('R 100 GS'),
    ('R 850 R'), ('R 1100 GS'), ('R 1100 R'), ('R 1100 RT'), ('R 1100 S'),
    ('R 1150 GS'), ('R 1150 R'), ('R 1150 RT'),
    ('R 1200 GS'), ('R 1200 R'), ('R 1200 RT'), ('R nineT'),
    ('K 75'), ('K 100'), ('K 1100'), ('K 1200'), ('K 1600'),
    ('F 650'), ('F 650 GS'), ('F 700 GS'), ('F 750 GS'), ('F 800 GS'),
    ('F 850 GS'), ('F 900 R'),
    ('G 310 GS'), ('G 310 R'),
    ('S 1000 R'), ('S 1000 RR'), ('S 1000 XR'),
    ('C 400 X'), ('C 650 GT');

CREATE TABLE storageLocations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    userId INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    parentId INTEGER REFERENCES storageLocations(id) ON DELETE SET NULL,
    createdAt TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    clientId TEXT,
    updatedAt TEXT,
    deletedAt TEXT
);

CREATE TABLE parts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    userId INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    partNumber TEXT NOT NULL,
    name TEXT NOT NULL,
    manufacturer TEXT NOT NULL DEFAULT 'BMW',
    description TEXT,
    isPublic INTEGER NOT NULL DEFAULT 0,
    createdAt TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    clientId TEXT,
    updatedAt TEXT,
    deletedAt TEXT
);
-- Identity = partNumber + name per user; partial so a tombstoned part can be recreated.
CREATE UNIQUE INDEX idx_parts_identity ON parts(userId, partNumber, name) WHERE deletedAt IS NULL;
CREATE INDEX idx_parts_public ON parts(isPublic) WHERE isPublic = 1;

-- Part <-> series fitment. Server-managed from the part's seriesIds payload;
-- fitment changes bump parts.updatedAt, so this table needs no sync columns.
CREATE TABLE partSeriesCompat (
    partId INTEGER NOT NULL REFERENCES parts(id) ON DELETE CASCADE,
    seriesId INTEGER NOT NULL REFERENCES modelSeries(id) ON DELETE CASCADE,
    PRIMARY KEY (partId, seriesId)
);

-- Purchase/stock entries. Ownership derives through parts.userId.
CREATE TABLE partStocks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    partId INTEGER NOT NULL REFERENCES parts(id) ON DELETE CASCADE,
    quantity INTEGER NOT NULL DEFAULT 1,
    price REAL,
    currency TEXT,
    normalizedPrice REAL,
    purchaseDate TEXT,
    storageLocationId INTEGER REFERENCES storageLocations(id) ON DELETE SET NULL,
    notes TEXT,
    createdAt TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    clientId TEXT,
    updatedAt TEXT,
    deletedAt TEXT
);

-- Consumption entries; maintenanceRecordId nullable to allow manual corrections.
-- On-hand is always derived: SUM(live stock qty) - SUM(live consumption qty).
CREATE TABLE partConsumptions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    partId INTEGER NOT NULL REFERENCES parts(id) ON DELETE CASCADE,
    maintenanceRecordId INTEGER REFERENCES maintenanceRecords(id) ON DELETE SET NULL,
    quantity INTEGER NOT NULL,
    date TEXT NOT NULL,
    notes TEXT,
    createdAt TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    clientId TEXT,
    updatedAt TEXT,
    deletedAt TEXT
);

-- Idempotency keys (011 pattern): a retried create with the same clientId must not duplicate.
CREATE UNIQUE INDEX idx_parts_client_id ON parts(clientId) WHERE clientId IS NOT NULL;
CREATE UNIQUE INDEX idx_part_stocks_client_id ON partStocks(clientId) WHERE clientId IS NOT NULL;
CREATE UNIQUE INDEX idx_part_cons_client_id ON partConsumptions(clientId) WHERE clientId IS NOT NULL;
CREATE UNIQUE INDEX idx_storage_loc_client_id ON storageLocations(clientId) WHERE clientId IS NOT NULL;

-- Delta-sync cursor indexes for ?since=<updatedAt> scans.
CREATE INDEX idx_parts_updated_at ON parts(userId, updatedAt);
CREATE INDEX idx_part_stocks_updated_at ON partStocks(partId, updatedAt);
CREATE INDEX idx_part_cons_updated_at ON partConsumptions(partId, updatedAt);
CREATE INDEX idx_storage_loc_updated_at ON storageLocations(userId, updatedAt);
CREATE INDEX idx_part_cons_maintenance ON partConsumptions(maintenanceRecordId);

-- Motorcycle -> series link; derives part<->motorcycle compatibility.
ALTER TABLE motorcycles ADD COLUMN seriesId INTEGER REFERENCES modelSeries(id);
