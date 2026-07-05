-- K1 frame range (Snowbum: 6365002-6365406, 1990, code 0535). The catalog
-- has no K1 entry, so both the range and the type code anchor at the
-- 4-valve K family. Other gaps (K75 RT ECE, K 1100, PD ECE, Mystic) are not
-- covered by the source table and stay editable via the Modellkatalog.
UPDATE modelSeries SET
    frameRanges = '6365002-6365406',
    typeCodes = CASE
        WHEN typeCodes IS NULL OR typeCodes = '' THEN '0535'
        ELSE typeCodes || ',0535'
    END
    WHERE userId IS NULL AND name = 'K-Modelle 4-Zyl. 4V';
