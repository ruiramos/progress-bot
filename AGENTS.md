# Progress Bot - Codebase Context for AI Agents

## Overview
Progress Bot is a Slack bot for managing daily standups and task tracking. Written in Rust using:
- **Web Framework**: Rocket 0.5.1 (async)
- **Database**: PostgreSQL via Diesel 2.1 ORM
- **HTTP Client**: reqwest 0.12 (blocking)
- **Runtime**: Tokio 1.x

## Architecture

### Core Components
```
src/
├── bin/
│   ├── main.rs          # Rocket web server with HTTP routes
│   └── reminders.rs     # Standalone reminder daemon
├── lib.rs               # Data access layer (CRUD operations)
├── models.rs            # Diesel models (User, Standup, Team)
├── schema.rs            # Database schema (auto-generated)
├── handle.rs            # Event routing and business logic
└── slack.rs             # Slack API client wrapper
```

### Data Model

**users** - Slack users with preferences
- Tracks: username (Slack ID), reminder time, channel preference, team_id
- Config: reminder hour, optional channel for posting standups
- Prevents duplicate notifications via last_notified timestamp

**standups** - Daily standup entries
- Three sections: prev_day, day (tasks), blocker
- State machine: PrevDay → Today → Blocker → Complete
- Task tracking: done integer array maps to line-numbered tasks
- Message tracking: multiple _message_ts fields for Slack message updates
- Date normalized to UTC midnight for querying

**teams** - Multi-workspace support
- Stores access_token, bot_access_token per Slack workspace
- Used for OAuth flow and API authentication

## Key Features

### 1. Standup Collection Flow
1. User DMs bot or mentions @progress in channel
2. Bot shows intro with previous standup tasks (if exists)
3. Three-step interactive collection:
   - "What did you accomplish yesterday?" (can skip)
   - "What are you working on today?" (multi-line, numbered tasks)
   - "Any blockers?"
4. Optionally posts summary to configured channel
5. Tasks become actionable items for tracking

**Implementation**: `handle.rs:handle_message()` routes by standup state

### 2. Task Management
- Tasks extracted from newline-separated "day" field
- Numbered with emoji (:one:, :two:, etc.)
- Mark done: `/d <number>` or button in intro message
- Mark undone: `/ud <number>` or button
- State stored in `done` integer array (0-indexed internally)
- Strikethrough formatting for completed tasks

**Implementation**: `handle.rs:handle_done()`, `handle.rs:handle_undone()`

### 3. Message Editing
- Bot detects `message_changed` events
- Matches message_ts to standup field (prev_day_message_ts, day_message_ts, etc.)
- Updates corresponding database field
- Updates channel message if standup was shared

**Implementation**: `handle.rs:handle_message_change()`

### 4. Daily Reminders
- Separate daemon (`reminders.rs`) runs continuously
- Queries users needing reminders at current hour
- Conditions: matching reminder time, no standup today, not weekend, not already notified
- Sends DM to start standup flow

**Implementation**: `bin/reminders.rs:send_reminders()`

### 5. Configuration
- `/progress-config` opens dialog
- Options: channel selection, reminder hour (7am-1pm)
- Interactive components handled at `/config` endpoint

**Implementation**: `slack.rs:build_config_dialog()`, `main.rs:config()`

## Request Flow

### Slack Events → Bot
1. Slack sends POST to `/` endpoint
2. Challenge verification for event subscription setup
3. Event routing in `main.rs:events()`
4. Delegated to `handle.rs:handle_event()`
5. Event type determines handler:
   - `message` → `handle_message()`
   - `message_changed` → `handle_message_change()`
   - `app_mention` → `handle_app_mention()`
   - `app_home_opened` → `handle_app_home_opened()`

### Bot → Slack
1. Business logic determines response
2. `slack.rs` functions wrap Slack API:
   - `send_message()` - Post new message
   - `send_to_channel()` - Post standup summary
   - `update_message()` - Edit existing message
   - `delete_message()` - Remove message
3. Uses bot_access_token from teams table
4. Blocking HTTP calls with reqwest

### OAuth Flow
1. User installs app → Redirected to Slack OAuth
2. Slack redirects to `/oauth?code=<code>`
3. Exchange code for tokens via `slack.rs:oauth()`
4. Store/update team record in database
5. Returns success page

## Key Patterns

### State Machine
Standup has 4 states determined by field presence:
```rust
PrevDay  → prev_day is None
Today    → prev_day is Some, day is None
Blocker  → day is Some, blocker is None
Complete → all fields are Some
```
Implemented in `models.rs:Standup::get_state()`

### Async/Sync Hybrid
- Rocket handlers are async
- Diesel operations wrapped in `conn.run(|c| { ... })` for sync execution
- Slack API uses blocking reqwest in spawned tasks
- Pattern: `tokio::spawn(move || { blocking_work() })`

### Date Handling
- User input timestamp stored as local_date
- Normalized to UTC midnight in date field
- Today's standup: `WHERE date = <today_utc_midnight>`
- Uses chrono for timezone conversions

### Message Timestamp Tracking
Every standup interaction creates Slack messages that may need updates:
- `message_ts` - Main channel post
- `prev_day_message_ts` - Question message in DM
- `day_message_ts` - Question message in DM
- `blocker_message_ts` - Question message in DM
- `intro_message_ts` - Intro message with task buttons

Enables edit detection and message updates.

### Multi-line Task Parsing
```rust
let tasks: Vec<&str> = standup.day.split('\n').collect();
// tasks[0] is task #1 for users (1-indexed)
// done array stores 0-indexed positions
```

### Error Handling
- Heavy use of `Option<T>` for nullable fields
- `.unwrap_or()` / `.unwrap_or_else()` for defaults
- Errors logged but don't crash handlers
- Graceful degradation (e.g., missing avatar URL)

## Database Access Patterns

All database operations in `lib.rs`:
- `get_user()` / `create_user()` / `update_user()`
- `get_standup()` / `create_standup()` / `update_standup()` / `delete_standup()`
- `get_team()` / `create_team()` / `update_team()`
- Date queries use UTC midnight: `date.date_naive().and_hms_opt(0, 0, 0)`

Connection managed by rocket_sync_db_pools with macro:
```rust
#[database("diesel_postgres_pool")]
pub struct DbConn(diesel::PgConnection);
```

## Configuration

### Environment Variables
```
DATABASE_URL=postgres://user:pass@host:port/database
CLIENT_ID=<Slack app client ID>
CLIENT_SECRET=<Slack app client secret>
PORT=8800  # Optional, defaults to 8800
```

### Rocket.toml
```toml
[default.databases.diesel_postgres_pool]
url = "env:DATABASE_URL"
```

### Heroku Deployment (Procfile)
```
release: ./target/release/main  # Runs migrations
web: ./target/release/main
```

## Slash Commands

Registered in Slack app config, handled in `main.rs`:
- `/progress-config` → Show configuration dialog
- `/progress-forget` → Delete today's standup
- `/progress-help` → Show help text
- `/progress-today` (or `/td`) → Show today's tasks
- `/progress-done` (or `/d`) → Mark task done
- `/progress-undo` (or `/ud`) → Mark task not done
- `/progress-add` → Add task to today

## Common Development Tasks

### Running Locally
```bash
docker-compose up          # Start PostgreSQL
diesel migration run       # Apply migrations
cargo run                  # Start web server (port 8800)
cargo run --bin reminders  # Start reminder daemon (separate process)
```

### Testing
```bash
cargo test  # Runs unit tests in lib.rs
```

### Database Migrations
```bash
diesel migration generate <name>  # Create new migration
diesel migration run               # Apply migrations
diesel migration revert            # Rollback last migration
```

## Important Files

- `Cargo.toml` - Dependencies and project metadata
- `Rocket.toml` - Web server and database configuration
- `rust-toolchain.toml` - Rust version specification
- `docker-compose.yml` - Local PostgreSQL setup
- `.env` - Environment variables (not in git)
- `Procfile` - Heroku deployment config

## Known Limitations & TODOs

- Timezone handling could be improved (currently UTC-centric)
- "yesterday/tomorrow" strings are hardcoded
- No pagination for tasks (assumes reasonable daily task count)
- Blocking HTTP calls in async context (works but not optimal)
- No retry logic for Slack API failures
- Test coverage could be expanded beyond lib.rs

## Dependencies Worth Noting

- **rocket 0.5** - Modern async web framework, significant API changes from 0.4
- **diesel 2.1** - ORM with compile-time query checking
- **rocket_sync_db_pools** - Bridges async Rocket with sync Diesel
- **chrono 0.4** - Date/time library, some deprecated methods used
- **serde/serde_json** - JSON serialization for Slack API
- **reqwest 0.12** - HTTP client in blocking mode
- **dotenvy** - Loads .env files for local development

## Migration History

14 migrations from 2019-2020 covering:
1. Initial database setup
2. User/standup/team table creation
3. Message timestamp tracking additions
4. Channel configuration
5. Task completion tracking (done array)
6. Local date and intro message tracking

Schema is stable, no recent migrations.
