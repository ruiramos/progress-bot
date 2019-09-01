-- Your SQL goes here
CREATE TABLE users (
  id SERIAL PRIMARY KEY,
  username VARCHAR NOT NULL,
  channel VARCHAR,
  reminder timestamp,
  real_name VARCHAR NOT NULL,
  avatar_url VARCHAR NOT NULL
)
