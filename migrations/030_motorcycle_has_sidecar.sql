-- Whether the motorcycle is a sidecar rig. Gates the sidecar-related UI
-- (e.g. the sidecar-wheel tire pressure) in the clients.

ALTER TABLE motorcycles ADD COLUMN hasSidecar BOOLEAN NOT NULL DEFAULT 0;
