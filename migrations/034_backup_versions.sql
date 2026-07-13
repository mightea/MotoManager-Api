-- Record the app versions in effect when each backup was taken, so an admin can
-- see (and the archive's manifest.json documents) exactly which frontend/backend
-- a given archive corresponds to. Nullable: the frontend version is only known
-- when supplied (manual backup from the webapp, or the FRONTEND_VERSION env).
ALTER TABLE backups ADD COLUMN backendVersion TEXT;
ALTER TABLE backups ADD COLUMN frontendVersion TEXT;
