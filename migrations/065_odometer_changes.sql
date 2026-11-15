-- Odometer changes: the instrument was replaced (or a five-digit counter rolled
-- over), so the reading on the bike no longer equals the distance it has run.
--
-- Each row is one such event. `readingBefore` is what the old instrument showed
-- when it came off, `readingAfter` what the new one showed when it went on;
-- both are raw instrument readings. The offset of a change is
-- readingBefore - readingAfter, and the offset that applies to a date is the
-- sum over every change dated on or before it.
--
-- Every `odo` column keeps holding the TOTAL distance (instrument reading +
-- offset), so MAX(odo), yearly distances, service intervals and the fuel chain
-- stay correct without knowing about this table. Clients convert at the edges:
-- the user types the instrument reading, the stored value is the total.
CREATE TABLE odometerChanges (
    id             INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    motorcycleId   INTEGER NOT NULL REFERENCES motorcycles(id) ON DELETE CASCADE,
    date           TEXT    NOT NULL,
    readingBefore  INTEGER NOT NULL CHECK (readingBefore >= 0),
    readingAfter   INTEGER NOT NULL CHECK (readingAfter >= 0),
    -- "replacement" | "rollover"; informational only, the maths is the same.
    kind           TEXT    NOT NULL DEFAULT 'replacement',
    createdAt      TEXT    NOT NULL DEFAULT (CURRENT_TIMESTAMP),
    updatedAt      TEXT    NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);

CREATE INDEX idx_odometerChanges_motorcycleId_date
    ON odometerChanges(motorcycleId, date, id);
