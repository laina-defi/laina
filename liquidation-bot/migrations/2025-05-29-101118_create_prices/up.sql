-- Your SQL goes here
CREATE TABLE prices (
  id SERIAL PRIMARY KEY,
  pool_address TEXT NOT NULL,
  time_weighted_average_price BIGINT NOT NULL
)
