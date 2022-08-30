-- Your SQL goes here
CREATE TABLE links (
  id SERIAL PRIMARY KEY,
  url VARCHAR NOT NULL,
  hash VARCHAR NOT NULL,
  expires_at TIMESTAMP,
  created_at TIMESTAMP NOT NULL DEFAULT NOW()
)
