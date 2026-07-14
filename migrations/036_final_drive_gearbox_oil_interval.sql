-- A separate final-drive gearbox oil ("Hinterachsgetriebeöl"), for motorcycles
-- whose final drive has its own gearbox — distinct from the shaft/Kardan oil.
-- Configurable like the other oils: default 2 years, km interval optional.
ALTER TABLE userSettings ADD COLUMN finalDriveGearboxOilInterval INTEGER NOT NULL DEFAULT 2;
ALTER TABLE userSettings ADD COLUMN finalDriveGearboxOilKmInterval INTEGER;
