-- Admin-configurable minimum build numbers for the iOS app (single-row table).
--
-- softUpgradeBuild: clients whose build number is <= this value show an
--   update reminder after login but keep working.
-- hardUpgradeBuild: clients whose build number is < this value are out of
--   support and must stop talking to the backend until updated.
--
-- 0/0 (the seeded defaults) disable both checks, since real build numbers
-- start at 1.
CREATE TABLE appUpgradeSettings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    softUpgradeBuild INTEGER NOT NULL DEFAULT 0,
    hardUpgradeBuild INTEGER NOT NULL DEFAULT 0,
    updatedAt TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO appUpgradeSettings (id) VALUES (1);
