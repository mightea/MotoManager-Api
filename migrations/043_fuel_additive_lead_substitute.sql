-- Fuel additive / lead substitute flags for fuel records
ALTER TABLE maintenanceRecords ADD COLUMN fuelAdditiveAdded INTEGER NOT NULL DEFAULT 0;
ALTER TABLE maintenanceRecords ADD COLUMN leadSubstituteAdded INTEGER NOT NULL DEFAULT 0;
