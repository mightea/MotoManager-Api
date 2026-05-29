-- Recommended tire pressures for a motorcycle.
-- 1:1 with motorcycles (UNIQUE motorcycleId enforces the contract).
-- Pressures stored canonically in bar; preferredUnit remembers whether
-- the user originally entered bar or psi so the form re-opens in their
-- chosen unit.

CREATE TABLE IF NOT EXISTS tirePressures (
    id            INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    motorcycleId  INTEGER NOT NULL UNIQUE,
    frontBar      REAL    NOT NULL,
    rearBar       REAL    NOT NULL,
    sidecarBar    REAL,
    preferredUnit TEXT    NOT NULL DEFAULT 'bar' CHECK (preferredUnit IN ('bar','psi')),
    createdAt     TEXT    NOT NULL DEFAULT (CURRENT_TIMESTAMP),
    updatedAt     TEXT    NOT NULL DEFAULT (CURRENT_TIMESTAMP),
    FOREIGN KEY (motorcycleId) REFERENCES motorcycles(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_tirePressures_motorcycleId
    ON tirePressures(motorcycleId);
