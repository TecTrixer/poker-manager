-- speed_steps: positive = faster (0.9^n factor on future levels), negative = slower
ALTER TABLE games ADD COLUMN speed_steps INTEGER NOT NULL DEFAULT 0;
