-- Make the solo values nullable so every riding configuration (solo /
-- passenger / offroad) is independently recordable and deletable — deleting
-- the solo set must no longer take the whole record with it. A row now
-- requires at least one complete front/rear pair, enforced by the handler.
-- SQLite can't drop NOT NULL in place, so rebuild the table.

CREATE TABLE tirePressures_new (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    motorcycleId        INTEGER NOT NULL UNIQUE,
    frontBar            REAL,
    rearBar             REAL,
    frontPassengerBar   REAL,
    rearPassengerBar    REAL,
    frontOffroadBar     REAL,
    rearOffroadBar      REAL,
    sidecarBar          REAL,
    sidecarPassengerBar REAL,
    sidecarOffroadBar   REAL,
    preferredUnit       TEXT NOT NULL DEFAULT 'bar' CHECK (preferredUnit IN ('bar','psi')),
    createdAt           TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP),
    updatedAt           TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP),
    FOREIGN KEY (motorcycleId) REFERENCES motorcycles(id) ON DELETE CASCADE
);

INSERT INTO tirePressures_new
    (id, motorcycleId, frontBar, rearBar,
     frontPassengerBar, rearPassengerBar, frontOffroadBar, rearOffroadBar,
     sidecarBar, sidecarPassengerBar, sidecarOffroadBar,
     preferredUnit, createdAt, updatedAt)
SELECT
    id, motorcycleId, frontBar, rearBar,
    frontPassengerBar, rearPassengerBar, frontOffroadBar, rearOffroadBar,
    sidecarBar, sidecarPassengerBar, sidecarOffroadBar,
    preferredUnit, createdAt, updatedAt
FROM tirePressures;

DROP TABLE tirePressures;
ALTER TABLE tirePressures_new RENAME TO tirePressures;

CREATE INDEX IF NOT EXISTS idx_tirePressures_motorcycleId
    ON tirePressures(motorcycleId);
