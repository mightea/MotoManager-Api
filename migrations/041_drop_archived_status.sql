-- Drop the "archived" lifecycle state: a bike is either active or sold. Any
-- previously-archived bike becomes sold. `isArchived` stays as a derived legacy
-- column (= sold) for backward-compatible clients (iOS still reads it).
UPDATE motorcycles SET status = 'sold' WHERE status = 'archived';
