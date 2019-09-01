-- Your SQL goes here
CREATE TABLE standups (
  id SERIAL PRIMARY KEY,
  username VARCHAR NOT NULL,
  date timestamp NOT NULL,
  prev_day VARCHAR,
  day VARCHAR,
  blocker VARCHAR
)
