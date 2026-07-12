-- The sidecar wheel is a tire position, not a riding configuration: it can
-- carry its own pressure in each of the solo / passenger / offroad sets.
-- The existing sidecarBar column keeps holding the solo value; these two add
-- the variant values (both nullable, like the other variant columns).

ALTER TABLE tirePressures ADD COLUMN sidecarPassengerBar REAL;
ALTER TABLE tirePressures ADD COLUMN sidecarOffroadBar REAL;
