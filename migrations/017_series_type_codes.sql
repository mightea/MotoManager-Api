-- BMW type codes (Baumuster) on catalog entries, for VIN decoding: in
-- 17-character BMW Motorrad VINs (WB1...), characters 4-7 hold the 4-digit
-- type code identifying the exact model/market variant. Comma-separated
-- because most entries group several market variants.
--
-- Codes sourced from Snowbum's model-code table
-- (bmwmotorcycletech.info/id-yr-codes-pwr.htm); only confirmed codes are
-- seeded — unknown ones (K 1100, 4V oilheads) are left empty and can be
-- added via the Modellkatalog. Updates match global rows by name and no-op
-- when the row was pruned or renamed.

ALTER TABLE modelSeries ADD COLUMN typeCodes TEXT;

-- Serien -------------------------------------------------------------------
UPDATE modelSeries SET typeCodes = '0240,0250,0251,0255,0281,0260'
    WHERE userId IS NULL AND name = 'R 50/5, R 60/5, R 75/5 (69-73)';

UPDATE modelSeries SET typeCodes = '0253,0257,0291,0261,0265,0282,0263,0267,0292,0271,0275,0283,0273,0277,0293,0272,0276,0284,0274,0278,0294'
    WHERE userId IS NULL AND name = 'R 60/6, R 75/6, R 90/6, R 90 S (73-76)';

UPDATE modelSeries SET typeCodes = '0301,0321,0371,0311,0331,0341,0381,0302,0326,0372,0312,0382,0332,0392,0327,0373,0423,0322,0374,0342,0383,0443,0343,0384,0304,0323,0375,0425,0314,0333,0393,0435,0344,0385,0445,0398,0305,0324,0376,0315,0334,0394,0426,0436,0306,0325,0377,0427,0316,0335,0395,0437,0345,0386,0339'
    WHERE userId IS NULL AND name = 'R 60/7, R 75/7, R 80/7, R 100/7-T-S-RS-RT (76-84)';

UPDATE modelSeries SET typeCodes = '0346,0471,0348,0347,0349'
    WHERE userId IS NULL AND name = 'R 80 G/S, R 80 ST (80-87)';

UPDATE modelSeries SET typeCodes = '0451,0452,0462,0453,0463,0456,0444,0457,0448,0464'
    WHERE userId IS NULL AND name = 'R 65, R 65 RT, R 80, R 80 RT (85-95)';

UPDATE modelSeries SET typeCodes = '0473,0478,0479,0489'
    WHERE userId IS NULL AND name = 'R 80 GS, R 100 GS, PD (90-95)';

UPDATE modelSeries SET typeCodes = '0455,0466,0459,0469,0449,0460'
    WHERE userId IS NULL AND name = 'R 100 RS, R 100 RT (87-95)';

UPDATE modelSeries SET typeCodes = '0476,0487'
    WHERE userId IS NULL AND name = 'R 80 R, R 100 R, Mystic (91-96)';

UPDATE modelSeries SET typeCodes = '0561,0562,0563,0564,0571,0572,0573,0574'
    WHERE userId IS NULL AND name = 'K569 (K 75, K 75 C, K 75 S, K 75 RT)';

UPDATE modelSeries SET typeCodes = '0501,0502,0503,0504,0505,0506,0511,0513,0514,0516,0521'
    WHERE userId IS NULL AND name = 'K589 (K 100, RS, RT, LT)';

-- K100RS 16V is the 4-valve platform.
UPDATE modelSeries SET typeCodes = '0533'
    WHERE userId IS NULL AND name = 'K589 4V (K 1100 LT, K 1100 RS)';

-- Twin-shock leftovers on the re-parented flat entries -----------------------
UPDATE modelSeries SET typeCodes = '0351,0355,0352,0354'
    WHERE userId IS NULL AND name = 'R 45';

UPDATE modelSeries SET typeCodes = '0353,0358,0363,0364,0359,0365,0360,0482'
    WHERE userId IS NULL AND name = 'R 65';

UPDATE modelSeries SET typeCodes = '0461'
    WHERE userId IS NULL AND name = 'R 100';

-- Modelle (deepest match wins over the surrounding Serie) --------------------
UPDATE modelSeries SET typeCodes = '0502,0503,0513'
    WHERE userId IS NULL AND name = 'K 100 RS 83 (0502,0503,0513) (ECE)';

UPDATE modelSeries SET typeCodes = '0473'
    WHERE userId IS NULL AND name = 'R 100 GS (ECE, 04/1990-07/1996)';

UPDATE modelSeries SET typeCodes = '0452'
    WHERE userId IS NULL AND name = 'R 65 (35KW) (ECE, 06/1985-12/1992)';

UPDATE modelSeries SET typeCodes = '0453'
    WHERE userId IS NULL AND name = 'R 80 (ECE, 03/1984-01/1995)';

UPDATE modelSeries SET typeCodes = '0444,0457'
    WHERE userId IS NULL AND name = 'R 80 RT (ECE, 07/1984-12/1995)';
