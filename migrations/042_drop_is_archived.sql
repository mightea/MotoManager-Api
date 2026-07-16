-- Drop the legacy isArchived column: `status` (active/sold) is now the single
-- source of truth, and all clients (including iOS) have moved off isArchived.
ALTER TABLE motorcycles DROP COLUMN isArchived;
