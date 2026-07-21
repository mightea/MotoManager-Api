-- Previous owners are ordered explicitly by the user. Preserve the current
-- newest-purchase-first presentation when assigning initial positions, while
-- allowing the historical purchase date to be unknown.
--
-- SQLite cannot drop a NOT NULL constraint in place, so rebuild the table.

CREATE TABLE previousOwners_new (
    id            INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    motorcycleId  INTEGER NOT NULL REFERENCES motorcycles(id) ON DELETE CASCADE,
    name          TEXT NOT NULL,
    surname       TEXT NOT NULL,
    purchaseDate  TEXT,
    sortOrder     INTEGER NOT NULL DEFAULT 0 CHECK (sortOrder >= 0),
    address       TEXT,
    city          TEXT,
    postcode      TEXT,
    country       TEXT,
    phoneNumber   TEXT,
    email         TEXT,
    comments      TEXT,
    createdAt     TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP),
    updatedAt     TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);

INSERT INTO previousOwners_new (
    id, motorcycleId, name, surname, purchaseDate, sortOrder, address, city,
    postcode, country, phoneNumber, email, comments, createdAt, updatedAt
)
SELECT
    id,
    motorcycleId,
    name,
    surname,
    purchaseDate,
    ROW_NUMBER() OVER (
        PARTITION BY motorcycleId
        ORDER BY purchaseDate DESC, id DESC
    ) - 1,
    address,
    city,
    postcode,
    country,
    phoneNumber,
    email,
    comments,
    createdAt,
    updatedAt
FROM previousOwners;

DROP TABLE previousOwners;
ALTER TABLE previousOwners_new RENAME TO previousOwners;

CREATE INDEX idx_previousOwners_motorcycleId_sortOrder
    ON previousOwners(motorcycleId, sortOrder, id);
