-- Anchor a root-level part storage location to a physical place from the
-- existing locations entity (garage/workshop). Only meaningful on roots —
-- nested containers inherit their place from the top of the tree; the handler
-- clears the link when a location is nested under a parent.
ALTER TABLE storageLocations ADD COLUMN locationId INTEGER REFERENCES locations(id);
