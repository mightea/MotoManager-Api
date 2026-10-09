-- Formatted twins for the remaining free-text fields (see 054 for the torque
-- note pattern). Each plain column stays the compatibility surface older iOS
-- builds read and write; the markup column is only honoured by clients while
-- stripping it yields the plain text, and the update handlers clear it when
-- an older client changes the plain text without sending markup.
ALTER TABLE maintenanceRecords ADD COLUMN descriptionMarkup TEXT;
ALTER TABLE issues ADD COLUMN descriptionMarkup TEXT;
ALTER TABLE expenses ADD COLUMN descriptionMarkup TEXT;
ALTER TABLE parts ADD COLUMN descriptionMarkup TEXT;
ALTER TABLE partStocks ADD COLUMN notesMarkup TEXT;
ALTER TABLE previousOwners ADD COLUMN commentsMarkup TEXT;
