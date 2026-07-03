-- Sync metadata for offline-first iOS client.
-- Adds a client-generated idempotency key (clientId), a server-authoritative
-- updatedAt cursor for ?since delta sync, and a deletedAt tombstone for
-- soft-delete. Columns are camelCase to match the live (post-002) schema.

-- maintenanceRecords
ALTER TABLE maintenanceRecords ADD COLUMN clientId TEXT;
ALTER TABLE maintenanceRecords ADD COLUMN updatedAt TEXT;
ALTER TABLE maintenanceRecords ADD COLUMN deletedAt TEXT;
UPDATE maintenanceRecords SET updatedAt = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE updatedAt IS NULL;

-- torqueSpecs
ALTER TABLE torqueSpecs ADD COLUMN clientId TEXT;
ALTER TABLE torqueSpecs ADD COLUMN updatedAt TEXT;
ALTER TABLE torqueSpecs ADD COLUMN deletedAt TEXT;
UPDATE torqueSpecs SET updatedAt = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE updatedAt IS NULL;

-- issues
ALTER TABLE issues ADD COLUMN clientId TEXT;
ALTER TABLE issues ADD COLUMN updatedAt TEXT;
ALTER TABLE issues ADD COLUMN deletedAt TEXT;
UPDATE issues SET updatedAt = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE updatedAt IS NULL;

-- Idempotency keys: a retried create with the same clientId must not duplicate.
-- Partial indexes so legacy rows with NULL clientId don't collide.
CREATE UNIQUE INDEX IF NOT EXISTS idx_maintenance_client_id ON maintenanceRecords(clientId) WHERE clientId IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_torque_client_id ON torqueSpecs(clientId) WHERE clientId IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_issues_client_id ON issues(clientId) WHERE clientId IS NOT NULL;

-- Delta-sync cursor indexes for ?since=<updatedAt> scans per motorcycle.
CREATE INDEX IF NOT EXISTS idx_maintenance_updated_at ON maintenanceRecords(motorcycleId, updatedAt);
CREATE INDEX IF NOT EXISTS idx_torque_updated_at ON torqueSpecs(motorcycleId, updatedAt);
CREATE INDEX IF NOT EXISTS idx_issues_updated_at ON issues(motorcycleId, updatedAt);
