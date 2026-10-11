-- Locations: optional ISO 3166-1 alpha-2 country code (stored uppercase).
--
-- Migration 009 dropped a `countryCode` column that was always 'CH' and never
-- displayed. This one re-adds it as a nullable, client-maintained value (the
-- webapp fills it via reverse geocoding) so the map analytics can report
-- countries visited and kilometres per country.
ALTER TABLE locations ADD COLUMN countryCode TEXT;
