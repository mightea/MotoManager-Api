-- Whether a fuel record filled the tank. Consumption is measured from one full
-- tank to the next: a partial fill gets no consumption and its litres count
-- towards the next full one. Existing records were all treated as full tanks,
-- so they default to 1 and their derived values stay as they are.
ALTER TABLE maintenanceRecords ADD COLUMN fullTank INTEGER NOT NULL DEFAULT 1;
