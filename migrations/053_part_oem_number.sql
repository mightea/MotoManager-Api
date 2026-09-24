-- BMW (OEM) part number for aftermarket/other-vendor parts: a Boxxerparts
-- article or a hand-made entry can name the BMW number it replaces. Used to
-- enrich the part from BMWBike (fitment, image, description) and to match
-- invoice lines that carry the BMW number onto the aftermarket part.
-- Plain column on parts, so it rides the existing ?since delta sync.
ALTER TABLE parts ADD COLUMN oemPartNumber TEXT;
CREATE INDEX idx_parts_oem_number ON parts(userId, oemPartNumber) WHERE oemPartNumber IS NOT NULL;
