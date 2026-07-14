-- Minimum kilometres a motorcycle should be ridden per year. The overview card
-- flags a bike's yearly distance as a warning when it falls below this value.
-- Default 150.
ALTER TABLE userSettings ADD COLUMN minKmPerYear INTEGER NOT NULL DEFAULT 150;
