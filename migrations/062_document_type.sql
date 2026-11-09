-- What kind of document this is (workshop manual, wiring diagram, checklist, …).
-- Nullable: NULL means "not set". Allowed keys are validated in the handler:
-- werkstatthandbuch, reparaturanleitung, schaltplan, checkliste, teilekatalog,
-- bedienungsanleitung, fahrzeugausweis, rechnung, sonstiges.
ALTER TABLE documents ADD COLUMN docType TEXT;

-- One-off best-effort backfill from the title. A document is only typed when
-- its title matches exactly one type's keywords; anything else stays NULL.
CREATE TEMP TABLE docTypeBackfill AS
SELECT id, 'werkstatthandbuch' AS docType FROM documents
 WHERE title LIKE '%werkstatthandbuch%' OR title LIKE '%workshop manual%' OR title LIKE '%service manual%'
UNION ALL
SELECT id, 'reparaturanleitung' FROM documents
 WHERE title LIKE '%reparaturanleitung%' OR title LIKE '%repair%'
UNION ALL
SELECT id, 'schaltplan' FROM documents
 WHERE title LIKE '%schaltplan%' OR title LIKE '%wiring%'
UNION ALL
SELECT id, 'checkliste' FROM documents
 WHERE title LIKE '%checklist%'
UNION ALL
SELECT id, 'teilekatalog' FROM documents
 WHERE title LIKE '%teilekatalog%' OR title LIKE '%parts catalog%'
UNION ALL
SELECT id, 'bedienungsanleitung' FROM documents
 WHERE title LIKE '%bedienungsanleitung%' OR title LIKE '%betriebsanleitung%'
    OR title LIKE '%owner''s manual%' OR title LIKE '%owners manual%'
UNION ALL
SELECT id, 'fahrzeugausweis' FROM documents
 WHERE title LIKE '%fahrzeugausweis%'
UNION ALL
SELECT id, 'rechnung' FROM documents
 WHERE title LIKE 'rechnung%' OR title LIKE '% rechnung%' OR title LIKE '%invoice%';

UPDATE documents
   SET docType = (SELECT b.docType FROM docTypeBackfill b WHERE b.id = documents.id LIMIT 1)
 WHERE docType IS NULL
   AND id IN (SELECT id FROM docTypeBackfill GROUP BY id HAVING COUNT(DISTINCT docType) = 1);

DROP TABLE docTypeBackfill;
