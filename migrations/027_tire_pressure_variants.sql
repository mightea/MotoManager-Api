-- Tire pressure variants: riding solo (the existing frontBar/rearBar values),
-- riding with a passenger, and offroad. The new columns are nullable — most
-- riders only record the solo values; the variants render only when present.

ALTER TABLE tirePressures ADD COLUMN frontPassengerBar REAL;
ALTER TABLE tirePressures ADD COLUMN rearPassengerBar REAL;
ALTER TABLE tirePressures ADD COLUMN frontOffroadBar REAL;
ALTER TABLE tirePressures ADD COLUMN rearOffroadBar REAL;
