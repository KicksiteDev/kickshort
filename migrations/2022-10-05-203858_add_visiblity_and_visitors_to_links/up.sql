-- Your SQL goes here
ALTER TABLE links
ADD COLUMN visibility BOOLEAN NOT NULL DEFAULT TRUE,
ADD COLUMN visitors INTEGER NOT NULL DEFAULT 0;