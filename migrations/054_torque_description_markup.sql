-- Inline formatting for torque spec notes (bold / italic / brand colors).
-- `description` stays the plain-text form so older iOS builds keep reading and
-- writing it unchanged; `descriptionMarkup` carries the formatted twin and is
-- only honoured by clients when stripping it yields `description`. The update
-- handler clears it whenever a client changes `description` without sending
-- markup (i.e. an older build edited the entry).
ALTER TABLE torqueSpecs ADD COLUMN descriptionMarkup TEXT;
