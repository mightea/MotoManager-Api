-- Hierarchical model catalog (realoem-style), optimized for post-war BMW up
-- to ~2000: Familie (depth 0) -> Serie (depth 1) -> Modell (depth 2).
-- The existing flat seed rows keep their ids (parts/motorcycles reference
-- them) and are re-parented under new family nodes. Depth is derived from the
-- parent chain; the handlers cap it at 3 levels.

ALTER TABLE modelSeries ADD COLUMN parentId INTEGER REFERENCES modelSeries(id);

-- --------------------------------------------------------------------------
-- Familien (depth 0)
-- --------------------------------------------------------------------------
INSERT INTO modelSeries (name, manufacturer) VALUES
    ('R-Modelle /2 (1950-1969)', 'BMW'),
    ('R-Modelle /5 /6 /7 (1969-1984)', 'BMW'),
    ('R-Modelle 2V (1978-1996)', 'BMW'),
    ('R-Modelle 4V (1993-2006)', 'BMW'),
    ('K-Modelle 3-Zyl.', 'BMW'),
    ('K-Modelle 4-Zyl. 2V', 'BMW'),
    ('K-Modelle 4-Zyl. 4V', 'BMW'),
    ('F-Modelle', 'BMW'),
    ('G-Modelle', 'BMW'),
    ('S-Modelle', 'BMW'),
    ('C-Modelle', 'BMW'),
    ('Modelle ab 2000', 'BMW');

-- --------------------------------------------------------------------------
-- Re-parent the existing flat seed rows (they become Serie-level nodes).
-- Matches global rows only; user-created custom entries are left untouched.
-- --------------------------------------------------------------------------
UPDATE modelSeries SET parentId =
    (SELECT id FROM modelSeries WHERE name = 'R-Modelle 2V (1978-1996)' AND userId IS NULL)
    WHERE userId IS NULL AND parentId IS NULL
    AND name IN ('R 45', 'R 65', 'R 80', 'R 80 GS', 'R 100', 'R 100 GS');

UPDATE modelSeries SET parentId =
    (SELECT id FROM modelSeries WHERE name = 'R-Modelle 4V (1993-2006)' AND userId IS NULL)
    WHERE userId IS NULL AND parentId IS NULL
    AND name IN ('R 850 R', 'R 1100 GS', 'R 1100 R', 'R 1100 RT', 'R 1100 S',
                 'R 1150 GS', 'R 1150 R', 'R 1150 RT');

UPDATE modelSeries SET parentId =
    (SELECT id FROM modelSeries WHERE name = 'K-Modelle 3-Zyl.' AND userId IS NULL)
    WHERE userId IS NULL AND parentId IS NULL AND name = 'K 75';

UPDATE modelSeries SET parentId =
    (SELECT id FROM modelSeries WHERE name = 'K-Modelle 4-Zyl. 2V' AND userId IS NULL)
    WHERE userId IS NULL AND parentId IS NULL AND name = 'K 100';

UPDATE modelSeries SET parentId =
    (SELECT id FROM modelSeries WHERE name = 'K-Modelle 4-Zyl. 4V' AND userId IS NULL)
    WHERE userId IS NULL AND parentId IS NULL AND name IN ('K 1100', 'K 1200', 'K 1600');

UPDATE modelSeries SET parentId =
    (SELECT id FROM modelSeries WHERE name = 'F-Modelle' AND userId IS NULL)
    WHERE userId IS NULL AND parentId IS NULL
    AND name IN ('F 650', 'F 650 GS', 'F 700 GS', 'F 750 GS', 'F 800 GS', 'F 850 GS', 'F 900 R');

UPDATE modelSeries SET parentId =
    (SELECT id FROM modelSeries WHERE name = 'G-Modelle' AND userId IS NULL)
    WHERE userId IS NULL AND parentId IS NULL AND name IN ('G 310 GS', 'G 310 R');

UPDATE modelSeries SET parentId =
    (SELECT id FROM modelSeries WHERE name = 'S-Modelle' AND userId IS NULL)
    WHERE userId IS NULL AND parentId IS NULL
    AND name IN ('S 1000 R', 'S 1000 RR', 'S 1000 XR');

UPDATE modelSeries SET parentId =
    (SELECT id FROM modelSeries WHERE name = 'C-Modelle' AND userId IS NULL)
    WHERE userId IS NULL AND parentId IS NULL AND name IN ('C 400 X', 'C 650 GT');

UPDATE modelSeries SET parentId =
    (SELECT id FROM modelSeries WHERE name = 'Modelle ab 2000' AND userId IS NULL)
    WHERE userId IS NULL AND parentId IS NULL
    AND name IN ('R 1200 GS', 'R 1200 R', 'R 1200 RT', 'R nineT');

-- --------------------------------------------------------------------------
-- Serien (depth 1) — canonical groupings for the classic range.
-- --------------------------------------------------------------------------
INSERT INTO modelSeries (name, manufacturer, parentId) VALUES
    ('R 25/3, R 26, R 27 (Einzylinder)', 'BMW',
        (SELECT id FROM modelSeries WHERE name = 'R-Modelle /2 (1950-1969)' AND userId IS NULL)),
    ('R 50, R 60, R 69 S (Boxer /2)', 'BMW',
        (SELECT id FROM modelSeries WHERE name = 'R-Modelle /2 (1950-1969)' AND userId IS NULL)),
    ('R 50/5, R 60/5, R 75/5 (69-73)', 'BMW',
        (SELECT id FROM modelSeries WHERE name = 'R-Modelle /5 /6 /7 (1969-1984)' AND userId IS NULL)),
    ('R 60/6, R 75/6, R 90/6, R 90 S (73-76)', 'BMW',
        (SELECT id FROM modelSeries WHERE name = 'R-Modelle /5 /6 /7 (1969-1984)' AND userId IS NULL)),
    ('R 60/7, R 75/7, R 80/7, R 100/7-T-S-RS-RT (76-84)', 'BMW',
        (SELECT id FROM modelSeries WHERE name = 'R-Modelle /5 /6 /7 (1969-1984)' AND userId IS NULL)),
    ('R 80 G/S, R 80 ST (80-87)', 'BMW',
        (SELECT id FROM modelSeries WHERE name = 'R-Modelle 2V (1978-1996)' AND userId IS NULL)),
    ('R 65, R 65 RT, R 80, R 80 RT (85-95)', 'BMW',
        (SELECT id FROM modelSeries WHERE name = 'R-Modelle 2V (1978-1996)' AND userId IS NULL)),
    ('R 80 GS, R 100 GS, PD (90-95)', 'BMW',
        (SELECT id FROM modelSeries WHERE name = 'R-Modelle 2V (1978-1996)' AND userId IS NULL)),
    ('R 100 RS, R 100 RT (87-95)', 'BMW',
        (SELECT id FROM modelSeries WHERE name = 'R-Modelle 2V (1978-1996)' AND userId IS NULL)),
    ('R 80 R, R 100 R, Mystic (91-96)', 'BMW',
        (SELECT id FROM modelSeries WHERE name = 'R-Modelle 2V (1978-1996)' AND userId IS NULL)),
    ('K569 (K 75, K 75 C, K 75 S, K 75 RT)', 'BMW',
        (SELECT id FROM modelSeries WHERE name = 'K-Modelle 3-Zyl.' AND userId IS NULL)),
    ('K589 (K 100, RS, RT, LT)', 'BMW',
        (SELECT id FROM modelSeries WHERE name = 'K-Modelle 4-Zyl. 2V' AND userId IS NULL)),
    ('K589 4V (K 1100 LT, K 1100 RS)', 'BMW',
        (SELECT id FROM modelSeries WHERE name = 'K-Modelle 4-Zyl. 4V' AND userId IS NULL)),
    ('R259 (R 850 R, R 1100 R/GS/RS/RT)', 'BMW',
        (SELECT id FROM modelSeries WHERE name = 'R-Modelle 4V (1993-2006)' AND userId IS NULL)),
    ('F650 (Funduro, ST) (93-00)', 'BMW',
        (SELECT id FROM modelSeries WHERE name = 'F-Modelle' AND userId IS NULL));

-- --------------------------------------------------------------------------
-- Modelle (depth 2) — catalog-model examples with market and build period.
-- --------------------------------------------------------------------------
INSERT INTO modelSeries (name, manufacturer, parentId) VALUES
    ('R 80 GS (ECE, 04/1990-10/1995)', 'BMW',
        (SELECT id FROM modelSeries WHERE name = 'R 80 GS, R 100 GS, PD (90-95)' AND userId IS NULL)),
    ('R 100 GS (ECE, 04/1990-07/1996)', 'BMW',
        (SELECT id FROM modelSeries WHERE name = 'R 80 GS, R 100 GS, PD (90-95)' AND userId IS NULL)),
    ('R 100 GS PD (ECE, 08/1990-07/1996)', 'BMW',
        (SELECT id FROM modelSeries WHERE name = 'R 80 GS, R 100 GS, PD (90-95)' AND userId IS NULL)),
    ('R 80 (ECE, 03/1984-01/1995)', 'BMW',
        (SELECT id FROM modelSeries WHERE name = 'R 65, R 65 RT, R 80, R 80 RT (85-95)' AND userId IS NULL)),
    ('R 80 RT (ECE, 07/1984-12/1995)', 'BMW',
        (SELECT id FROM modelSeries WHERE name = 'R 65, R 65 RT, R 80, R 80 RT (85-95)' AND userId IS NULL)),
    ('R 65 (35KW) (ECE, 06/1985-12/1992)', 'BMW',
        (SELECT id FROM modelSeries WHERE name = 'R 65, R 65 RT, R 80, R 80 RT (85-95)' AND userId IS NULL)),
    ('K 75 RT (ECE, 10/1989-06/1996)', 'BMW',
        (SELECT id FROM modelSeries WHERE name = 'K569 (K 75, K 75 C, K 75 S, K 75 RT)' AND userId IS NULL)),
    ('K 100 RS 83 (0502,0503,0513) (ECE)', 'BMW',
        (SELECT id FROM modelSeries WHERE name = 'K589 (K 100, RS, RT, LT)' AND userId IS NULL));

CREATE INDEX idx_model_series_parent ON modelSeries(parentId);
