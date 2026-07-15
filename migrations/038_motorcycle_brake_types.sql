-- Per-wheel brake type so the maintenance UI can present only the relevant
-- brake jobs. Values: 'disc' (Scheibenbremse) or 'drum' (Trommelbremse); NULL
-- means unconfigured (the UI then falls back to showing every brake option).
-- The sidecar column is only meaningful when the motorcycle hasSidecar.
ALTER TABLE motorcycles ADD COLUMN frontBrakeType TEXT;
ALTER TABLE motorcycles ADD COLUMN rearBrakeType TEXT;
ALTER TABLE motorcycles ADD COLUMN sidecarBrakeType TEXT;
