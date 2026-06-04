-- Drop the countryCode column from locations.
--
-- The frontend no longer surfaces or sorts by country, and every existing row
-- defaulted to 'CH', so the column carried no real information. SQLite
-- supports DROP COLUMN since 3.35.

ALTER TABLE locations DROP COLUMN countryCode;
