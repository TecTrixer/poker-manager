ALTER TABLE games ADD COLUMN players_left INTEGER NOT NULL DEFAULT 0;

UPDATE games SET players_left = num_players WHERE players_left = 0;
