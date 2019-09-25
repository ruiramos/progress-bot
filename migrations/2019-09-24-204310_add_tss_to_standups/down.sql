-- This file should undo anything in `up.sql`
ALTER TABLE standups DROP COLUMN prev_day_message_ts 
ALTER TABLE standups DROP COLUMN day_message_ts 
ALTER TABLE standups DROP COLUMN blocker_message_ts 
