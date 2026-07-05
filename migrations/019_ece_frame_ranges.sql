-- ECE-market frame ranges for K bikes and monolever/paralever airheads:
-- European BMWs kept short 7-digit frame numbers (often with leading zeros,
-- e.g. "0103596 K75S") well into the 90s — only US bikes carried 17-char
-- VINs. Ranges from Snowbum's serial table; leading zeros are irrelevant to
-- the numeric range match. The K75-Authorities block is trimmed to 6219999
-- to avoid colliding with the R75/7 block starting at 6220000.

UPDATE modelSeries SET frameRanges =
    '0100001-0110000,0110001-0118000,0120001-0121000,6215001-6219999'
    WHERE userId IS NULL AND name = 'K569 (K 75, K 75 C, K 75 S, K 75 RT)';

UPDATE modelSeries SET frameRanges =
    '0000001-0010000,0033001-0033500,0010001-0020000,0140001-0145000,0060001-0061000,0020001-0030000,0061001-0065000,0090001-0100000,0070001-0071000'
    WHERE userId IS NULL AND name = 'K589 (K 100, RS, RT, LT)';

-- K100RS 4V (12/1989+)
UPDATE modelSeries SET frameRanges = '0080001-0090000'
    WHERE userId IS NULL AND name = 'K589 4V (K 1100 LT, K 1100 RS)';

-- Monolever R65/R80 (incl. R65/20 6430001+, R65/35 6073001+/6118001+,
-- R65GS 6125001+, R80 6440001+, R80RT 6420001+/6470001+/6483001+)
UPDATE modelSeries SET frameRanges =
    '6430001-6435000,6073001-6075000,6118001-6119000,6125001-6126000,6440001-6449000,6420001-6425000,6470001-6480000,6483001-6486000'
    WHERE userId IS NULL AND name = 'R 65, R 65 RT, R 80, R 80 RT (85-95)';

-- R80 G/S + R80 ST (incl. Paris-Dakar 6245001+)
UPDATE modelSeries SET frameRanges =
    '6250001-6260000,6281001-6290000,6291001-6292600,6245001-6247000,6054001-6060000'
    WHERE userId IS NULL AND name = 'R 80 G/S, R 80 ST (80-87)';

-- Monolever R100RS 44kW + R100RT
UPDATE modelSeries SET frameRanges = '0160001-0163000,6016001-6018000,6247001-6248000'
    WHERE userId IS NULL AND name = 'R 100 RS, R 100 RT (87-95)';

-- Paralever R100GS (incl. Paris-Dakar 6331001+)
UPDATE modelSeries SET frameRanges = '6276001-6280000,6331001-6336000'
    WHERE userId IS NULL AND name = 'R 80 GS, R 100 GS, PD (90-95)';

UPDATE modelSeries SET frameRanges = '6034001-6035000'
    WHERE userId IS NULL AND name = 'R 80 R, R 100 R, Mystic (91-96)';
