-- This file should undo anything in `up.sql`
ALTER TABLE links
DROP COLUMN visible,
DROP COLUMN visitors;
