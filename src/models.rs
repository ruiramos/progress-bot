use crate::schema::standups;
use crate::schema::teams;
use crate::schema::users;
use crate::{EventDetails, StandupState};
use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime, Utc};

#[derive(Debug, Queryable, AsChangeset, QueryableByName)]
#[changeset_options(treat_none_as_null = "true")]
#[table_name = "users"]
pub struct User {
    pub id: i32,
    pub username: String,
    pub channel: Option<String>,
    pub reminder: Option<NaiveDateTime>,
    pub real_name: String,
    pub avatar_url: String,
    pub team_id: String,
    pub last_notified: Option<NaiveDateTime>,
}

#[derive(Insertable)]
#[table_name = "users"]
pub struct NewUser<'a> {
    pub username: &'a str,
    pub real_name: &'a str,
    pub avatar_url: &'a str,
    pub team_id: &'a str,
}

#[derive(Debug, Queryable, AsChangeset)]
pub struct Standup {
    pub id: i32,
    pub username: String,
    pub date: NaiveDateTime,
    pub prev_day: Option<String>,
    pub day: Option<String>,
    pub blocker: Option<String>,
    pub message_ts: Option<String>,
    pub channel: Option<String>,
    pub prev_day_message_ts: Option<String>,
    pub day_message_ts: Option<String>,
    pub blocker_message_ts: Option<String>,
    pub team_id: Option<String>,
}

impl Standup {
    pub fn get_state(&self) -> StandupState {
        if self.prev_day.is_none() {
            StandupState::PrevDay
        } else if self.day.is_none() {
            StandupState::Today
        } else if self.blocker.is_none() {
            StandupState::Blocker
        } else {
            StandupState::Complete
        }
    }

    pub fn add_content(&mut self, content: &str, evt: &EventDetails) {
        let ts = evt.ts.as_ref().unwrap().to_string();
        match self.get_state() {
            StandupState::PrevDay => {
                self.prev_day = Some(content.to_string());
                self.prev_day_message_ts = Some(ts);
            }
            StandupState::Today => {
                self.day = Some(content.to_string());
                self.day_message_ts = Some(ts);
            }
            StandupState::Blocker => {
                self.blocker = Some(content.to_string());
                self.blocker_message_ts = Some(ts);
            }
            _ => (),
        }
    }

    pub fn get_copy(&self, channel: &Option<String>) -> String {
        match self.get_state() {
            StandupState::PrevDay => {
                // @TODO
                let prev_day_str = "yesterday";

                format!(
                    ":one: Firstly how did *{}* go? What were you able to achieve? _(use shift+enter to create new lines for separating tasks)_",
                    prev_day_str
                )
            }
            StandupState::Today => {
                ":two: What are you going to be focusing on *today*?".to_string()
            }
            StandupState::Blocker => ":three: Any blockers impacting your work?".to_string(),
            StandupState::Complete => {
                let extra = match channel {
                    None => String::from(""),
                    Some(channel) => format!(
                        "Additionally, I've shared the standup notes to <#{}>.",
                        channel
                    ),
                };

                // @TODO tomorrow_str
                format!(":white_check_mark: *All done here!* {}\n\n Thank you, have a great day and talk to you {}.",
                    extra, "tomorrow"
                )
            }
        }
    }
}

#[derive(Insertable)]
#[table_name = "standups"]
pub struct NewStandup {
    pub username: String,
    pub team_id: Option<String>,
    pub date: NaiveDateTime,
}

impl NewStandup {
    pub fn new(username: &str, team_id: &str) -> NewStandup {
        let now = Utc::now();
        let d = NaiveDate::from_ymd(now.year(), now.month(), now.day());
        let t = NaiveTime::from_hms_milli(0, 0, 0, 0);
        let today = NaiveDateTime::new(d, t);

        NewStandup {
            username: username.to_string(),
            team_id: Some(team_id.to_string()),
            date: today,
        }
    }
}

#[derive(Debug, Queryable, AsChangeset, Insertable)]
pub struct Team {
    pub id: i32,
    pub access_token: String,
    pub team_id: String,
    pub team_name: String,
    pub bot_user_id: String,
    pub bot_access_token: String,
}

#[derive(Insertable)]
#[table_name = "teams"]
pub struct NewTeam {
    pub access_token: String,
    pub team_id: String,
    pub team_name: String,
    pub bot_user_id: String,
    pub bot_access_token: String,
}
