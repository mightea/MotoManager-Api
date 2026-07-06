-- Catalog restructure per BMWBike.com (https://bmwbike.com/de/bikes):
-- Familie > Serie > Modell mirrored from the site's live data, excluding
-- pre-war, C-, G-, F-, S-Modelle, 6-cyl K bikes and Sondermotoren.
-- Existing global rows matching site names by rename or equality keep
-- their ids (and thus part/motorcycle links plus VIN metadata); verified
-- typeCodes/frameRanges are transplanted onto the new structure; obsolete
-- old entries are deleted only when unreferenced and childless.
-- Generated from the site's API (see repo history for the generator).

-- 1) Renames (keep ids + metadata) --------------------------------------
UPDATE modelSeries SET name = 'Boxer' WHERE userId IS NULL AND name = 'R-Modelle /2 (1950-1969)'
    AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'Boxer' AND userId IS NULL);
UPDATE modelSeries SET name = 'R-Boxer' WHERE userId IS NULL AND name = 'R-Modelle /5 /6 /7 (1969-1984)'
    AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R-Boxer' AND userId IS NULL);
UPDATE modelSeries SET name = 'R-Modelle 2V' WHERE userId IS NULL AND name = 'R-Modelle 2V (1978-1996)'
    AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R-Modelle 2V' AND userId IS NULL);
UPDATE modelSeries SET name = 'R-Modelle 4V' WHERE userId IS NULL AND name = 'R-Modelle 4V (1993-2006)'
    AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R-Modelle 4V' AND userId IS NULL);
UPDATE modelSeries SET name = 'K569 (K 75, K 75 c, K 75 s, K 75 RT)' WHERE userId IS NULL AND name = 'K569 (K 75, K 75 C, K 75 S, K 75 RT)'
    AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K569 (K 75, K 75 c, K 75 s, K 75 RT)' AND userId IS NULL);
UPDATE modelSeries SET name = 'K589 (K100, RS, RT, LT)' WHERE userId IS NULL AND name = 'K589 (K 100, RS, RT, LT)'
    AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K589 (K100, RS, RT, LT)' AND userId IS NULL);
UPDATE modelSeries SET name = 'R 80 R, R 100 R, Mystik (91-95)' WHERE userId IS NULL AND name = 'R 80 R, R 100 R, Mystic (91-96)'
    AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80 R, R 100 R, Mystik (91-95)' AND userId IS NULL);
UPDATE modelSeries SET name = 'R 65 GS, R 80 G/S, R 80 ST (80-92)' WHERE userId IS NULL AND name = 'R 80 G/S, R 80 ST (80-87)'
    AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 65 GS, R 80 G/S, R 80 ST (80-92)' AND userId IS NULL);
UPDATE modelSeries SET name = 'K589 (K1, K 100 RS)' WHERE userId IS NULL AND name = 'K589 4V (K 1100 LT, K 1100 RS)'
    AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K589 (K1, K 100 RS)' AND userId IS NULL);

-- 2) Familien ------------------------------------------------------------
INSERT INTO modelSeries (name, manufacturer)
SELECT '1-Zyl.', 'BMW'
WHERE NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = '1-Zyl.' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer)
SELECT 'Boxer', 'BMW'
WHERE NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'Boxer' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer)
SELECT 'R-Boxer', 'BMW'
WHERE NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R-Boxer' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer)
SELECT 'R-Modelle 2V', 'BMW'
WHERE NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R-Modelle 2V' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer)
SELECT 'R-Modelle 4V', 'BMW'
WHERE NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R-Modelle 4V' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer)
SELECT 'R-Modelle K2x', 'BMW'
WHERE NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R-Modelle K2x' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer)
SELECT 'R-Modelle K5x', 'BMW'
WHERE NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R-Modelle K5x' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer)
SELECT 'R-Modelle R nineT', 'BMW'
WHERE NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R-Modelle R nineT' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer)
SELECT 'K-Modelle 3-Zyl.', 'BMW'
WHERE NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K-Modelle 3-Zyl.' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer)
SELECT 'K-Modelle 4-Zyl. 2V', 'BMW'
WHERE NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K-Modelle 4-Zyl. 2V' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer)
SELECT 'K-Modelle 4-Zyl. 4V', 'BMW'
WHERE NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K-Modelle 4-Zyl. 4V' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer)
SELECT 'K-Modelle K4x 4-Zyl.', 'BMW'
WHERE NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K-Modelle K4x 4-Zyl.' AND userId IS NULL);

-- 3) Serien --------------------------------------------------------------
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'R 24 -50', 'BMW', id, '200009-212007', NULL FROM modelSeries
WHERE name = '1-Zyl.' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 24 -50' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = '1-Zyl.' AND userId IS NULL), frameRanges = '200009-212007'
WHERE userId IS NULL AND name = 'R 24 -50' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = '1-Zyl.' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'R 25 -56', 'BMW', id, '220011-243410,245001-283650,284001-331705', NULL FROM modelSeries
WHERE name = '1-Zyl.' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 25 -56' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = '1-Zyl.' AND userId IS NULL), frameRanges = '220011-243410,245001-283650,284001-331705'
WHERE userId IS NULL AND name = 'R 25 -56' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = '1-Zyl.' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'R 26 -60', 'BMW', id, '340001-370242', NULL FROM modelSeries
WHERE name = '1-Zyl.' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 26 -60' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = '1-Zyl.' AND userId IS NULL), frameRanges = '340001-370242'
WHERE userId IS NULL AND name = 'R 26 -60' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = '1-Zyl.' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'R 27 -66', 'BMW', id, '372001-387566', NULL FROM modelSeries
WHERE name = '1-Zyl.' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 27 -66' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = '1-Zyl.' AND userId IS NULL), frameRanges = '372001-387566'
WHERE userId IS NULL AND name = 'R 27 -66' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = '1-Zyl.' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'R 50 -69', 'BMW', id, '550001-563515,564001-565634,630001-649037', NULL FROM modelSeries
WHERE name = 'Boxer' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 50 -69' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'Boxer' AND userId IS NULL), frameRanges = '550001-563515,564001-565634,630001-649037'
WHERE userId IS NULL AND name = 'R 50 -69' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'Boxer' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'R 51 -54', 'BMW', id, '516001-521005,522001-540950', NULL FROM modelSeries
WHERE name = 'Boxer' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 51 -54' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'Boxer' AND userId IS NULL), frameRanges = '516001-521005,522001-540950'
WHERE userId IS NULL AND name = 'R 51 -54' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'Boxer' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'R 60 -69', 'BMW', id, '618001-621530,622001-629999,1810001-1819307', NULL FROM modelSeries
WHERE name = 'Boxer' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 60 -69' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'Boxer' AND userId IS NULL), frameRanges = '618001-621530,622001-629999,1810001-1819307'
WHERE userId IS NULL AND name = 'R 60 -69' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'Boxer' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'R 67 -55', 'BMW', id, '610001-611445,612001-617700', NULL FROM modelSeries
WHERE name = 'Boxer' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 67 -55' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'Boxer' AND userId IS NULL), frameRanges = '610001-611445,612001-617700'
WHERE userId IS NULL AND name = 'R 67 -55' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'Boxer' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'R 68 -54', 'BMW', id, '650001-651453', NULL FROM modelSeries
WHERE name = 'Boxer' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 68 -54' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'Boxer' AND userId IS NULL), frameRanges = '650001-651453'
WHERE userId IS NULL AND name = 'R 68 -54' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'Boxer' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'R 69 -69', 'BMW', id, '652001-654955,655004-666320', NULL FROM modelSeries
WHERE name = 'Boxer' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 69 -69' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'Boxer' AND userId IS NULL), frameRanges = '652001-654955,655004-666320'
WHERE userId IS NULL AND name = 'R 69 -69' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'Boxer' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'R50/5-R90S 69-76', 'BMW', id, '2900001-2910000,2930001-2950000,2970001-3000000,4900001-4947578,4040001-4100000,4950001-4991260', '0240,0250,0251,0255,0281,0260,0253,0257,0291,0261,0265,0282,0263,0267,0292,0271,0275,0283,0273,0277,0293,0272,0276,0284,0274,0278,0294' FROM modelSeries
WHERE name = 'R-Boxer' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R50/5-R90S 69-76' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Boxer' AND userId IS NULL), frameRanges = '2900001-2910000,2930001-2950000,2970001-3000000,4900001-4947578,4040001-4100000,4950001-4991260', typeCodes = '0240,0250,0251,0255,0281,0260,0253,0257,0291,0261,0265,0282,0263,0267,0292,0271,0275,0283,0273,0277,0293,0272,0276,0284,0274,0278,0294'
WHERE userId IS NULL AND name = 'R50/5-R90S 69-76' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Boxer' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'R45-R65LS..78-85', 'BMW', id, NULL, '0351,0355,0352,0354,0353,0358,0363,0364,0359,0365,0360' FROM modelSeries
WHERE name = 'R-Boxer' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R45-R65LS..78-85' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Boxer' AND userId IS NULL), typeCodes = '0351,0355,0352,0354,0353,0358,0363,0364,0359,0365,0360'
WHERE userId IS NULL AND name = 'R45-R65LS..78-85' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Boxer' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'R 60, R 75 , R 80, /7, RT (76-85)', 'BMW', id, '6000001-6012000,6015001-6016000,6100001-6102000,6020001-6025000,6220000-6223000,6120001-6125000,6126001-6128000', '0301,0321,0371,0311,0331,0341,0381,0302,0326,0372,0312,0382,0332,0392,0327,0373,0423,0322,0374,0342,0383,0443,0343,0384' FROM modelSeries
WHERE name = 'R-Modelle 2V' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 60, R 75 , R 80, /7, RT (76-85)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle 2V' AND userId IS NULL), frameRanges = '6000001-6012000,6015001-6016000,6100001-6102000,6020001-6025000,6220000-6223000,6120001-6125000,6126001-6128000', typeCodes = '0301,0321,0371,0311,0331,0341,0381,0302,0326,0372,0312,0382,0332,0392,0327,0373,0423,0322,0374,0342,0383,0443,0343,0384'
WHERE userId IS NULL AND name = 'R 60, R 75 , R 80, /7, RT (76-85)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle 2V' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'R 65 GS, R 80 G/S, R 80 ST (80-92)', 'BMW', id, '6250001-6260000,6281001-6290000,6291001-6292600,6245001-6247000,6054001-6060000,6125001-6126000', '0346,0471,0348,0347,0349,0482' FROM modelSeries
WHERE name = 'R-Modelle 2V' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 65 GS, R 80 G/S, R 80 ST (80-92)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle 2V' AND userId IS NULL), frameRanges = '6250001-6260000,6281001-6290000,6291001-6292600,6245001-6247000,6054001-6060000,6125001-6126000', typeCodes = '0346,0471,0348,0347,0349,0482'
WHERE userId IS NULL AND name = 'R 65 GS, R 80 G/S, R 80 ST (80-92)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle 2V' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'R 65, R 65 RT, R 80, R 80 RT (85-95)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle 2V' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 65, R 65 RT, R 80, R 80 RT (85-95)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle 2V' AND userId IS NULL)
WHERE userId IS NULL AND name = 'R 65, R 65 RT, R 80, R 80 RT (85-95)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle 2V' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'R 80 GS, R 100 GS, PD (87-90)', 'BMW', id, '6276001-6280000,6331001-6336000', NULL FROM modelSeries
WHERE name = 'R-Modelle 2V' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80 GS, R 100 GS, PD (87-90)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle 2V' AND userId IS NULL), frameRanges = '6276001-6280000,6331001-6336000'
WHERE userId IS NULL AND name = 'R 80 GS, R 100 GS, PD (87-90)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle 2V' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'R 80 GS, R 100 GS, PD (90-95)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle 2V' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80 GS, R 100 GS, PD (90-95)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle 2V' AND userId IS NULL)
WHERE userId IS NULL AND name = 'R 80 GS, R 100 GS, PD (90-95)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle 2V' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'R 80 GS Basic (96)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle 2V' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80 GS Basic (96)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle 2V' AND userId IS NULL)
WHERE userId IS NULL AND name = 'R 80 GS Basic (96)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle 2V' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'R 80 R, R 100 R, Mystik (91-95)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle 2V' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80 R, R 100 R, Mystik (91-95)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle 2V' AND userId IS NULL)
WHERE userId IS NULL AND name = 'R 80 R, R 100 R, Mystik (91-95)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle 2V' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'R 100, /7, /T, CS, RS, RT, S (76-84)', 'BMW', id, '6040001-6054000,6035001-6040000,6075001-6080000,6135001-6140000,6193001-6195000', '0304,0323,0375,0425,0314,0333,0393,0435,0344,0385,0445,0398,0305,0324,0376,0315,0334,0394,0426,0436,0306,0325,0377,0427,0316,0335,0395,0437,0345,0386,0339' FROM modelSeries
WHERE name = 'R-Modelle 2V' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle 2V' AND userId IS NULL), frameRanges = '6040001-6054000,6035001-6040000,6075001-6080000,6135001-6140000,6193001-6195000', typeCodes = '0304,0323,0375,0425,0314,0333,0393,0435,0344,0385,0445,0398,0305,0324,0376,0315,0334,0394,0426,0436,0306,0325,0377,0427,0316,0335,0395,0437,0345,0386,0339'
WHERE userId IS NULL AND name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle 2V' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'R 100 RS, R 100 RT (87-95)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle 2V' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 RS, R 100 RT (87-95)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle 2V' AND userId IS NULL)
WHERE userId IS NULL AND name = 'R 100 RS, R 100 RT (87-95)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle 2V' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT '259 (R 850 GS, R 1100 GS)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle 4V' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = '259 (R 850 GS, R 1100 GS)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle 4V' AND userId IS NULL)
WHERE userId IS NULL AND name = '259 (R 850 GS, R 1100 GS)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle 4V' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'R21 (R 1150 GS)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle 4V' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R21 (R 1150 GS)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle 4V' AND userId IS NULL)
WHERE userId IS NULL AND name = 'R21 (R 1150 GS)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle 4V' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'R21 (R 1150 GS Adventure)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle 4V' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R21 (R 1150 GS Adventure)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle 4V' AND userId IS NULL)
WHERE userId IS NULL AND name = 'R21 (R 1150 GS Adventure)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle 4V' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT '259R (R 850 R, R 1100 R)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle 4V' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = '259R (R 850 R, R 1100 R)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle 4V' AND userId IS NULL)
WHERE userId IS NULL AND name = '259R (R 850 R, R 1100 R)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle 4V' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'R28 (R 850 R, R 1150 R, Rockster)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle 4V' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R28 (R 850 R, R 1150 R, Rockster)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle 4V' AND userId IS NULL)
WHERE userId IS NULL AND name = 'R28 (R 850 R, R 1150 R, Rockster)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle 4V' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT '259 (R 850 RT, R 1100 RT)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle 4V' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = '259 (R 850 RT, R 1100 RT)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle 4V' AND userId IS NULL)
WHERE userId IS NULL AND name = '259 (R 850 RT, R 1100 RT)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle 4V' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'R22 (R 850 RT, R 1150 RT, R 1150 RS)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle 4V' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R22 (R 850 RT, R 1150 RT, R 1150 RS)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle 4V' AND userId IS NULL)
WHERE userId IS NULL AND name = 'R22 (R 850 RT, R 1150 RT, R 1150 RS)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle 4V' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT '259 (R 1100 S, R 1100 RS)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle 4V' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = '259 (R 1100 S, R 1100 RS)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle 4V' AND userId IS NULL)
WHERE userId IS NULL AND name = '259 (R 1100 S, R 1100 RS)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle 4V' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT '259C (R 850 C, R 1200 C)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle 4V' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = '259C (R 850 C, R 1200 C)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle 4V' AND userId IS NULL)
WHERE userId IS NULL AND name = '259C (R 850 C, R 1200 C)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle 4V' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT '259C (R 1200 C Montauk)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle 4V' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = '259C (R 1200 C Montauk)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle 4V' AND userId IS NULL)
WHERE userId IS NULL AND name = '259C (R 1200 C Montauk)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle 4V' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT '259C (R 1200 C Independent)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle 4V' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = '259C (R 1200 C Independent)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle 4V' AND userId IS NULL)
WHERE userId IS NULL AND name = '259C (R 1200 C Independent)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle 4V' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'K30 (R1200 CL)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle 4V' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K30 (R1200 CL)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle 4V' AND userId IS NULL)
WHERE userId IS NULL AND name = 'K30 (R1200 CL)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle 4V' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'K25 (R 1200 GS)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle K2x' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K25 (R 1200 GS)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle K2x' AND userId IS NULL)
WHERE userId IS NULL AND name = 'K25 (R 1200 GS)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle K2x' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'K25 (R 1200 GS Adventure)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle K2x' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K25 (R 1200 GS Adventure)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle K2x' AND userId IS NULL)
WHERE userId IS NULL AND name = 'K25 (R 1200 GS Adventure)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle K2x' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'K25 (HP)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle K2x' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K25 (HP)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle K2x' AND userId IS NULL)
WHERE userId IS NULL AND name = 'K25 (HP)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle K2x' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'K26 (R 900 RT, R 1200 RT)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle K2x' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K26 (R 900 RT, R 1200 RT)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle K2x' AND userId IS NULL)
WHERE userId IS NULL AND name = 'K26 (R 900 RT, R 1200 RT)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle K2x' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'K27 (R 1200 R)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle K2x' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K27 (R 1200 R)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle K2x' AND userId IS NULL)
WHERE userId IS NULL AND name = 'K27 (R 1200 R)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle K2x' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'K28 (R 1200 ST)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle K2x' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K28 (R 1200 ST)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle K2x' AND userId IS NULL)
WHERE userId IS NULL AND name = 'K28 (R 1200 ST)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle K2x' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'K29 (R 1200 S, HP2 Sport)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle K2x' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K29 (R 1200 S, HP2 Sport)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle K2x' AND userId IS NULL)
WHERE userId IS NULL AND name = 'K29 (R 1200 S, HP2 Sport)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle K2x' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'K50 (R 1200 GS)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle K5x' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K50 (R 1200 GS)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle K5x' AND userId IS NULL)
WHERE userId IS NULL AND name = 'K50 (R 1200 GS)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle K5x' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'K51 (R 1200 GS Adventure)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle K5x' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K51 (R 1200 GS Adventure)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle K5x' AND userId IS NULL)
WHERE userId IS NULL AND name = 'K51 (R 1200 GS Adventure)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle K5x' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'K52 (R 1200 RT)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle K5x' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K52 (R 1200 RT)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle K5x' AND userId IS NULL)
WHERE userId IS NULL AND name = 'K52 (R 1200 RT)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle K5x' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'K53 (R 1200 R)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle K5x' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K53 (R 1200 R)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle K5x' AND userId IS NULL)
WHERE userId IS NULL AND name = 'K53 (R 1200 R)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle K5x' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'K54 (R 1200 RS)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle K5x' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K54 (R 1200 RS)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle K5x' AND userId IS NULL)
WHERE userId IS NULL AND name = 'K54 (R 1200 RS)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle K5x' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'K21 (R nineT)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle R nineT' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K21 (R nineT)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle R nineT' AND userId IS NULL)
WHERE userId IS NULL AND name = 'K21 (R nineT)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle R nineT' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'K22 (R nineT Pure)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle R nineT' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K22 (R nineT Pure)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle R nineT' AND userId IS NULL)
WHERE userId IS NULL AND name = 'K22 (R nineT Pure)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle R nineT' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'K23 (R nineT Scrambler)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle R nineT' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K23 (R nineT Scrambler)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle R nineT' AND userId IS NULL)
WHERE userId IS NULL AND name = 'K23 (R nineT Scrambler)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle R nineT' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'K32 (R nineT Racer)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle R nineT' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K32 (R nineT Racer)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle R nineT' AND userId IS NULL)
WHERE userId IS NULL AND name = 'K32 (R nineT Racer)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle R nineT' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'K33 (R nineT Urban G/S)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'R-Modelle R nineT' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K33 (R nineT Urban G/S)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Modelle R nineT' AND userId IS NULL)
WHERE userId IS NULL AND name = 'K33 (R nineT Urban G/S)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'R-Modelle R nineT' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'K569 (K 75, K 75 c, K 75 s, K 75 RT)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'K-Modelle 3-Zyl.' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K569 (K 75, K 75 c, K 75 s, K 75 RT)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'K-Modelle 3-Zyl.' AND userId IS NULL)
WHERE userId IS NULL AND name = 'K569 (K 75, K 75 c, K 75 s, K 75 RT)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'K-Modelle 3-Zyl.' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'K589 (K100, RS, RT, LT)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'K-Modelle 4-Zyl. 2V' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K589 (K100, RS, RT, LT)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'K-Modelle 4-Zyl. 2V' AND userId IS NULL)
WHERE userId IS NULL AND name = 'K589 (K100, RS, RT, LT)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'K-Modelle 4-Zyl. 2V' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'K589 (K1, K 100 RS)', 'BMW', id, '0080001-0090000,6365002-6365406', '0533,0535' FROM modelSeries
WHERE name = 'K-Modelle 4-Zyl. 4V' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K589 (K1, K 100 RS)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'K-Modelle 4-Zyl. 4V' AND userId IS NULL), frameRanges = '0080001-0090000,6365002-6365406', typeCodes = '0533,0535'
WHERE userId IS NULL AND name = 'K589 (K1, K 100 RS)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'K-Modelle 4-Zyl. 4V' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'K589 (K 1100 RS, K 1100 LT)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'K-Modelle 4-Zyl. 4V' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K589 (K 1100 RS, K 1100 LT)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'K-Modelle 4-Zyl. 4V' AND userId IS NULL)
WHERE userId IS NULL AND name = 'K589 (K 1100 RS, K 1100 LT)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'K-Modelle 4-Zyl. 4V' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'K589 (K 1200 RS, K 1200 LT)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'K-Modelle 4-Zyl. 4V' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K589 (K 1200 RS, K 1200 LT)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'K-Modelle 4-Zyl. 4V' AND userId IS NULL)
WHERE userId IS NULL AND name = 'K589 (K 1200 RS, K 1200 LT)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'K-Modelle 4-Zyl. 4V' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'K41 (K 1200 GT, K 1200 RS)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'K-Modelle 4-Zyl. 4V' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K41 (K 1200 GT, K 1200 RS)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'K-Modelle 4-Zyl. 4V' AND userId IS NULL)
WHERE userId IS NULL AND name = 'K41 (K 1200 GT, K 1200 RS)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'K-Modelle 4-Zyl. 4V' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'K40 (K 1200 S, K 1300 S)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'K-Modelle K4x 4-Zyl.' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K40 (K 1200 S, K 1300 S)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'K-Modelle K4x 4-Zyl.' AND userId IS NULL)
WHERE userId IS NULL AND name = 'K40 (K 1200 S, K 1300 S)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'K-Modelle K4x 4-Zyl.' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'K43 (K 1200 R, Sport, K 1300 R)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'K-Modelle K4x 4-Zyl.' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K43 (K 1200 R, Sport, K 1300 R)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'K-Modelle K4x 4-Zyl.' AND userId IS NULL)
WHERE userId IS NULL AND name = 'K43 (K 1200 R, Sport, K 1300 R)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'K-Modelle K4x 4-Zyl.' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, frameRanges, typeCodes)
SELECT 'K44 (K 1200 GT, K 1300 GT)', 'BMW', id, NULL, NULL FROM modelSeries
WHERE name = 'K-Modelle K4x 4-Zyl.' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K44 (K 1200 GT, K 1300 GT)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'K-Modelle K4x 4-Zyl.' AND userId IS NULL)
WHERE userId IS NULL AND name = 'K44 (K 1200 GT, K 1300 GT)' AND parentId IS NOT (SELECT id FROM modelSeries WHERE name = 'K-Modelle K4x 4-Zyl.' AND userId IS NULL);

-- Explicit metadata sync (covers renamed rows whose parent did not change)
UPDATE modelSeries SET frameRanges = '200009-212007' WHERE userId IS NULL AND name = 'R 24 -50';
UPDATE modelSeries SET frameRanges = '220011-243410,245001-283650,284001-331705' WHERE userId IS NULL AND name = 'R 25 -56';
UPDATE modelSeries SET frameRanges = '340001-370242' WHERE userId IS NULL AND name = 'R 26 -60';
UPDATE modelSeries SET frameRanges = '372001-387566' WHERE userId IS NULL AND name = 'R 27 -66';
UPDATE modelSeries SET frameRanges = '550001-563515,564001-565634,630001-649037' WHERE userId IS NULL AND name = 'R 50 -69';
UPDATE modelSeries SET frameRanges = '516001-521005,522001-540950' WHERE userId IS NULL AND name = 'R 51 -54';
UPDATE modelSeries SET frameRanges = '618001-621530,622001-629999,1810001-1819307' WHERE userId IS NULL AND name = 'R 60 -69';
UPDATE modelSeries SET frameRanges = '610001-611445,612001-617700' WHERE userId IS NULL AND name = 'R 67 -55';
UPDATE modelSeries SET frameRanges = '650001-651453' WHERE userId IS NULL AND name = 'R 68 -54';
UPDATE modelSeries SET frameRanges = '652001-654955,655004-666320' WHERE userId IS NULL AND name = 'R 69 -69';
UPDATE modelSeries SET frameRanges = '2900001-2910000,2930001-2950000,2970001-3000000,4900001-4947578,4040001-4100000,4950001-4991260', typeCodes = '0240,0250,0251,0255,0281,0260,0253,0257,0291,0261,0265,0282,0263,0267,0292,0271,0275,0283,0273,0277,0293,0272,0276,0284,0274,0278,0294' WHERE userId IS NULL AND name = 'R50/5-R90S 69-76';
UPDATE modelSeries SET typeCodes = '0351,0355,0352,0354,0353,0358,0363,0364,0359,0365,0360' WHERE userId IS NULL AND name = 'R45-R65LS..78-85';
UPDATE modelSeries SET frameRanges = '6000001-6012000,6015001-6016000,6100001-6102000,6020001-6025000,6220000-6223000,6120001-6125000,6126001-6128000', typeCodes = '0301,0321,0371,0311,0331,0341,0381,0302,0326,0372,0312,0382,0332,0392,0327,0373,0423,0322,0374,0342,0383,0443,0343,0384' WHERE userId IS NULL AND name = 'R 60, R 75 , R 80, /7, RT (76-85)';
UPDATE modelSeries SET frameRanges = '6040001-6054000,6035001-6040000,6075001-6080000,6135001-6140000,6193001-6195000', typeCodes = '0304,0323,0375,0425,0314,0333,0393,0435,0344,0385,0445,0398,0305,0324,0376,0315,0334,0394,0426,0436,0306,0325,0377,0427,0316,0335,0395,0437,0345,0386,0339' WHERE userId IS NULL AND name = 'R 100, /7, /T, CS, RS, RT, S (76-84)';
UPDATE modelSeries SET frameRanges = '6250001-6260000,6281001-6290000,6291001-6292600,6245001-6247000,6054001-6060000,6125001-6126000', typeCodes = '0346,0471,0348,0347,0349,0482' WHERE userId IS NULL AND name = 'R 65 GS, R 80 G/S, R 80 ST (80-92)';
UPDATE modelSeries SET frameRanges = '6276001-6280000,6331001-6336000' WHERE userId IS NULL AND name = 'R 80 GS, R 100 GS, PD (87-90)';
UPDATE modelSeries SET frameRanges = '0080001-0090000,6365002-6365406', typeCodes = '0533,0535' WHERE userId IS NULL AND name = 'K589 (K1, K 100 RS)';

-- Metadata housekeeping on survivors --------------------------------------
-- 87-90 GS blocks moved off the 90-95 Serie (codes stay).
UPDATE modelSeries SET frameRanges = NULL WHERE userId IS NULL AND name = 'R 80 GS, R 100 GS, PD (90-95)';
-- K1 metadata moved from the family onto 'K589 (K1, K 100 RS)'.
UPDATE modelSeries SET frameRanges = NULL, typeCodes = NULL WHERE userId IS NULL AND name = 'K-Modelle 4-Zyl. 4V';

-- 4) Modelle ---------------------------------------------------------------
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R24 (ECE, 01/1949-02/1950)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 24 -50' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R24 (ECE, 01/1949-02/1950)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R25 (ECE, 04/1950-09/1951)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 25 -56' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R25 (ECE, 04/1950-09/1951)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R25/2 (ECE, 10/1951-08/1953)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 25 -56' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R25/2 (ECE, 10/1951-08/1953)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R25/3 (ECE, 09/1953-07/1955)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 25 -56' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R25/3 (ECE, 09/1953-07/1955)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R26 (ECE, 11/1955-07/1960)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 26 -60' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R26 (ECE, 11/1955-07/1960)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R27 (ECE, 10/1960-10/1966)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 27 -66' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R27 (ECE, 10/1960-10/1966)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R50 (ECE, 03/1955-09/1960)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 50 -69' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R50 (ECE, 03/1955-09/1960)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R50 S (ECE, 01/1961-10/1962)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 50 -69' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R50 S (ECE, 01/1961-10/1962)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R50/2 (ECE, 01/1961-12/1969)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 50 -69' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R50/2 (ECE, 01/1961-12/1969)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R50/2 (USA, 08/1967-08/1969)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 50 -69' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R50/2 (USA, 08/1967-08/1969)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R51/2 (ECE, 01/1950-12/1950)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 51 -54' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R51/2 (ECE, 01/1950-12/1950)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R51/3 (ECE, 12/1950-07/1954)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 51 -54' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R51/3 (ECE, 12/1950-07/1954)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R60 (ECE, 04/1956-07/1960)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 60 -69' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R60 (ECE, 04/1956-07/1960)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R60/2 (ECE, 09/1960-11/1967)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 60 -69' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R60/2 (ECE, 09/1960-11/1967)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R60/2 (USA, 01/1967-XX)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 60 -69' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R60/2 (USA, 01/1967-XX)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R67 (ECE, 01/1951-10/1951)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 67 -55' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R67 (ECE, 01/1951-10/1951)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R67/2/3 (ECE, 12/1951-12/1955)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 67 -55' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R67/2/3 (ECE, 12/1951-12/1955)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R68 (ECE, 03/1952-07/1954)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 68 -54' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R68 (ECE, 03/1952-07/1954)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R69 (ECE, 04/1955-07/1960)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 69 -69' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R69 (ECE, 04/1955-07/1960)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R69 S (ECE, 10/1960-07/1969)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 69 -69' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R69 S (ECE, 10/1960-07/1969)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R69 S (USA, 01/1967-12/1969)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 69 -69' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R69 S (USA, 01/1967-12/1969)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R50/5 (ECE, 08/1969-07/1973)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R50/5-R90S 69-76' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R50/5 (ECE, 08/1969-07/1973)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R60/5 (ECE, 08/1969-07/1973)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R50/5-R90S 69-76' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R60/5 (ECE, 08/1969-07/1973)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R60/6 (ECE, 07/1973-08/1974)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R50/5-R90S 69-76' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R60/6 (ECE, 07/1973-08/1974)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R60/6 (ECE, 08/1974-08/1975)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R50/5-R90S 69-76' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R60/6 (ECE, 08/1974-08/1975)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R60/6 (ECE, 08/1975-06/1976)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R50/5-R90S 69-76' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R60/6 (ECE, 08/1975-06/1976)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R60/6 (USA, 01/1974-07/1974)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R50/5-R90S 69-76' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R60/6 (USA, 01/1974-07/1974)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R60/6 (USA, 08/1974-07/1975)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R50/5-R90S 69-76' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R60/6 (USA, 08/1974-07/1975)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R60/6 (USA, 09/1975-05/1976)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R50/5-R90S 69-76' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R60/6 (USA, 09/1975-05/1976)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R75/5 (ECE, 08/1969-07/1980)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R50/5-R90S 69-76' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R75/5 (ECE, 08/1969-07/1980)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R75/6 (ECE, 08/1974-08/1975)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R50/5-R90S 69-76' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R75/6 (ECE, 08/1974-08/1975)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R75/6 (ECE, 08/1975-06/1976)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R50/5-R90S 69-76' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R75/6 (ECE, 08/1975-06/1976)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R75/6 (ECE, 09/1973-08/1974)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R50/5-R90S 69-76' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R75/6 (ECE, 09/1973-08/1974)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R75/6 (USA, 01/1974-07/1974)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R50/5-R90S 69-76' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R75/6 (USA, 01/1974-07/1974)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R75/6 (USA, 08/1974-08/1975)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R50/5-R90S 69-76' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R75/6 (USA, 08/1974-08/1975)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R75/6 (USA, 09/1975-06/1976)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R50/5-R90S 69-76' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R75/6 (USA, 09/1975-06/1976)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R90/6 (ECE, 06/1974-08/1975)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R50/5-R90S 69-76' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R90/6 (ECE, 06/1974-08/1975)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R90/6 (ECE, 08/1975-06/1976)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R50/5-R90S 69-76' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R90/6 (ECE, 08/1975-06/1976)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R90/6 (ECE, 09/1973-08/1974)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R50/5-R90S 69-76' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R90/6 (ECE, 09/1973-08/1974)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R90/6 (USA, 01/1974-07/1974)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R50/5-R90S 69-76' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R90/6 (USA, 01/1974-07/1974)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R90/6 (USA, 08/1974-08/1975)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R50/5-R90S 69-76' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R90/6 (USA, 08/1974-08/1975)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R90/6 (USA, 09/1975-06/1976)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R50/5-R90S 69-76' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R90/6 (USA, 09/1975-06/1976)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R90S (ECE, 06/1974-09/1975)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R50/5-R90S 69-76' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R90S (ECE, 06/1974-09/1975)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R90S (ECE, 08/1975-06/1976)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R50/5-R90S 69-76' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R90S (ECE, 08/1975-06/1976)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R90S (ECE, 09/1973-08/1974)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R50/5-R90S 69-76' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R90S (ECE, 09/1973-08/1974)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R90S (USA, 01/1974-07/1974)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R50/5-R90S 69-76' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R90S (USA, 01/1974-07/1974)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R90S (USA, 07/1974-08/1975)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R50/5-R90S 69-76' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R90S (USA, 07/1974-08/1975)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R90S (USA, 08/1975-06/1976)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R50/5-R90S 69-76' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R90S (USA, 08/1975-06/1976)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R45 (ECE, 01/1978-07/1980)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R45-R65LS..78-85' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R45 (ECE, 01/1978-07/1980)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R45 (ECE, 06/1980-07/1985)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R45-R65LS..78-85' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R45 (ECE, 06/1980-07/1985)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R45/N (ECE, 03/1978-07/1980)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R45-R65LS..78-85' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R45/N (ECE, 03/1978-07/1980)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R45/N (ECE, 08/1980-07/1985)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R45-R65LS..78-85' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R45/N (ECE, 08/1980-07/1985)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R45T (ECE, 10/1980-06/1985)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R45-R65LS..78-85' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R45T (ECE, 10/1980-06/1985)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R45T/N (ECE, 09/1980-07/1985)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R45-R65LS..78-85' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R45T/N (ECE, 09/1980-07/1985)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R65 (ECE, 01/1978-07/1980)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R45-R65LS..78-85' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R65 (ECE, 01/1978-07/1980)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R65 (ECE, 06/1980-07/1985)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R45-R65LS..78-85' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R65 (ECE, 06/1980-07/1985)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R65 (USA, 06/1980-04/1985)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R45-R65LS..78-85' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R65 (USA, 06/1980-04/1985)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R65 (USA, 07/1978-07/1980)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R45-R65LS..78-85' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R65 (USA, 07/1978-07/1980)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R65LS (ECE, 03/1981-03/1985)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R45-R65LS..78-85' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R65LS (ECE, 03/1981-03/1985)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R65LS (USA, 06/1981-02/1985)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R45-R65LS..78-85' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R65LS (USA, 06/1981-02/1985)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R65T (ECE, 09/1980-07/1985)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R45-R65LS..78-85' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R65T (ECE, 09/1980-07/1985)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 60 /7 (ECE, 05/1976-06/1977)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 60, R 75 , R 80, /7, RT (76-85)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 60 /7 (ECE, 05/1976-06/1977)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 60 /7 (ECE, 07/1977-07/1978)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 60, R 75 , R 80, /7, RT (76-85)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 60 /7 (ECE, 07/1977-07/1978)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 60 /7 (ECE, 07/1978-07/1978)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 60, R 75 , R 80, /7, RT (76-85)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 60 /7 (ECE, 07/1978-07/1978)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 60 /7 (ECE, 09/1978-04/1980)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 60, R 75 , R 80, /7, RT (76-85)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 60 /7 (ECE, 09/1978-04/1980)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 60 /7 (USA, 07/1976-05/1977)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 60, R 75 , R 80, /7, RT (76-85)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 60 /7 (USA, 07/1976-05/1977)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 60 /7 (USA, 08/1977-11/1977)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 60, R 75 , R 80, /7, RT (76-85)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 60 /7 (USA, 08/1977-11/1977)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 60 TIC (ECE, 09/1978-11/1982)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 60, R 75 , R 80, /7, RT (76-85)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 60 TIC (ECE, 09/1978-11/1982)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 75 /7 (ECE, 01/1979-01/1979)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 60, R 75 , R 80, /7, RT (76-85)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 75 /7 (ECE, 01/1979-01/1979)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 75 /7 (ECE, 05/1976-06/1977)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 60, R 75 , R 80, /7, RT (76-85)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 75 /7 (ECE, 05/1976-06/1977)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 75 /7 (ECE, 08/1977-04/1978)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 60, R 75 , R 80, /7, RT (76-85)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 75 /7 (ECE, 08/1977-04/1978)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 75 /7 (USA, 07/1976-06/1977)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 60, R 75 , R 80, /7, RT (76-85)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 75 /7 (USA, 07/1976-06/1977)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 80 RT (ECE, 06/1982-11/1984)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 60, R 75 , R 80, /7, RT (76-85)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80 RT (ECE, 06/1982-11/1984)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 80 RT (USA, 08/1982-10/1984)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 60, R 75 , R 80, /7, RT (76-85)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80 RT (USA, 08/1982-10/1984)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 80 TIC (ECE, 06/1978-07/1978)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 60, R 75 , R 80, /7, RT (76-85)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80 TIC (ECE, 06/1978-07/1978)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 80 TIC (ECE, 06/1978-08/1978)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 60, R 75 , R 80, /7, RT (76-85)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80 TIC (ECE, 06/1978-08/1978)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 80 TIC (ECE, 09/1978-07/1980)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 60, R 75 , R 80, /7, RT (76-85)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80 TIC (ECE, 09/1978-07/1980)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 80 TIC (ECE, 09/1980-01/1985)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 60, R 75 , R 80, /7, RT (76-85)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80 TIC (ECE, 09/1980-01/1985)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 80, R 80 /7 (ECE, 04/1977-07/1978)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 60, R 75 , R 80, /7, RT (76-85)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80, R 80 /7 (ECE, 04/1977-07/1978)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 80, R 80 /7 (ECE, 06/1978-07/1980)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 60, R 75 , R 80, /7, RT (76-85)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80, R 80 /7 (ECE, 06/1978-07/1980)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 80, R 80 /7 (ECE, 08/1977-07/1978)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 60, R 75 , R 80, /7, RT (76-85)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80, R 80 /7 (ECE, 08/1977-07/1978)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 80, R 80 /7 (ECE, 09/1980-11/1984)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 60, R 75 , R 80, /7, RT (76-85)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80, R 80 /7 (ECE, 09/1980-11/1984)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 80, R 80 /7 (USA, 04/1977-07/1978)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 60, R 75 , R 80, /7, RT (76-85)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80, R 80 /7 (USA, 04/1977-07/1978)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 80, R 80 /7 (USA, 08/1978-07/1980)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 60, R 75 , R 80, /7, RT (76-85)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80, R 80 /7 (USA, 08/1978-07/1980)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 65 GS (ECE, 11/1987-11/1992)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 65 GS, R 80 G/S, R 80 ST (80-92)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 65 GS (ECE, 11/1987-11/1992)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 80 G/S (ECE, 05/1980-07/1987)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 65 GS, R 80 G/S, R 80 ST (80-92)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80 G/S (ECE, 05/1980-07/1987)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 80 G/S (USA, 06/1980-03/1987)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 65 GS, R 80 G/S, R 80 ST (80-92)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80 G/S (USA, 06/1980-03/1987)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 80 ST (ECE, 04/1982-10/1984)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 65 GS, R 80 G/S, R 80 ST (80-92)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80 ST (ECE, 04/1982-10/1984)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 80 ST (USA, 10/1982-10/1984)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 65 GS, R 80 G/S, R 80 ST (80-92)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80 ST (USA, 10/1982-10/1984)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 65 (20KW) (ECE, 04/1985-06/1993)', 'BMW', id, '20KW' FROM modelSeries
WHERE name = 'R 65, R 65 RT, R 80, R 80 RT (85-95)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 65 (20KW) (ECE, 04/1985-06/1993)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 65 (35KW) (ECE, 06/1985-10/1988)', 'BMW', id, '35KW' FROM modelSeries
WHERE name = 'R 65, R 65 RT, R 80, R 80 RT (85-95)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 65 (35KW) (ECE, 06/1985-10/1988)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 65 (USA, 07/1985-05/1987)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 65, R 65 RT, R 80, R 80 RT (85-95)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 65 (USA, 07/1985-05/1987)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 65 RT SF (ECE, 07/1985-11/1988)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 65, R 65 RT, R 80, R 80 RT (85-95)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 65 RT SF (ECE, 07/1985-11/1988)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 80 (ECE, 03/1984-01/1995)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 65, R 65 RT, R 80, R 80 RT (85-95)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80 (ECE, 03/1984-01/1995)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 80 (USA, 07/1984-07/1987)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 65, R 65 RT, R 80, R 80 RT (85-95)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80 (USA, 07/1984-07/1987)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 80 RT (ECE, 07/1984-12/1995)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 65, R 65 RT, R 80, R 80 RT (85-95)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80 RT (ECE, 07/1984-12/1995)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 80 RT (USA, 07/1984-04/1987)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 65, R 65 RT, R 80, R 80 RT (85-95)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80 RT (USA, 07/1984-04/1987)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 GS (ECE, 12/1986-07/1990)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 80 GS, R 100 GS, PD (87-90)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 GS (ECE, 12/1986-07/1990)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 GS (USA, 01/1987-03/1990)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 80 GS, R 100 GS, PD (87-90)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 GS (USA, 01/1987-03/1990)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 GS PD (ECE, 02/1989-07/1990)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 80 GS, R 100 GS, PD (87-90)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 GS PD (ECE, 02/1989-07/1990)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 GS PD (USA, 06/1989-04/1990)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 80 GS, R 100 GS, PD (87-90)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 GS PD (USA, 06/1989-04/1990)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 80 GS (ECE, 01/1987-07/1990)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 80 GS, R 100 GS, PD (87-90)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80 GS (ECE, 01/1987-07/1990)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 80 GS PD (ECE, 06/1990-07/1990)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 80 GS, R 100 GS, PD (87-90)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80 GS PD (ECE, 06/1990-07/1990)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 GS (ECE, 04/1990-07/1994)', 'BMW', id, '0473' FROM modelSeries
WHERE name = 'R 80 GS, R 100 GS, PD (90-95)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 GS (ECE, 04/1990-07/1994)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 GS (USA, 11/1990-09/1994)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 80 GS, R 100 GS, PD (90-95)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 GS (USA, 11/1990-09/1994)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 GS PD (ECE, 08/1990-02/1996)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 80 GS, R 100 GS, PD (90-95)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 GS PD (ECE, 08/1990-02/1996)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 GS PD (USA, 09/1990-12/1995)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 80 GS, R 100 GS, PD (90-95)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 GS PD (USA, 09/1990-12/1995)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 80 GS (ECE, 04/1990-10/1995)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 80 GS, R 100 GS, PD (90-95)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80 GS (ECE, 04/1990-10/1995)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 80 GS PD (CH) (ECE, 08/1990-06/1995)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 80 GS, R 100 GS, PD (90-95)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80 GS PD (CH) (ECE, 08/1990-06/1995)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 80 GS (ECE, 01/1996-12/1996)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 80 GS Basic (96)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80 GS (ECE, 01/1996-12/1996)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 R 91 (ECE, 03/1991-12/1995)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 80 R, R 100 R, Mystik (91-95)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 R 91 (ECE, 03/1991-12/1995)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 R 91 (USA, 09/1991-12/1995)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 80 R, R 100 R, Mystik (91-95)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 R 91 (USA, 09/1991-12/1995)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 R Mystik 94 (ECE, 12/1993-01/1996)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 80 R, R 100 R, Mystik (91-95)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 R Mystik 94 (ECE, 12/1993-01/1996)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 R Mystik 94 (USA, 03/1994-09/1995)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 80 R, R 100 R, Mystik (91-95)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 R Mystik 94 (USA, 03/1994-09/1995)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 80 R 91 (ECE, 03/1991-06/1994)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 80 R, R 100 R, Mystik (91-95)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80 R 91 (ECE, 03/1991-06/1994)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 80 R Mystik 94 (ECE, 03/1994-03/1995)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 80 R, R 100 R, Mystik (91-95)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80 R Mystik 94 (ECE, 03/1994-03/1995)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 /7 (ECE, 05/1976-06/1977)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 /7 (ECE, 05/1976-06/1977)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 /7T (ECE, 03/1978-07/1978)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 /7T (ECE, 03/1978-07/1978)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 /7T (ECE, 04/1977-08/1978)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 /7T (ECE, 04/1977-08/1978)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 /7T (ECE, 06/1978-07/1980)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 /7T (ECE, 06/1978-07/1980)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 /7T (ECE, 06/1980-10/1984)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 /7T (ECE, 06/1980-10/1984)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 /7T (ECE, 07/1978-07/1978)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 /7T (ECE, 07/1978-07/1978)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 /7T (ECE, 11/1978-07/1980)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 /7T (ECE, 11/1978-07/1980)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 /7T (USA, 05/1976-06/1977)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 /7T (USA, 05/1976-06/1977)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 /7T (USA, 07/1977-07/1978)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 /7T (USA, 07/1977-07/1978)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 /T (USA, 09/1978-07/1980)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 /T (USA, 09/1978-07/1980)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 /T (USA, 09/1980-09/1984)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 /T (USA, 09/1980-09/1984)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 CS (ECE, 06/1980-10/1984)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 CS (ECE, 06/1980-10/1984)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 CS (USA, 09/1980-09/1984)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 CS (USA, 09/1980-09/1984)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 RS (ECE, 03/1976-06/1977)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 RS (ECE, 03/1976-06/1977)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 RS (ECE, 04/1977-07/1978)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 RS (ECE, 04/1977-07/1978)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 RS (ECE, 06/1978-07/1980)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 RS (ECE, 06/1978-07/1980)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 RS (ECE, 06/1980-11/1984)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 RS (ECE, 06/1980-11/1984)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 RS (USA, 05/1976-06/1977)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 RS (USA, 05/1976-06/1977)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 RS (USA, 08/1978-06/1980)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 RS (USA, 08/1978-06/1980)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 RS (USA, 09/1980-10/1984)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 RS (USA, 09/1980-10/1984)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 RT (ECE, 06/1978-07/1980)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 RT (ECE, 06/1978-07/1980)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 RT (ECE, 08/1980-10/1984)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 RT (ECE, 08/1980-10/1984)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 RT (USA, 04/1978-04/1978)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 RT (USA, 04/1978-04/1978)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 RT (USA, 06/1980-09/1984)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 RT (USA, 06/1980-09/1984)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 RT (USA, 08/1978-07/1980)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 RT (USA, 08/1978-07/1980)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 S (ECE, 04/1977-07/1978)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 S (ECE, 04/1977-07/1978)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 S (ECE, 05/1976-06/1977)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 S (ECE, 05/1976-06/1977)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 S (ECE, 06/1978-07/1980)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 S (ECE, 06/1978-07/1980)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 S (USA, 05/1976-06/1977)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 S (USA, 05/1976-06/1977)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 S (USA, 07/1977-07/1978)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 S (USA, 07/1977-07/1978)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 S (USA, 08/1978-07/1980)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 S (USA, 08/1978-07/1980)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 TIC (ECE, 11/1980-09/1984)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 TIC (ECE, 11/1980-09/1984)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 RS (ECE, 07/1986-10/1992)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100 RS, R 100 RT (87-95)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 RS (ECE, 07/1986-10/1992)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 RS (USA, 08/1987-08/1992)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100 RS, R 100 RT (87-95)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 RS (USA, 08/1987-08/1992)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 RT (ECE, 07/1987-12/1995)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100 RS, R 100 RT (87-95)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 RT (ECE, 07/1987-12/1995)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 100 RT (USA, 08/1987-12/1995)', 'BMW', id, NULL FROM modelSeries
WHERE name = 'R 100 RS, R 100 RT (87-95)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100 RT (USA, 08/1987-12/1995)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1100 GS 94 (0404,0409) (ECE, 04/1993-12/1999)', 'BMW', id, '0404,0409' FROM modelSeries
WHERE name = '259 (R 850 GS, R 1100 GS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1100 GS 94 (0404,0409) (ECE, 04/1993-12/1999)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1100 GS 94 (0404,0409) (USA, 02/1994-07/2006)', 'BMW', id, '0404,0409' FROM modelSeries
WHERE name = '259 (R 850 GS, R 1100 GS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1100 GS 94 (0404,0409) (USA, 02/1994-07/2006)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 850 GS 95 (0403) (ECE, 10/1996-07/2000)', 'BMW', id, '0403' FROM modelSeries
WHERE name = '259 (R 850 GS, R 1100 GS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 850 GS 95 (0403) (ECE, 10/1996-07/2000)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1150 GS 00 (0415,0495) (ECE, 09/1998-11/2003)', 'BMW', id, '0415,0495' FROM modelSeries
WHERE name = 'R21 (R 1150 GS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1150 GS 00 (0415,0495) (ECE, 09/1998-11/2003)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1150 GS 00 (0415,0495) (USA, 09/1998-11/2003)', 'BMW', id, '0415,0495' FROM modelSeries
WHERE name = 'R21 (R 1150 GS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1150 GS 00 (0415,0495) (USA, 09/1998-11/2003)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1150 GS Adv. 01 (0441,0492) (ECE, 07/2001-09/2005)', 'BMW', id, '0441,0492' FROM modelSeries
WHERE name = 'R21 (R 1150 GS Adventure)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1150 GS Adv. 01 (0441,0492) (ECE, 07/2001-09/2005)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1150 GS Adv. 01 (0441,0492) (USA, 07/2001-09/2005)', 'BMW', id, '0441,0492' FROM modelSeries
WHERE name = 'R21 (R 1150 GS Adventure)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1150 GS Adv. 01 (0441,0492) (USA, 07/2001-09/2005)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1100 R 94 (0402,0407) (ECE, 09/1993-04/2001)', 'BMW', id, '0402,0407' FROM modelSeries
WHERE name = '259R (R 850 R, R 1100 R)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1100 R 94 (0402,0407) (ECE, 09/1993-04/2001)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1100 R 94 (0402,0407) (USA, 09/1994-11/2000)', 'BMW', id, '0402,0407' FROM modelSeries
WHERE name = '259R (R 850 R, R 1100 R)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1100 R 94 (0402,0407) (USA, 09/1994-11/2000)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 850 R 94 (0401,0406) (ECE, 03/1994-11/2002)', 'BMW', id, '0401,0406' FROM modelSeries
WHERE name = '259R (R 850 R, R 1100 R)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 850 R 94 (0401,0406) (ECE, 03/1994-11/2002)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 850 R 94 (0401,0406) (USA, 08/1995-04/1997)', 'BMW', id, '0401,0406' FROM modelSeries
WHERE name = '259R (R 850 R, R 1100 R)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 850 R 94 (0401,0406) (USA, 08/1995-04/1997)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1150 R 01 (0429,0439) (ECE, 11/1999-06/2006)', 'BMW', id, '0429,0439' FROM modelSeries
WHERE name = 'R28 (R 850 R, R 1150 R, Rockster)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1150 R 01 (0429,0439) (ECE, 11/1999-06/2006)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1150 R 01 (0429,0439) (USA, 03/2000-02/2006)', 'BMW', id, '0429,0439' FROM modelSeries
WHERE name = 'R28 (R 850 R, R 1150 R, Rockster)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1150 R 01 (0429,0439) (USA, 03/2000-02/2006)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1150 R Rockster (0308,0318) (ECE, 03/2002-07/2005)', 'BMW', id, '0308,0318' FROM modelSeries
WHERE name = 'R28 (R 850 R, R 1150 R, Rockster)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1150 R Rockster (0308,0318) (ECE, 03/2002-07/2005)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1150 R Rockster (0308,0318) (USA, 03/2002-07/2005)', 'BMW', id, '0308,0318' FROM modelSeries
WHERE name = 'R28 (R 850 R, R 1150 R, Rockster)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1150 R Rockster (0308,0318) (USA, 03/2002-07/2005)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 850 R 02 (0428) (ECE, 11/1999-07/2007)', 'BMW', id, '0428' FROM modelSeries
WHERE name = 'R28 (R 850 R, R 1150 R, Rockster)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 850 R 02 (0428) (ECE, 11/1999-07/2007)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1100 RT 96 (0413,0418) (ECE, 05/1994-08/2001)', 'BMW', id, '0413,0418' FROM modelSeries
WHERE name = '259 (R 850 RT, R 1100 RT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1100 RT 96 (0413,0418) (ECE, 05/1994-08/2001)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1100 RT 96 (0413,0418) (USA, 09/1994-07/2001)', 'BMW', id, '0413,0418' FROM modelSeries
WHERE name = '259 (R 850 RT, R 1100 RT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1100 RT 96 (0413,0418) (USA, 09/1994-07/2001)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 850 RT 96 (0412) (ECE, 03/1996-11/2001)', 'BMW', id, '0412' FROM modelSeries
WHERE name = '259 (R 850 RT, R 1100 RT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 850 RT 96 (0412) (ECE, 03/1996-11/2001)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1150 RS 01 (0447,0498) (ECE, 11/2000-11/2004)', 'BMW', id, '0447,0498' FROM modelSeries
WHERE name = 'R22 (R 850 RT, R 1150 RT, R 1150 RS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1150 RS 01 (0447,0498) (ECE, 11/2000-11/2004)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1150 RS 01 (0447,0498) (USA, 11/2000-01/2004)', 'BMW', id, '0447,0498' FROM modelSeries
WHERE name = 'R22 (R 850 RT, R 1150 RT, R 1150 RS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1150 RS 01 (0447,0498) (USA, 11/2000-01/2004)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1150 RT 00 (0419,0499) (ECE, 01/2000-03/2006)', 'BMW', id, '0419,0499' FROM modelSeries
WHERE name = 'R22 (R 850 RT, R 1150 RT, R 1150 RS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1150 RT 00 (0419,0499) (ECE, 01/2000-03/2006)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1150 RT 00 (0419,0499) (USA, 02/2000-02/2006)', 'BMW', id, '0419,0499' FROM modelSeries
WHERE name = 'R22 (R 850 RT, R 1150 RT, R 1150 RS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1150 RT 00 (0419,0499) (USA, 02/2000-02/2006)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 850 RT 02 (0417) (ECE, 06/2000-02/2006)', 'BMW', id, '0417' FROM modelSeries
WHERE name = 'R22 (R 850 RT, R 1150 RT, R 1150 RS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 850 RT 02 (0417) (ECE, 06/2000-02/2006)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1100 RS 93 (0411,0416) (ECE, 01/1992-06/2001)', 'BMW', id, '0411,0416' FROM modelSeries
WHERE name = '259 (R 1100 S, R 1100 RS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1100 RS 93 (0411,0416) (ECE, 01/1992-06/2001)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1100 RS 93 (0411,0416) (USA, 06/1992-02/2001)', 'BMW', id, '0411,0416' FROM modelSeries
WHERE name = '259 (R 1100 S, R 1100 RS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1100 RS 93 (0411,0416) (USA, 06/1992-02/2001)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1100 S 98 (0422,0432) (ECE, 12/1996-07/2005)', 'BMW', id, '0422,0432' FROM modelSeries
WHERE name = '259 (R 1100 S, R 1100 RS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1100 S 98 (0422,0432) (ECE, 12/1996-07/2005)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1100 S 98 (0422,0432) (USA, 01/1997-09/2004)', 'BMW', id, '0422,0432' FROM modelSeries
WHERE name = '259 (R 1100 S, R 1100 RS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1100 S 98 (0422,0432) (USA, 01/1997-09/2004)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 C 03 (0329,0379) (ECE, 10/2002-07/2004)', 'BMW', id, '0329,0379' FROM modelSeries
WHERE name = '259C (R 850 C, R 1200 C)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 C 03 (0329,0379) (ECE, 10/2002-07/2004)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 C 03 (0329,0379) (USA, 10/2002-07/2004)', 'BMW', id, '0329,0379' FROM modelSeries
WHERE name = '259C (R 850 C, R 1200 C)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 C 03 (0329,0379) (USA, 10/2002-07/2004)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 C 97 (0424,0434) (ECE, 04/1996-05/2003)', 'BMW', id, '0424,0434' FROM modelSeries
WHERE name = '259C (R 850 C, R 1200 C)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 C 97 (0424,0434) (ECE, 04/1996-05/2003)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 C 97 (0424,0434) (USA, 06/1996-05/2003)', 'BMW', id, '0424,0434' FROM modelSeries
WHERE name = '259C (R 850 C, R 1200 C)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 C 97 (0424,0434) (USA, 06/1996-05/2003)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 850 C 99 (0421) (ECE, 01/1997-09/2000)', 'BMW', id, '0421' FROM modelSeries
WHERE name = '259C (R 850 C, R 1200 C)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 850 C 99 (0421) (ECE, 01/1997-09/2000)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 Montauk 03 (0309,0319) (ECE, 10/2002-10/2004)', 'BMW', id, '0309,0319' FROM modelSeries
WHERE name = '259C (R 1200 C Montauk)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 Montauk 03 (0309,0319) (ECE, 10/2002-10/2004)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 Montauk 03 (0309,0319) (USA, 10/2002-10/2004)', 'BMW', id, '0309,0319' FROM modelSeries
WHERE name = '259C (R 1200 C Montauk)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 Montauk 03 (0309,0319) (USA, 10/2002-10/2004)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 C Indep. 00 (0405,0433) (ECE, 05/2000-05/2003)', 'BMW', id, '0405,0433' FROM modelSeries
WHERE name = '259C (R 1200 C Independent)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 C Indep. 00 (0405,0433) (ECE, 05/2000-05/2003)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 C Indep. 00 (0405,0433) (USA, 09/2000-05/2003)', 'BMW', id, '0405,0433' FROM modelSeries
WHERE name = '259C (R 1200 C Independent)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 C Indep. 00 (0405,0433) (USA, 09/2000-05/2003)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 C Indep. 03 (0362,0391) (ECE, 02/2003-08/2004)', 'BMW', id, '0362,0391' FROM modelSeries
WHERE name = '259C (R 1200 C Independent)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 C Indep. 03 (0362,0391) (ECE, 02/2003-08/2004)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 C Indep. 03 (0362,0391) (USA, 10/2003-05/2004)', 'BMW', id, '0362,0391' FROM modelSeries
WHERE name = '259C (R 1200 C Independent)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 C Indep. 03 (0362,0391) (USA, 10/2003-05/2004)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 CL (0442,0496) (ECE, 10/2001-07/2004)', 'BMW', id, '0442,0496' FROM modelSeries
WHERE name = 'K30 (R1200 CL)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 CL (0442,0496) (ECE, 10/2001-07/2004)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 CL (0442,0496) (USA, 10/2001-07/2004)', 'BMW', id, '0442,0496' FROM modelSeries
WHERE name = 'K30 (R1200 CL)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 CL (0442,0496) (USA, 10/2001-07/2004)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 GS 04 (0307,0317) (ECE, 12/2002-10/2007)', 'BMW', id, '0307,0317' FROM modelSeries
WHERE name = 'K25 (R 1200 GS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 GS 04 (0307,0317) (ECE, 12/2002-10/2007)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 GS 04 (0307,0317) (USA, 03/2003-09/2007)', 'BMW', id, '0307,0317' FROM modelSeries
WHERE name = 'K25 (R 1200 GS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 GS 04 (0307,0317) (USA, 03/2003-09/2007)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 GS 08 (0303,0313) (ECE, 11/2006-09/2009)', 'BMW', id, '0303,0313' FROM modelSeries
WHERE name = 'K25 (R 1200 GS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 GS 08 (0303,0313) (ECE, 11/2006-09/2009)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 GS 08 (0303,0313) (USA, 11/2006-09/2009)', 'BMW', id, '0303,0313' FROM modelSeries
WHERE name = 'K25 (R 1200 GS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 GS 08 (0303,0313) (USA, 11/2006-09/2009)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 GS 10 (0450,0460) (ECE, 10/2008-12/2012)', 'BMW', id, '0450,0460' FROM modelSeries
WHERE name = 'K25 (R 1200 GS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 GS 10 (0450,0460) (ECE, 10/2008-12/2012)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 GS 10 (0450,0460) (USA, 10/2008-09/2012)', 'BMW', id, '0450,0460' FROM modelSeries
WHERE name = 'K25 (R 1200 GS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 GS 10 (0450,0460) (USA, 10/2008-09/2012)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 GS Adve. 06 (0382,0397) (ECE, 03/2005-09/2007)', 'BMW', id, '0382,0397' FROM modelSeries
WHERE name = 'K25 (R 1200 GS Adventure)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 GS Adve. 06 (0382,0397) (ECE, 03/2005-09/2007)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 GS Adve. 06 (0382,0397) (USA, 03/2005-09/2007)', 'BMW', id, '0382,0397' FROM modelSeries
WHERE name = 'K25 (R 1200 GS Adventure)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 GS Adve. 06 (0382,0397) (USA, 03/2005-09/2007)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 GS Adve. 08 (0380,0390) (ECE, 11/2006-09/2009)', 'BMW', id, '0380,0390' FROM modelSeries
WHERE name = 'K25 (R 1200 GS Adventure)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 GS Adve. 08 (0380,0390) (ECE, 11/2006-09/2009)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 GS Adve. 08 (0380,0390) (USA, 11/2006-09/2009)', 'BMW', id, '0380,0390' FROM modelSeries
WHERE name = 'K25 (R 1200 GS Adventure)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 GS Adve. 08 (0380,0390) (USA, 11/2006-09/2009)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 GS Adve. 10 (0470,0480) (ECE, 10/2008-07/2013)', 'BMW', id, '0470,0480' FROM modelSeries
WHERE name = 'K25 (R 1200 GS Adventure)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 GS Adve. 10 (0470,0480) (ECE, 10/2008-07/2013)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 GS Adve. 10 (0470,0480) (USA, 10/2008-07/2013)', 'BMW', id, '0470,0480' FROM modelSeries
WHERE name = 'K25 (R 1200 GS Adventure)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 GS Adve. 10 (0470,0480) (USA, 10/2008-07/2013)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'HP2 Enduro (0369,0389) (ECE, 08/2004-08/2006)', 'BMW', id, '0369,0389' FROM modelSeries
WHERE name = 'K25 (HP)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'HP2 Enduro (0369,0389) (ECE, 08/2004-08/2006)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'HP2 Enduro (0369,0389) (USA, 08/2004-08/2006)', 'BMW', id, '0369,0389' FROM modelSeries
WHERE name = 'K25 (HP)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'HP2 Enduro (0369,0389) (USA, 08/2004-08/2006)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'HP2 Megamoto (0310,0320) (ECE, 08/2006-11/2008)', 'BMW', id, '0310,0320' FROM modelSeries
WHERE name = 'K25 (HP)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'HP2 Megamoto (0310,0320) (ECE, 08/2006-11/2008)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'HP2 Megamoto (0310,0320) (USA, 08/2006-11/2008)', 'BMW', id, '0310,0320' FROM modelSeries
WHERE name = 'K25 (HP)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'HP2 Megamoto (0310,0320) (USA, 08/2006-11/2008)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 RT 05 (0368,0388) (ECE, 11/2003-11/2009)', 'BMW', id, '0368,0388' FROM modelSeries
WHERE name = 'K26 (R 900 RT, R 1200 RT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 RT 05 (0368,0388) (ECE, 11/2003-11/2009)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 RT 05 (0368,0388) (USA, 11/2003-09/2009)', 'BMW', id, '0368,0388' FROM modelSeries
WHERE name = 'K26 (R 900 RT, R 1200 RT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 RT 05 (0368,0388) (USA, 11/2003-09/2009)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 RT 10 (0430,0440) (ECE, 07/2008-06/2014)', 'BMW', id, '0430,0440' FROM modelSeries
WHERE name = 'K26 (R 900 RT, R 1200 RT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 RT 10 (0430,0440) (ECE, 07/2008-06/2014)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 RT 10 (0430,0440) (USA, 07/2008-05/2014)', 'BMW', id, '0430,0440' FROM modelSeries
WHERE name = 'K26 (R 900 RT, R 1200 RT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 RT 10 (0430,0440) (USA, 07/2008-05/2014)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 900 RT 05 SF (0367,0387) (ECE, 04/2005-09/2009)', 'BMW', id, '0367,0387' FROM modelSeries
WHERE name = 'K26 (R 900 RT, R 1200 RT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 900 RT 05 SF (0367,0387) (ECE, 04/2005-09/2009)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 900 RT 05 SF (0367,0387) (USA, 04/2005-07/2009)', 'BMW', id, '0367,0387' FROM modelSeries
WHERE name = 'K26 (R 900 RT, R 1200 RT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 900 RT 05 SF (0367,0387) (USA, 04/2005-07/2009)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 900 RT 10 SF (0330,0340) (ECE, 07/2009-09/2013)', 'BMW', id, '0330,0340' FROM modelSeries
WHERE name = 'K26 (R 900 RT, R 1200 RT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 900 RT 10 SF (0330,0340) (ECE, 07/2009-09/2013)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 900 RT 10 SF (0330,0340) (USA, 03/2009-05/2010)', 'BMW', id, '0330,0340' FROM modelSeries
WHERE name = 'K26 (R 900 RT, R 1200 RT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 900 RT 10 SF (0330,0340) (USA, 03/2009-05/2010)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 R 06 (0378,0398) (ECE, 11/2005-10/2010)', 'BMW', id, '0378,0398' FROM modelSeries
WHERE name = 'K27 (R 1200 R)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 R 06 (0378,0398) (ECE, 11/2005-10/2010)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 R 06 (0378,0398) (USA, 11/2005-09/2010)', 'BMW', id, '0378,0398' FROM modelSeries
WHERE name = 'K27 (R 1200 R)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 R 06 (0378,0398) (USA, 11/2005-09/2010)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 R 11 (0400,0490) (ECE, 12/2009-07/2014)', 'BMW', id, '0400,0490' FROM modelSeries
WHERE name = 'K27 (R 1200 R)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 R 11 (0400,0490) (ECE, 12/2009-07/2014)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 R 11 (0400,0490) (USA, 12/2009-07/2014)', 'BMW', id, '0400,0490' FROM modelSeries
WHERE name = 'K27 (R 1200 R)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 R 11 (0400,0490) (USA, 12/2009-07/2014)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 ST (0328,0338) (ECE, 05/2003-11/2007)', 'BMW', id, '0328,0338' FROM modelSeries
WHERE name = 'K28 (R 1200 ST)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 ST (0328,0338) (ECE, 05/2003-11/2007)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 ST (0328,0338) (USA, 06/2003-06/2007)', 'BMW', id, '0328,0338' FROM modelSeries
WHERE name = 'K28 (R 1200 ST)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 ST (0328,0338) (USA, 06/2003-06/2007)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'HP2 Sport (0458, 0468) (ECE, 01/2007-07/2010)', 'BMW', id, '0458,0468' FROM modelSeries
WHERE name = 'K29 (R 1200 S, HP2 Sport)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'HP2 Sport (0458, 0468) (ECE, 01/2007-07/2010)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'HP2 Sport (0458, 0468) (USA, 01/2007-04/2010)', 'BMW', id, '0458,0468' FROM modelSeries
WHERE name = 'K29 (R 1200 S, HP2 Sport)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'HP2 Sport (0458, 0468) (USA, 01/2007-04/2010)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 S (0366,0396) (ECE, 11/2004-12/2006)', 'BMW', id, '0366,0396' FROM modelSeries
WHERE name = 'K29 (R 1200 S, HP2 Sport)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 S (0366,0396) (ECE, 11/2004-12/2006)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 S (0366,0396) (USA, 11/2004-09/2006)', 'BMW', id, '0366,0396' FROM modelSeries
WHERE name = 'K29 (R 1200 S, HP2 Sport)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 S (0366,0396) (USA, 11/2004-09/2006)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 GS (0A01, 0A11) (ECE, 10/2011-12/2016)', 'BMW', id, '0A01,0A11' FROM modelSeries
WHERE name = 'K50 (R 1200 GS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 GS (0A01, 0A11) (ECE, 10/2011-12/2016)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 GS (0A01, 0A11) (USA, 10/2011-11/2016)', 'BMW', id, '0A01,0A11' FROM modelSeries
WHERE name = 'K50 (R 1200 GS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 GS (0A01, 0A11) (USA, 10/2011-11/2016)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 GS (0A21) (BRA, 06/2014-04/2016)', 'BMW', id, '0A21' FROM modelSeries
WHERE name = 'K50 (R 1200 GS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 GS (0A21) (BRA, 06/2014-04/2016)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 GS (0A31) (THA, 12/2014-07/2016)', 'BMW', id, '0A31' FROM modelSeries
WHERE name = 'K50 (R 1200 GS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 GS (0A31) (THA, 12/2014-07/2016)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 GS (0A41) (BRA, 06/2016-07/2016)', 'BMW', id, '0A41' FROM modelSeries
WHERE name = 'K50 (R 1200 GS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 GS (0A41) (BRA, 06/2016-07/2016)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 GS 17 (0A51, 0A61) (ECE, 11/2015-06/2018)', 'BMW', id, '0A51,0A61' FROM modelSeries
WHERE name = 'K50 (R 1200 GS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 GS 17 (0A51, 0A61) (ECE, 11/2015-06/2018)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 GS 17 (0A51, 0A61) (USA, 09/2015-06/2018)', 'BMW', id, '0A51,0A61' FROM modelSeries
WHERE name = 'K50 (R 1200 GS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 GS 17 (0A51, 0A61) (USA, 09/2015-06/2018)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 GS 17 (0A71) (BRA, 01/2017-07/2018)', 'BMW', id, '0A71' FROM modelSeries
WHERE name = 'K50 (R 1200 GS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 GS 17 (0A71) (BRA, 01/2017-07/2018)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 GS 17 (0A81) (THA, 04/2017-07/2018)', 'BMW', id, '0A81' FROM modelSeries
WHERE name = 'K50 (R 1200 GS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 GS 17 (0A81) (THA, 04/2017-07/2018)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 GS Adve. (0A02, 0A12) (ECE, 09/2012-06/2018)', 'BMW', id, '0A02,0A12' FROM modelSeries
WHERE name = 'K51 (R 1200 GS Adventure)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 GS Adve. (0A02, 0A12) (ECE, 09/2012-06/2018)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 GS Adve. (0A02, 0A12) (USA, 09/2012-06/2018)', 'BMW', id, '0A02,0A12' FROM modelSeries
WHERE name = 'K51 (R 1200 GS Adventure)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 GS Adve. (0A02, 0A12) (USA, 09/2012-06/2018)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 GS Adve. (0A22) (BRA, 06/2014-04/2016)', 'BMW', id, '0A22' FROM modelSeries
WHERE name = 'K51 (R 1200 GS Adventure)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 GS Adve. (0A22) (BRA, 06/2014-04/2016)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 GS Adve. (0A32) (THA, 02/2015-08/2018)', 'BMW', id, '0A32' FROM modelSeries
WHERE name = 'K51 (R 1200 GS Adventure)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 GS Adve. (0A32) (THA, 02/2015-08/2018)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 GS Adve. (0A42) (BRA, 06/2016-08/2018)', 'BMW', id, '0A42' FROM modelSeries
WHERE name = 'K51 (R 1200 GS Adventure)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 GS Adve. (0A42) (BRA, 06/2016-08/2018)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 RT (0A03, 0A13) (ECE, 01/2013-06/2018)', 'BMW', id, '0A03,0A13' FROM modelSeries
WHERE name = 'K52 (R 1200 RT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 RT (0A03, 0A13) (ECE, 01/2013-06/2018)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 RT (0A03, 0A13) (USA, 01/2013-06/2018)', 'BMW', id, '0A03,0A13' FROM modelSeries
WHERE name = 'K52 (R 1200 RT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 RT (0A03, 0A13) (USA, 01/2013-06/2018)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 R (0A04, 0A14) (ECE, 11/2013-06/2018)', 'BMW', id, '0A04,0A14' FROM modelSeries
WHERE name = 'K53 (R 1200 R)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 R (0A04, 0A14) (ECE, 11/2013-06/2018)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 R (0A04, 0A14) (USA, 11/2013-06/2018)', 'BMW', id, '0A04,0A14' FROM modelSeries
WHERE name = 'K53 (R 1200 R)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 R (0A04, 0A14) (USA, 11/2013-06/2018)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 RS (0A05, 0A15) (ECE, 06/2014-06/2018)', 'BMW', id, '0A05,0A15' FROM modelSeries
WHERE name = 'K54 (R 1200 RS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 RS (0A05, 0A15) (ECE, 06/2014-06/2018)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R 1200 RS (0A05, 0A15) (USA, 06/2014-06/2018)', 'BMW', id, '0A05,0A15' FROM modelSeries
WHERE name = 'K54 (R 1200 RS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 1200 RS (0A05, 0A15) (USA, 06/2014-06/2018)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R nineT (0A06, 0A16) (ECE, 01/2013-08/2016)', 'BMW', id, '0A06,0A16' FROM modelSeries
WHERE name = 'K21 (R nineT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R nineT (0A06, 0A16) (ECE, 01/2013-08/2016)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R nineT (0A06, 0A16) (USA, 01/2013-08/2016)', 'BMW', id, '0A06,0A16' FROM modelSeries
WHERE name = 'K21 (R nineT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R nineT (0A06, 0A16) (USA, 01/2013-08/2016)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R nineT 16 (0J01, 0J03) (ECE, 10/2015-06/2018)', 'BMW', id, '0J01,0J03' FROM modelSeries
WHERE name = 'K21 (R nineT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R nineT 16 (0J01, 0J03) (ECE, 10/2015-06/2018)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R nineT 16 (0J01, 0J03) (USA, 10/2015-06/2018)', 'BMW', id, '0J01,0J03' FROM modelSeries
WHERE name = 'K21 (R nineT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R nineT 16 (0J01, 0J03) (USA, 10/2015-06/2018)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R nineT Pure (0J11, 0J13) (ECE, 12/2015-06/2018)', 'BMW', id, '0J11,0J13' FROM modelSeries
WHERE name = 'K22 (R nineT Pure)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R nineT Pure (0J11, 0J13) (ECE, 12/2015-06/2018)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R nineT Pure (0J11, 0J13) (USA, 12/2015-04/2018)', 'BMW', id, '0J11,0J13' FROM modelSeries
WHERE name = 'K22 (R nineT Pure)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R nineT Pure (0J11, 0J13) (USA, 12/2015-04/2018)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R nineT Scrambler (0J31, 0J33) (ECE, 09/2015-06/2018)', 'BMW', id, '0J31,0J33' FROM modelSeries
WHERE name = 'K23 (R nineT Scrambler)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R nineT Scrambler (0J31, 0J33) (ECE, 09/2015-06/2018)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R nineT Scrambler (0J31, 0J33) (USA, 09/2015-06/2018)', 'BMW', id, '0J31,0J33' FROM modelSeries
WHERE name = 'K23 (R nineT Scrambler)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R nineT Scrambler (0J31, 0J33) (USA, 09/2015-06/2018)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R nineT Racer (0J21, 0J23) (ECE, 12/2015-06/2018)', 'BMW', id, '0J21,0J23' FROM modelSeries
WHERE name = 'K32 (R nineT Racer)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R nineT Racer (0J21, 0J23) (ECE, 12/2015-06/2018)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R nineT Racer (0J21, 0J23) (USA, 01/2016-02/2018)', 'BMW', id, '0J21,0J23' FROM modelSeries
WHERE name = 'K32 (R nineT Racer)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R nineT Racer (0J21, 0J23) (USA, 01/2016-02/2018)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R nineT Urban G/S (0J41, 0J43) (ECE, 03/2016-06/2018)', 'BMW', id, '0J41,0J43' FROM modelSeries
WHERE name = 'K33 (R nineT Urban G/S)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R nineT Urban G/S (0J41, 0J43) (ECE, 03/2016-06/2018)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'R nineT Urban G/S (0J41, 0J43) (USA, 03/2016-04/2018)', 'BMW', id, '0J41,0J43' FROM modelSeries
WHERE name = 'K33 (R nineT Urban G/S)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R nineT Urban G/S (0J41, 0J43) (USA, 03/2016-04/2018)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 75 84 (0561) (ECE, 07/1985-12/1988)', 'BMW', id, '0561' FROM modelSeries
WHERE name = 'K569 (K 75, K 75 c, K 75 s, K 75 RT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 75 84 (0561) (ECE, 07/1985-12/1988)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 75 85 (0562,0571) (ECE, 12/1984-11/1996)', 'BMW', id, '0562,0571' FROM modelSeries
WHERE name = 'K569 (K 75, K 75 c, K 75 s, K 75 RT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 75 85 (0562,0571) (ECE, 12/1984-11/1996)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 75 85 (0562,0571) (USA, 07/1990-09/1995)', 'BMW', id, '0562,0571' FROM modelSeries
WHERE name = 'K569 (K 75, K 75 c, K 75 s, K 75 RT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 75 85 (0562,0571) (USA, 07/1990-09/1995)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 75 C (0564,0574) (ECE, 06/1985-06/1988)', 'BMW', id, '0564,0574' FROM modelSeries
WHERE name = 'K569 (K 75, K 75 c, K 75 s, K 75 RT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 75 C (0564,0574) (ECE, 06/1985-06/1988)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 75 C (0564,0574) (USA, 07/1985-03/1990)', 'BMW', id, '0564,0574' FROM modelSeries
WHERE name = 'K569 (K 75, K 75 c, K 75 s, K 75 RT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 75 C (0564,0574) (USA, 07/1985-03/1990)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 75 RT (0565,0573) (ECE, 01/1989-06/2005)', 'BMW', id, '0565,0573' FROM modelSeries
WHERE name = 'K569 (K 75, K 75 c, K 75 s, K 75 RT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 75 RT (0565,0573) (ECE, 01/1989-06/2005)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 75 RT (0565,0573) (USA, 09/1989-01/1995)', 'BMW', id, '0565,0573' FROM modelSeries
WHERE name = 'K569 (K 75, K 75 c, K 75 s, K 75 RT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 75 RT (0565,0573) (USA, 09/1989-01/1995)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 75 S (0563,0572) (ECE, 10/1985-05/1995)', 'BMW', id, '0563,0572' FROM modelSeries
WHERE name = 'K569 (K 75, K 75 c, K 75 s, K 75 RT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 75 S (0563,0572) (ECE, 10/1985-05/1995)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 75 S (0563,0572) (USA, 05/1986-09/1994)', 'BMW', id, '0563,0572' FROM modelSeries
WHERE name = 'K569 (K 75, K 75 c, K 75 s, K 75 RT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 75 S (0563,0572) (USA, 05/1986-09/1994)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 100 83 (0501,0511) (ECE, 05/1982-12/1988)', 'BMW', id, '0501,0511' FROM modelSeries
WHERE name = 'K589 (K100, RS, RT, LT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 100 83 (0501,0511) (ECE, 05/1982-12/1988)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 100 83 (0501,0511) (USA, 03/1984-10/1986)', 'BMW', id, '0501,0511' FROM modelSeries
WHERE name = 'K589 (K100, RS, RT, LT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 100 83 (0501,0511) (USA, 03/1984-10/1986)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 100 88 (0521) (ECE, 10/1987-07/1990)', 'BMW', id, '0521' FROM modelSeries
WHERE name = 'K589 (K100, RS, RT, LT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 100 88 (0521) (ECE, 10/1987-07/1990)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 100 LT 87 (0506,0516) (ECE, 07/1986-10/1991)', 'BMW', id, '0506,0516' FROM modelSeries
WHERE name = 'K589 (K100, RS, RT, LT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 100 LT 87 (0506,0516) (ECE, 07/1986-10/1991)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 100 LT 87 (0506,0516) (USA, 07/1986-03/1991)', 'BMW', id, '0506,0516' FROM modelSeries
WHERE name = 'K589 (K100, RS, RT, LT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 100 LT 87 (0506,0516) (USA, 07/1986-03/1991)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 100 RS 83 (0502,0503,0513) (ECE, 04/1984-10/1984)', 'BMW', id, '0502,0503,0513' FROM modelSeries
WHERE name = 'K589 (K100, RS, RT, LT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 100 RS 83 (0502,0503,0513) (ECE, 04/1984-10/1984)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 100 RS 83 (0502,0503,0513) (ECE, 05/1983-10/1989)', 'BMW', id, '0502,0503,0513' FROM modelSeries
WHERE name = 'K589 (K100, RS, RT, LT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 100 RS 83 (0502,0503,0513) (ECE, 05/1983-10/1989)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 100 RS 83 (0502,0503,0513) (USA, 03/1984-07/1989)', 'BMW', id, '0502,0503,0513' FROM modelSeries
WHERE name = 'K589 (K100, RS, RT, LT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 100 RS 83 (0502,0503,0513) (USA, 03/1984-07/1989)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 100 RT 84 (0504,0505,0514) (ECE, 04/1984-10/1984)', 'BMW', id, '0504,0505,0514' FROM modelSeries
WHERE name = 'K589 (K100, RS, RT, LT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 100 RT 84 (0504,0505,0514) (ECE, 04/1984-10/1984)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 100 RT 84 (0504,0505,0514) (ECE, 10/1983-07/1989)', 'BMW', id, '0504,0505,0514' FROM modelSeries
WHERE name = 'K589 (K100, RS, RT, LT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 100 RT 84 (0504,0505,0514) (ECE, 10/1983-07/1989)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 100 RT 84 (0504,0505,0514) (USA, 04/1984-05/1988)', 'BMW', id, '0504,0505,0514' FROM modelSeries
WHERE name = 'K589 (K100, RS, RT, LT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 100 RT 84 (0504,0505,0514) (USA, 04/1984-05/1988)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 1 (0525,0535) (ECE, 08/1988-09/1993)', 'BMW', id, '0525,0535' FROM modelSeries
WHERE name = 'K589 (K1, K 100 RS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 1 (0525,0535) (ECE, 08/1988-09/1993)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 1 (0525,0535) (USA, 01/1989-11/1992)', 'BMW', id, '0525,0535' FROM modelSeries
WHERE name = 'K589 (K1, K 100 RS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 1 (0525,0535) (USA, 01/1989-11/1992)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 100 RS (0523,0533) (ECE, 03/1989-06/1992)', 'BMW', id, '0523,0533' FROM modelSeries
WHERE name = 'K589 (K1, K 100 RS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 100 RS (0523,0533) (ECE, 03/1989-06/1992)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 100 RS (0523,0533) (USA, 07/1990-01/1992)', 'BMW', id, '0523,0533' FROM modelSeries
WHERE name = 'K589 (K1, K 100 RS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 100 RS (0523,0533) (USA, 07/1990-01/1992)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 1100 LT (0526, 0536) (ECE, 06/1989-04/1999)', 'BMW', id, '0526,0536' FROM modelSeries
WHERE name = 'K589 (K 1100 RS, K 1100 LT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 1100 LT (0526, 0536) (ECE, 06/1989-04/1999)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 1100 LT (0526, 0536) (USA, 02/1992-03/1997)', 'BMW', id, '0526,0536' FROM modelSeries
WHERE name = 'K589 (K 1100 RS, K 1100 LT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 1100 LT (0526, 0536) (USA, 02/1992-03/1997)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 1100 RS (0522,0532) (ECE, 04/1992-12/1996)', 'BMW', id, '0522,0532' FROM modelSeries
WHERE name = 'K589 (K 1100 RS, K 1100 LT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 1100 RS (0522,0532) (ECE, 04/1992-12/1996)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 1100 RS (0522,0532) (USA, 11/1992-06/1996)', 'BMW', id, '0522,0532' FROM modelSeries
WHERE name = 'K589 (K 1100 RS, K 1100 LT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 1100 RS (0522,0532) (USA, 11/1992-06/1996)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 1200 LT 04 (0549,0559) (ECE, 02/2003-07/2008)', 'BMW', id, '0549,0559' FROM modelSeries
WHERE name = 'K589 (K 1200 RS, K 1200 LT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 1200 LT 04 (0549,0559) (ECE, 02/2003-07/2008)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 1200 LT 04 (0549,0559) (USA, 02/2003-07/2008)', 'BMW', id, '0549,0559' FROM modelSeries
WHERE name = 'K589 (K 1200 RS, K 1200 LT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 1200 LT 04 (0549,0559) (USA, 02/2003-07/2008)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 1200 LT 99 (0545,0555) (ECE, 06/1997-12/2003)', 'BMW', id, '0545,0555' FROM modelSeries
WHERE name = 'K589 (K 1200 RS, K 1200 LT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 1200 LT 99 (0545,0555) (ECE, 06/1997-12/2003)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 1200 LT 99 (0545,0555) (USA, 06/1997-11/2003)', 'BMW', id, '0545,0555' FROM modelSeries
WHERE name = 'K589 (K 1200 RS, K 1200 LT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 1200 LT 99 (0545,0555) (USA, 06/1997-11/2003)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 1200 RS 97 (0544,0554) (ECE, 04/1996-04/2005)', 'BMW', id, '0544,0554' FROM modelSeries
WHERE name = 'K589 (K 1200 RS, K 1200 LT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 1200 RS 97 (0544,0554) (ECE, 04/1996-04/2005)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 1200 RS 97 (0544,0554) (USA, 05/1996-12/2000)', 'BMW', id, '0544,0554' FROM modelSeries
WHERE name = 'K589 (K 1200 RS, K 1200 LT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 1200 RS 97 (0544,0554) (USA, 05/1996-12/2000)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 1200 GT 01 (0548,0558) (ECE, 03/2002-07/2005)', 'BMW', id, '0548,0558' FROM modelSeries
WHERE name = 'K41 (K 1200 GT, K 1200 RS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 1200 GT 01 (0548,0558) (ECE, 03/2002-07/2005)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 1200 GT 01 (0548,0558) (USA, 04/2002-10/2004)', 'BMW', id, '0548,0558' FROM modelSeries
WHERE name = 'K41 (K 1200 GT, K 1200 RS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 1200 GT 01 (0548,0558) (USA, 04/2002-10/2004)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 1200 RS 01 (0547,0557) (ECE, 03/2000-07/2005)', 'BMW', id, '0547,0557' FROM modelSeries
WHERE name = 'K41 (K 1200 GT, K 1200 RS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 1200 RS 01 (0547,0557) (ECE, 03/2000-07/2005)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 1200 RS 01 (0547,0557) (USA, 03/2000-07/2004)', 'BMW', id, '0547,0557' FROM modelSeries
WHERE name = 'K41 (K 1200 GT, K 1200 RS)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 1200 RS 01 (0547,0557) (USA, 03/2000-07/2004)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 1200 S (0581,0591) (ECE, 04/2003-08/2008)', 'BMW', id, '0581,0591' FROM modelSeries
WHERE name = 'K40 (K 1200 S, K 1300 S)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 1200 S (0581,0591) (ECE, 04/2003-08/2008)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 1200 S (0581,0591) (USA, 04/2003-08/2008)', 'BMW', id, '0581,0591' FROM modelSeries
WHERE name = 'K40 (K 1200 S, K 1300 S)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 1200 S (0581,0591) (USA, 04/2003-08/2008)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 1300 S (0508,0509) (ECE, 12/2007-09/2015)', 'BMW', id, '0508,0509' FROM modelSeries
WHERE name = 'K40 (K 1200 S, K 1300 S)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 1300 S (0508,0509) (ECE, 12/2007-09/2015)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 1300 S (0508,0509) (USA, 11/2007-09/2015)', 'BMW', id, '0508,0509' FROM modelSeries
WHERE name = 'K40 (K 1200 S, K 1300 S)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 1300 S (0508,0509) (USA, 11/2007-09/2015)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 1200 R (0584,0594) (ECE, 03/2004-08/2008)', 'BMW', id, '0584,0594' FROM modelSeries
WHERE name = 'K43 (K 1200 R, Sport, K 1300 R)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 1200 R (0584,0594) (ECE, 03/2004-08/2008)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 1200 R (0584,0594) (USA, 04/2004-07/2008)', 'BMW', id, '0584,0594' FROM modelSeries
WHERE name = 'K43 (K 1200 R, Sport, K 1300 R)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 1200 R (0584,0594) (USA, 04/2004-07/2008)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 1200 R Sport (0585,0595) (ECE, 06/2005-07/2007)', 'BMW', id, '0585,0595' FROM modelSeries
WHERE name = 'K43 (K 1200 R, Sport, K 1300 R)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 1200 R Sport (0585,0595) (ECE, 06/2005-07/2007)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 1200 R Sport (0585,0595) (USA, 06/2005-07/2007)', 'BMW', id, '0585,0595' FROM modelSeries
WHERE name = 'K43 (K 1200 R, Sport, K 1300 R)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 1200 R Sport (0585,0595) (USA, 06/2005-07/2007)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 1300 R (0518,0519) (ECE, 11/2007-07/2015)', 'BMW', id, '0518,0519' FROM modelSeries
WHERE name = 'K43 (K 1200 R, Sport, K 1300 R)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 1300 R (0518,0519) (ECE, 11/2007-07/2015)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 1300 R (0518,0519) (USA, 12/2007-01/2012)', 'BMW', id, '0518,0519' FROM modelSeries
WHERE name = 'K43 (K 1200 R, Sport, K 1300 R)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 1300 R (0518,0519) (USA, 12/2007-01/2012)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 1200 GT (0587,0597) (ECE, 11/2004-09/2008)', 'BMW', id, '0587,0597' FROM modelSeries
WHERE name = 'K44 (K 1200 GT, K 1300 GT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 1200 GT (0587,0597) (ECE, 11/2004-09/2008)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 1200 GT (0587,0597) (USA, 11/2004-09/2008)', 'BMW', id, '0587,0597' FROM modelSeries
WHERE name = 'K44 (K 1200 GT, K 1300 GT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 1200 GT (0587,0597) (USA, 11/2004-09/2008)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 1300 GT (0538,0539) (ECE, 12/2007-10/2010)', 'BMW', id, '0538,0539' FROM modelSeries
WHERE name = 'K44 (K 1200 GT, K 1300 GT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 1300 GT (0538,0539) (ECE, 12/2007-10/2010)' AND userId IS NULL);
INSERT INTO modelSeries (name, manufacturer, parentId, typeCodes)
SELECT 'K 1300 GT (0538,0539) (USA, 12/2007-08/2010)', 'BMW', id, '0538,0539' FROM modelSeries
WHERE name = 'K44 (K 1200 GT, K 1300 GT)' AND userId IS NULL
  AND NOT EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K 1300 GT (0538,0539) (USA, 12/2007-08/2010)' AND userId IS NULL);

-- 5) Re-parent surviving old entries into the new structure ---------------
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = '1-Zyl.' AND userId IS NULL)
WHERE userId IS NULL AND name = 'R 25/3, R 26, R 27 (Einzylinder)'
  AND EXISTS (SELECT 1 FROM modelSeries WHERE name = '1-Zyl.' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'Boxer' AND userId IS NULL)
WHERE userId IS NULL AND name = 'R 50, R 60, R 69 S (Boxer /2)'
  AND EXISTS (SELECT 1 FROM modelSeries WHERE name = 'Boxer' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Boxer' AND userId IS NULL)
WHERE userId IS NULL AND name = 'R 50/5, R 60/5, R 75/5 (69-73)'
  AND EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R-Boxer' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R-Boxer' AND userId IS NULL)
WHERE userId IS NULL AND name = 'R 60/6, R 75/6, R 90/6, R 90 S (73-76)'
  AND EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R-Boxer' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R45-R65LS..78-85' AND userId IS NULL)
WHERE userId IS NULL AND name = 'R 45'
  AND EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R45-R65LS..78-85' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R45-R65LS..78-85' AND userId IS NULL)
WHERE userId IS NULL AND name = 'R 65'
  AND EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R45-R65LS..78-85' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R 60, R 75 , R 80, /7, RT (76-85)' AND userId IS NULL)
WHERE userId IS NULL AND name = 'R 80'
  AND EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 60, R 75 , R 80, /7, RT (76-85)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL)
WHERE userId IS NULL AND name = 'R 100'
  AND EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R 80 GS, R 100 GS, PD (90-95)' AND userId IS NULL)
WHERE userId IS NULL AND name = 'R 80 GS'
  AND EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80 GS, R 100 GS, PD (90-95)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R 80 GS, R 100 GS, PD (90-95)' AND userId IS NULL)
WHERE userId IS NULL AND name = 'R 100 GS'
  AND EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 80 GS, R 100 GS, PD (90-95)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = '259 (R 850 GS, R 1100 GS)' AND userId IS NULL)
WHERE userId IS NULL AND name = 'R 1100 GS'
  AND EXISTS (SELECT 1 FROM modelSeries WHERE name = '259 (R 850 GS, R 1100 GS)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R21 (R 1150 GS)' AND userId IS NULL)
WHERE userId IS NULL AND name = 'R 1150 GS'
  AND EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R21 (R 1150 GS)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'K25 (R 1200 GS)' AND userId IS NULL)
WHERE userId IS NULL AND name = 'R 1200 GS'
  AND EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K25 (R 1200 GS)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'K27 (R 1200 R)' AND userId IS NULL)
WHERE userId IS NULL AND name = 'R 1200 R'
  AND EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K27 (R 1200 R)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'K26 (R 900 RT, R 1200 RT)' AND userId IS NULL)
WHERE userId IS NULL AND name = 'R 1200 RT'
  AND EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K26 (R 900 RT, R 1200 RT)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'K21 (R nineT)' AND userId IS NULL)
WHERE userId IS NULL AND name = 'R nineT'
  AND EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K21 (R nineT)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'K569 (K 75, K 75 c, K 75 s, K 75 RT)' AND userId IS NULL)
WHERE userId IS NULL AND name = 'K 75'
  AND EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K569 (K 75, K 75 c, K 75 s, K 75 RT)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'K589 (K100, RS, RT, LT)' AND userId IS NULL)
WHERE userId IS NULL AND name = 'K 100'
  AND EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K589 (K100, RS, RT, LT)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'K589 (K 1100 RS, K 1100 LT)' AND userId IS NULL)
WHERE userId IS NULL AND name = 'K 1100'
  AND EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K589 (K 1100 RS, K 1100 LT)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'K589 (K 1200 RS, K 1200 LT)' AND userId IS NULL)
WHERE userId IS NULL AND name = 'K 1200'
  AND EXISTS (SELECT 1 FROM modelSeries WHERE name = 'K589 (K 1200 RS, K 1200 LT)' AND userId IS NULL);
UPDATE modelSeries SET parentId = (SELECT id FROM modelSeries WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL)
WHERE userId IS NULL AND name = 'R 100 /7T (ECE, 06/1980-10/1984)'
  AND EXISTS (SELECT 1 FROM modelSeries WHERE name = 'R 100, /7, /T, CS, RS, RT, S (76-84)' AND userId IS NULL);

-- 6) Guarded cleanup of obsolete entries ----------------------------------
DELETE FROM modelSeries WHERE userId IS NULL AND name = 'R 100 GS (ECE, 04/1990-07/1996)'
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat) AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL) AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);
DELETE FROM modelSeries WHERE userId IS NULL AND name = 'R 100 GS PD (ECE, 08/1990-07/1996)'
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat) AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL) AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);
DELETE FROM modelSeries WHERE userId IS NULL AND name = 'R 65 (35KW) (ECE, 06/1985-12/1992)'
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat) AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL) AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);
DELETE FROM modelSeries WHERE userId IS NULL AND name = 'K 100 RS 83 (0502,0503,0513) (ECE)'
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat) AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL) AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);
DELETE FROM modelSeries WHERE userId IS NULL AND name = 'K 75 RT (ECE, 10/1989-06/1996)'
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat) AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL) AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);
DELETE FROM modelSeries WHERE userId IS NULL AND name = 'G 310 GS'
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat) AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL) AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);
DELETE FROM modelSeries WHERE userId IS NULL AND name = 'G 310 R'
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat) AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL) AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);
DELETE FROM modelSeries WHERE userId IS NULL AND name = 'K 1600'
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat) AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL) AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);
DELETE FROM modelSeries WHERE userId IS NULL AND name = 'R 25/3, R 26, R 27 (Einzylinder)'
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat) AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL) AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);
DELETE FROM modelSeries WHERE userId IS NULL AND name = 'R 50, R 60, R 69 S (Boxer /2)'
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat) AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL) AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);
DELETE FROM modelSeries WHERE userId IS NULL AND name = 'R 50/5, R 60/5, R 75/5 (69-73)'
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat) AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL) AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);
DELETE FROM modelSeries WHERE userId IS NULL AND name = 'R 60/6, R 75/6, R 90/6, R 90 S (73-76)'
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat) AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL) AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);
DELETE FROM modelSeries WHERE userId IS NULL AND name = 'R 60/7, R 75/7, R 80/7, R 100/7-T-S-RS-RT (76-84)'
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat) AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL) AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);
DELETE FROM modelSeries WHERE userId IS NULL AND name = 'R 45'
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat) AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL) AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);
DELETE FROM modelSeries WHERE userId IS NULL AND name = 'R 65'
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat) AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL) AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);
DELETE FROM modelSeries WHERE userId IS NULL AND name = 'R 80'
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat) AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL) AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);
DELETE FROM modelSeries WHERE userId IS NULL AND name = 'R 100'
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat) AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL) AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);
DELETE FROM modelSeries WHERE userId IS NULL AND name = 'R 80 GS'
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat) AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL) AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);
DELETE FROM modelSeries WHERE userId IS NULL AND name = 'R 100 GS'
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat) AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL) AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);
DELETE FROM modelSeries WHERE userId IS NULL AND name = 'R 1100 GS'
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat) AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL) AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);
DELETE FROM modelSeries WHERE userId IS NULL AND name = 'R 1150 GS'
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat) AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL) AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);
DELETE FROM modelSeries WHERE userId IS NULL AND name = 'R 1200 GS'
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat) AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL) AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);
DELETE FROM modelSeries WHERE userId IS NULL AND name = 'R 1200 R'
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat) AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL) AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);
DELETE FROM modelSeries WHERE userId IS NULL AND name = 'R 1200 RT'
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat) AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL) AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);
DELETE FROM modelSeries WHERE userId IS NULL AND name = 'R nineT'
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat) AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL) AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);
DELETE FROM modelSeries WHERE userId IS NULL AND name = 'K 75'
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat) AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL) AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);
DELETE FROM modelSeries WHERE userId IS NULL AND name = 'K 100'
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat) AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL) AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);
DELETE FROM modelSeries WHERE userId IS NULL AND name = 'K 1100'
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat) AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL) AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);
DELETE FROM modelSeries WHERE userId IS NULL AND name = 'K 1200'
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat) AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL) AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);
DELETE FROM modelSeries WHERE userId IS NULL AND name = 'G-Modelle'
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat) AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL) AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);
DELETE FROM modelSeries WHERE userId IS NULL AND name = 'Modelle ab 2000'
  AND id NOT IN (SELECT seriesId FROM partSeriesCompat) AND id NOT IN (SELECT seriesId FROM motorcycles WHERE seriesId IS NOT NULL) AND id NOT IN (SELECT parentId FROM modelSeries WHERE parentId IS NOT NULL);
