-- R 80 GS PD (CH): the Swiss-market Paris-Dakar catalog model (08/1990 -
-- 06/1995), missing from the seed. Its ECE serial block sits between two
-- known blocks — K75C ends at 0118000 and K75-Authorities starts at 0120001
-- — so 0118001-0119999 is the tightest safe bound (factory records for the
-- exact end are not public; refine via the Modellkatalog if needed).
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges)
SELECT 'R 80 GS PD (CH) (ECE, 08/1990-06/1995)', 'BMW', id, '0118001-0119999'
FROM modelSeries
WHERE name = 'R 80 GS, R 100 GS, PD (90-95)' AND userId IS NULL
  AND NOT EXISTS (
      SELECT 1 FROM modelSeries
      WHERE name = 'R 80 GS PD (CH) (ECE, 08/1990-06/1995)' AND userId IS NULL
  );
