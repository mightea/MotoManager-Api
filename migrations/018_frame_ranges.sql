-- Frame-number ranges for pre-1981 BMWs (no 17-char VIN yet): identification
-- works by 6-7 digit serial ranges per model. Stored as comma-separated
-- "start-end" pairs. Ranges sit at the Serie level where the catalog has a
-- matching Serie; models predating the seeded Serien (R24-R25/2 singles,
-- R51-R68 boxers) anchor at the Familie level so they still resolve.
--
-- Ranges sourced from Snowbum's serial table
-- (bmwmotorcycletech.info/id-yr-codes-pwr.htm).

ALTER TABLE modelSeries ADD COLUMN frameRanges TEXT;

-- Familie level: early models without a seeded Serie -------------------------
-- R24 200009-212007, R25 220011-243410, R25/2 245001-283650,
-- R51/2 516001-521005, R51/3 522001-540950, R67 610001-611445,
-- R67/2 612001-616226, R67/3 616227-617700, R68 650001-651453
UPDATE modelSeries SET frameRanges =
    '200009-212007,220011-243410,245001-283650,516001-521005,522001-540950,610001-611445,612001-616226,616227-617700,650001-651453'
    WHERE userId IS NULL AND name = 'R-Modelle /2 (1950-1969)';

-- Serie level -----------------------------------------------------------------
-- R25/3 284001-331705, R26 340001-370242, R27 372001-387566
UPDATE modelSeries SET frameRanges = '284001-331705,340001-370242,372001-387566'
    WHERE userId IS NULL AND name = 'R 25/3, R 26, R 27 (Einzylinder)';

-- R50 550001-563515, R50S 564001-565634, R60 618001-621530,
-- R60/2 622001-629999 + 1810001-1819307 (number rollover spring 1966),
-- R50/2 630001-649037, R69 652001-654955, R69S 655004-666320
UPDATE modelSeries SET frameRanges =
    '550001-563515,564001-565634,618001-621530,622001-629999,1810001-1819307,630001-649037,652001-654955,655004-666320'
    WHERE userId IS NULL AND name = 'R 50, R 60, R 69 S (Boxer /2)';

-- R50/5 2900001-2910000, R60/5 2930001-2950000, R75/5 2970001-3000000
UPDATE modelSeries SET frameRanges = '2900001-2910000,2930001-2950000,2970001-3000000'
    WHERE userId IS NULL AND name = 'R 50/5, R 60/5, R 75/5 (69-73)';

-- R60/6 + R75/6 4900001-4947578 (overlapping factory blocks, merged),
-- R90/6 4040001-4100000, R90S 4950001-4991260
UPDATE modelSeries SET frameRanges = '4900001-4947578,4040001-4100000,4950001-4991260'
    WHERE userId IS NULL AND name = 'R 60/6, R 75/6, R 90/6, R 90 S (73-76)';

-- R60/7 6000001-6012000 + 6015001-6016000, R60/7 USA 6100001-6102000,
-- R75/7 6020001-6025000 + 6220000-6223000, R75/7+R80/7 USA 6120001-6125000
-- + 6126001-6128000, R100/7 6040001-6054000
UPDATE modelSeries SET frameRanges =
    '6000001-6012000,6015001-6016000,6100001-6102000,6020001-6025000,6220000-6223000,6120001-6125000,6126001-6128000,6040001-6054000'
    WHERE userId IS NULL AND name = 'R 60/7, R 75/7, R 80/7, R 100/7-T-S-RS-RT (76-84)';
