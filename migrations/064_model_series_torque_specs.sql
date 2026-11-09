-- Torque values shared into the model catalog. Catalog lists are visible to
-- every user of the instance, so rows only get here through an explicit
-- "share" by a user: a copy of that user's bike specs, keyed by the bike's
-- catalog node (Familie, Serie or Modell). One contribution per user and node
-- — re-sharing replaces the previous rows of that user for that node.
-- Importing copies rows back into a bike as ordinary torqueSpecs.
CREATE TABLE modelSeriesTorqueSpecs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    seriesId INTEGER NOT NULL REFERENCES modelSeries(id) ON DELETE CASCADE,
    userId INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    sourceMotorcycleId INTEGER REFERENCES motorcycles(id) ON DELETE SET NULL,
    category TEXT NOT NULL,
    name TEXT NOT NULL,
    torque REAL NOT NULL,
    torqueEnd REAL,
    variation REAL,
    toolSize TEXT,
    description TEXT,
    descriptionMarkup TEXT,
    unverified INTEGER NOT NULL DEFAULT 0,
    createdAt TEXT NOT NULL
);
CREATE INDEX idx_model_series_torque_specs_series ON modelSeriesTorqueSpecs(seriesId);
CREATE INDEX idx_model_series_torque_specs_user ON modelSeriesTorqueSpecs(userId, seriesId);
