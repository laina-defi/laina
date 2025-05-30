-- Your SQL goes here
CREATE TABLE prices (
  id SERIAL PRIMARY KEY,
  address TEXT NOT NULL,
  twap BIGINT NOT NULL
)
