-- Drivetrain type so the maintenance UI can show only the relevant options:
-- 'chain' (Kettenantrieb) or 'shaft' (Kardanantrieb). NULL = unconfigured, in
-- which case the UI keeps showing every option (chain and shaft alike).
ALTER TABLE motorcycles ADD COLUMN driveType TEXT;
