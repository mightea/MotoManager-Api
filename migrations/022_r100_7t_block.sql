-- The 1980-1984 R 100 catalog model ("R 100 /7T", code 0425, frames
-- 6035001-6040000 per Snowbum) was missing, leaving e.g. frame 6039893
-- unmatched. Adds it as a Modell under the /7 Serie, and extends the Serie
-- with the other verified 1980+ blocks (R100RS 6075001-6080000, R100CS
-- 6135001-6140000, R100T 6193001-6195000) so those frames resolve too.

INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes, frameRanges)
SELECT 'R 100 /7T (ECE, 06/1980-10/1984)', 'BMW', id, '0425', '6035001-6040000'
FROM modelSeries
WHERE name = 'R 60/7, R 75/7, R 80/7, R 100/7-T-S-RS-RT (76-84)' AND userId IS NULL
  AND NOT EXISTS (
      SELECT 1 FROM modelSeries
      WHERE name = 'R 100 /7T (ECE, 06/1980-10/1984)' AND userId IS NULL
  );

UPDATE modelSeries SET frameRanges =
    frameRanges || ',6035001-6040000,6075001-6080000,6135001-6140000,6193001-6195000'
    WHERE userId IS NULL
    AND name = 'R 60/7, R 75/7, R 80/7, R 100/7-T-S-RS-RT (76-84)'
    AND frameRanges NOT LIKE '%6035001-6040000%';
