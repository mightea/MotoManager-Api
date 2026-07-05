-- Part photos. Stores the image filename (same convention as motorcycles.image);
-- uploads go through POST /api/parts/{id}/image, which bumps updatedAt so the
-- change flows to offline clients via the ?since delta sync.
ALTER TABLE parts ADD COLUMN image TEXT;
