-- F 650 GS (R13, single cylinder): the only F-Modell the garage supports.
-- Migration 016 pruned the F-Modelle seed branch, but only unreferenced
-- rows — a DB whose motorcycles link 'F 650 GS' still carries the old rows.
-- Re-add the minimal branch (Familie 'F-Modelle' > Modell 'F 650 GS') and
-- stamp type code 0172 (ECE, VIN positions 4-7) so VIN decode matches.

INSERT INTO modelSeries (name, manufacturer)
SELECT 'F-Modelle', 'BMW'
WHERE NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'F-Modelle' AND userId IS NULL);

INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'F 650 GS', 'BMW', id, '0172' FROM modelSeries
WHERE name = 'F-Modelle' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'F 650 GS' AND userId IS NULL);

UPDATE modelSeries
SET typeCodes = '0172',
    parentId = (SELECT id FROM modelSeries WHERE name = 'F-Modelle' AND userId IS NULL)
WHERE userId IS NULL AND name = 'F 650 GS'
  AND (typeCodes IS NOT '0172'
       OR parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'F-Modelle' AND userId IS NULL));
