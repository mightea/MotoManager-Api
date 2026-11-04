-- Odometer reading at the moment of sale, alongside the other sale details
-- (migration 040). Nullable: unknown for older sales and for active bikes.
-- It is the bike's last known reading, so stats count the km driven up to the
-- sale even when no maintenance or fuel record was logged that day.
ALTER TABLE motorcycles ADD COLUMN saleOdo INTEGER;
