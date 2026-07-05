-- Prune seed catalog entries that aren't relevant for this garage:
-- S-Modelle, C-Modelle and F-Modelle entirely, and everything under
-- R-Modelle 4V except the GS models (R 1100 GS / R 1150 GS stay).
--
-- Guards: only global seed rows (userId IS NULL) that are unreferenced by
-- parts or motorcycles are removed — anything in use survives the prune.
-- Children first, then the (now empty) families.

DELETE FROM modelSeries
WHERE userId IS NULL
  AND name IN (
      -- S-Modelle
      'S 1000 R', 'S 1000 RR', 'S 1000 XR',
      -- C-Modelle
      'C 400 X', 'C 650 GT',
      -- F-Modelle
      'F 650', 'F 650 GS', 'F 700 GS', 'F 750 GS', 'F 800 GS', 'F 850 GS',
      'F 900 R', 'F650 (Funduro, ST) (93-00)',
      -- R-Modelle 4V, alles ausser GS
      'R 850 R', 'R 1100 R', 'R 1100 RT', 'R 1100 S', 'R 1150 R', 'R 1150 RT',
      'R259 (R 850 R, R 1100 R/GS/RS/RT)'
  )
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat)
  AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL)
  AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);

DELETE FROM modelSeries
WHERE userId IS NULL
  AND name IN ('S-Modelle', 'C-Modelle', 'F-Modelle')
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat)
  AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL)
  AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);
