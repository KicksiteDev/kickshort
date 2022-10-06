-- This file should undo anything in `up.sql`
ALTER TABLE links
ADD COLUMN expires_at TIMESTAMP;
