-- Add parentId column for bundled maintenance items
ALTER TABLE maintenanceRecords ADD COLUMN parentId INTEGER REFERENCES maintenanceRecords(id) ON DELETE CASCADE;
