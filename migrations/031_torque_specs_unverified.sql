-- Flag torque specs whose values come from an uncertain source and should not be
-- trusted blindly. Follows the default-0 boolean convention (see 026_part_stocks_is_used,
-- 030_motorcycle_has_sidecar).
ALTER TABLE torqueSpecs ADD COLUMN unverified INTEGER NOT NULL DEFAULT 0;
