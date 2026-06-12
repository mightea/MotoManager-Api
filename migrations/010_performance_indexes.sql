-- Indexes for per-motorcycle lookups used by the detail, list, and document endpoints.
CREATE INDEX IF NOT EXISTS idx_maintenanceRecords_motorcycleId ON maintenanceRecords(motorcycleId);
CREATE INDEX IF NOT EXISTS idx_maintenanceRecords_parentId ON maintenanceRecords(parentId);
CREATE INDEX IF NOT EXISTS idx_issues_motorcycleId ON issues(motorcycleId);
CREATE INDEX IF NOT EXISTS idx_previousOwners_motorcycleId ON previousOwners(motorcycleId);
CREATE INDEX IF NOT EXISTS idx_torqueSpecs_motorcycleId ON torqueSpecs(motorcycleId);
CREATE INDEX IF NOT EXISTS idx_documentMotorcycles_motorcycleId ON documentMotorcycles(motorcycleId);
