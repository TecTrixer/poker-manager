-- Add selected flag to support game management (only one game is "active" at a time)
ALTER TABLE games ADD COLUMN selected INTEGER NOT NULL DEFAULT 0;
-- Add name for easier identification in the games list
ALTER TABLE games ADD COLUMN name TEXT NOT NULL DEFAULT '';

-- Mark the most recent non-ended game as selected (if any)
UPDATE games SET selected = 1
WHERE id = (SELECT id FROM games WHERE status != 'ended' ORDER BY id DESC LIMIT 1);
