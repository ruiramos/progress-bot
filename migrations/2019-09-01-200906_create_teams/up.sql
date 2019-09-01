-- Your SQL goes here
CREATE TABLE teams (
  id SERIAL PRIMARY KEY,
  access_token VARCHAR NOT NULL,
  team_id VARCHAR NOT NULL,
  team_name VARCHAR NOT NULL,
  bot_user_id VARCHAR NOT NULL,
  bot_access_token VARCHAR NOT NULL
)
