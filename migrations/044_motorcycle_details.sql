-- Free-form Title/Value details per motorcycle (e.g. spark plug brand/model).
-- Sync-enabled like torqueSpecs: clientId idempotency + updatedAt delta +
-- deletedAt tombstones (see migration 011).
CREATE TABLE motorcycleDetails (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    motorcycleId INTEGER NOT NULL REFERENCES motorcycles(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    value TEXT NOT NULL,
    createdAt TEXT NOT NULL,
    clientId TEXT,
    updatedAt TEXT,
    deletedAt TEXT
);

CREATE UNIQUE INDEX idx_motorcycle_details_client_id
    ON motorcycleDetails(clientId) WHERE clientId IS NOT NULL;
CREATE INDEX idx_motorcycle_details_updated_at
    ON motorcycleDetails(motorcycleId, updatedAt);
