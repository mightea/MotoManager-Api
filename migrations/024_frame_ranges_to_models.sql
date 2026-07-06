-- Push frame-number blocks down to the most specific catalog Modell so a
-- decoded serial assigns e.g. a K 75 S to 'K 75 S (0563,0572) (ECE, ...)'
-- instead of stopping at the K569 Serie. Blocks that cannot be pinned to
-- a single Modell (year-split /6 variants, K75 Authorities) stay on the
-- Serie as fallback; the decoder always picks the deepest match.

UPDATE modelSeries SET frameRanges = '200009-212007' WHERE userId IS NULL AND name = 'R24 (ECE, 01/1949-02/1950)';
UPDATE modelSeries SET frameRanges = '220011-243410' WHERE userId IS NULL AND name = 'R25 (ECE, 04/1950-09/1951)';
UPDATE modelSeries SET frameRanges = '245001-283650' WHERE userId IS NULL AND name = 'R25/2 (ECE, 10/1951-08/1953)';
UPDATE modelSeries SET frameRanges = '284001-331705' WHERE userId IS NULL AND name = 'R25/3 (ECE, 09/1953-07/1955)';
UPDATE modelSeries SET frameRanges = '340001-370242' WHERE userId IS NULL AND name = 'R26 (ECE, 11/1955-07/1960)';
UPDATE modelSeries SET frameRanges = '372001-387566' WHERE userId IS NULL AND name = 'R27 (ECE, 10/1960-10/1966)';
UPDATE modelSeries SET frameRanges = '550001-563515' WHERE userId IS NULL AND name = 'R50 (ECE, 03/1955-09/1960)';
UPDATE modelSeries SET frameRanges = '564001-565634' WHERE userId IS NULL AND name = 'R50 S (ECE, 01/1961-10/1962)';
UPDATE modelSeries SET frameRanges = '630001-649037' WHERE userId IS NULL AND name = 'R50/2 (ECE, 01/1961-12/1969)';
UPDATE modelSeries SET frameRanges = '516001-521005' WHERE userId IS NULL AND name = 'R51/2 (ECE, 01/1950-12/1950)';
UPDATE modelSeries SET frameRanges = '522001-540950' WHERE userId IS NULL AND name = 'R51/3 (ECE, 12/1950-07/1954)';
UPDATE modelSeries SET frameRanges = '618001-621530' WHERE userId IS NULL AND name = 'R60 (ECE, 04/1956-07/1960)';
UPDATE modelSeries SET frameRanges = '622001-629999,1810001-1819307' WHERE userId IS NULL AND name = 'R60/2 (ECE, 09/1960-11/1967)';
UPDATE modelSeries SET frameRanges = '610001-611445' WHERE userId IS NULL AND name = 'R67 (ECE, 01/1951-10/1951)';
UPDATE modelSeries SET frameRanges = '612001-617700' WHERE userId IS NULL AND name = 'R67/2/3 (ECE, 12/1951-12/1955)';
UPDATE modelSeries SET frameRanges = '650001-651453' WHERE userId IS NULL AND name = 'R68 (ECE, 03/1952-07/1954)';
UPDATE modelSeries SET frameRanges = '652001-654955' WHERE userId IS NULL AND name = 'R69 (ECE, 04/1955-07/1960)';
UPDATE modelSeries SET frameRanges = '655004-666320' WHERE userId IS NULL AND name = 'R69 S (ECE, 10/1960-07/1969)';
UPDATE modelSeries SET frameRanges = '2900001-2910000' WHERE userId IS NULL AND name = 'R50/5 (ECE, 08/1969-07/1973)';
UPDATE modelSeries SET frameRanges = '2930001-2950000' WHERE userId IS NULL AND name = 'R60/5 (ECE, 08/1969-07/1973)';
UPDATE modelSeries SET frameRanges = '2970001-3000000' WHERE userId IS NULL AND name = 'R75/5 (ECE, 08/1969-07/1980)';
UPDATE modelSeries SET frameRanges = '6430001-6435000' WHERE userId IS NULL AND name = 'R 65 (20KW) (ECE, 04/1985-06/1993)';
UPDATE modelSeries SET frameRanges = '6073001-6075000,6118001-6119000' WHERE userId IS NULL AND name = 'R 65 (35KW) (ECE, 06/1985-10/1988)';
UPDATE modelSeries SET frameRanges = '6440001-6449000' WHERE userId IS NULL AND name = 'R 80 (ECE, 03/1984-01/1995)';
UPDATE modelSeries SET frameRanges = '6420001-6425000,6470001-6480000,6483001-6486000' WHERE userId IS NULL AND name = 'R 80 RT (ECE, 07/1984-12/1995)';
UPDATE modelSeries SET frameRanges = '6125001-6126000' WHERE userId IS NULL AND name = 'R 65 GS (ECE, 11/1987-11/1992)';
UPDATE modelSeries SET frameRanges = '6250001-6260000,6281001-6290000,6291001-6292600,6245001-6247000' WHERE userId IS NULL AND name = 'R 80 G/S (ECE, 05/1980-07/1987)';
UPDATE modelSeries SET frameRanges = '6054001-6060000' WHERE userId IS NULL AND name = 'R 80 ST (ECE, 04/1982-10/1984)';
UPDATE modelSeries SET frameRanges = '6276001-6280000' WHERE userId IS NULL AND name = 'R 100 GS (ECE, 12/1986-07/1990)';
UPDATE modelSeries SET frameRanges = '6331001-6336000' WHERE userId IS NULL AND name = 'R 100 GS PD (ECE, 02/1989-07/1990)';
UPDATE modelSeries SET frameRanges = '0160001-0163000' WHERE userId IS NULL AND name = 'R 100 RS (ECE, 07/1986-10/1992)';
UPDATE modelSeries SET frameRanges = '6016001-6018000,6247001-6248000' WHERE userId IS NULL AND name = 'R 100 RT (ECE, 07/1987-12/1995)';
UPDATE modelSeries SET frameRanges = '6034001-6035000' WHERE userId IS NULL AND name = 'R 80 R 91 (ECE, 03/1991-06/1994)';
UPDATE modelSeries SET frameRanges = '0100001-0110000' WHERE userId IS NULL AND name = 'K 75 S (0563,0572) (ECE, 10/1985-05/1995)';
UPDATE modelSeries SET frameRanges = '0110001-0118000' WHERE userId IS NULL AND name = 'K 75 C (0564,0574) (ECE, 06/1985-06/1988)';
UPDATE modelSeries SET frameRanges = '0000001-0010000,0033001-0033500' WHERE userId IS NULL AND name = 'K 100 83 (0501,0511) (ECE, 05/1982-12/1988)';
UPDATE modelSeries SET frameRanges = '0010001-0020000,0140001-0145000,0060001-0061000' WHERE userId IS NULL AND name = 'K 100 RS 83 (0502,0503,0513) (ECE, 05/1983-10/1989)';
UPDATE modelSeries SET frameRanges = '0020001-0030000,0061001-0065000,0070001-0071000,0090001-0100000' WHERE userId IS NULL AND name = 'K 100 RT 84 (0504,0505,0514) (ECE, 10/1983-07/1989)';
UPDATE modelSeries SET frameRanges = '6365002-6365406' WHERE userId IS NULL AND name = 'K 1 (0525,0535) (ECE, 08/1988-09/1993)';
UPDATE modelSeries SET frameRanges = '0080001-0090000' WHERE userId IS NULL AND name = 'K 100 RS (0523,0533) (ECE, 03/1989-06/1992)';

UPDATE modelSeries SET frameRanges = NULL WHERE userId IS NULL AND name = 'R 24 -50';
UPDATE modelSeries SET frameRanges = NULL WHERE userId IS NULL AND name = 'R 25 -56';
UPDATE modelSeries SET frameRanges = NULL WHERE userId IS NULL AND name = 'R 26 -60';
UPDATE modelSeries SET frameRanges = NULL WHERE userId IS NULL AND name = 'R 27 -66';
UPDATE modelSeries SET frameRanges = NULL WHERE userId IS NULL AND name = 'R 50 -69';
UPDATE modelSeries SET frameRanges = NULL WHERE userId IS NULL AND name = 'R 51 -54';
UPDATE modelSeries SET frameRanges = NULL WHERE userId IS NULL AND name = 'R 60 -69';
UPDATE modelSeries SET frameRanges = NULL WHERE userId IS NULL AND name = 'R 67 -55';
UPDATE modelSeries SET frameRanges = NULL WHERE userId IS NULL AND name = 'R 68 -54';
UPDATE modelSeries SET frameRanges = NULL WHERE userId IS NULL AND name = 'R 69 -69';
UPDATE modelSeries SET frameRanges = '4900001-4947578,4040001-4100000,4950001-4991260' WHERE userId IS NULL AND name = 'R50/5-R90S 69-76';
UPDATE modelSeries SET frameRanges = NULL WHERE userId IS NULL AND name = 'R 65, R 65 RT, R 80, R 80 RT (85-95)';
UPDATE modelSeries SET frameRanges = NULL WHERE userId IS NULL AND name = 'R 65 GS, R 80 G/S, R 80 ST (80-92)';
UPDATE modelSeries SET frameRanges = NULL WHERE userId IS NULL AND name = 'R 80 GS, R 100 GS, PD (87-90)';
UPDATE modelSeries SET frameRanges = NULL WHERE userId IS NULL AND name = 'R 100 RS, R 100 RT (87-95)';
UPDATE modelSeries SET frameRanges = NULL WHERE userId IS NULL AND name = 'R 80 R, R 100 R, Mystik (91-95)';
UPDATE modelSeries SET frameRanges = '0120001-0121000,6215001-6219999' WHERE userId IS NULL AND name = 'K569 (K 75, K 75 c, K 75 s, K 75 RT)';
UPDATE modelSeries SET frameRanges = NULL WHERE userId IS NULL AND name = 'K589 (K100, RS, RT, LT)';
UPDATE modelSeries SET frameRanges = NULL WHERE userId IS NULL AND name = 'K589 (K1, K 100 RS)';
