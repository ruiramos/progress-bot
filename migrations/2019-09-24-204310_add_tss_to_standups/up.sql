-- Your SQL goes here
ALTER TABLE standups ADD COLUMN prev_day_message_ts varchar;
ALTER TABLE standups ADD COLUMN day_message_ts varchar;
ALTER TABLE standups ADD COLUMN blocker_message_ts varchar;
