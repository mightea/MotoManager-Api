-- Marks that further, unidentified previous owners exist beyond the recorded
-- ones — i.e. the ownership history is known to be incomplete. When set, the
-- clients stop asserting a definitive "N. Hand" position (see the webapp info
-- card). Follows the default-0 boolean convention (see 030_motorcycle_has_sidecar).

ALTER TABLE motorcycles ADD COLUMN hasUnknownOwners BOOLEAN NOT NULL DEFAULT 0;
