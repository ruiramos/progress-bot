#[macro_use]
extern crate serde_derive;
extern crate chrono;

pub mod handle;
pub mod slack;
use chrono::prelude::*;
use rocket::request::FromForm;
use std::collections::HashMap;

#[derive(Deserialize, Debug)]
pub struct SlackEvent {
    pub token: String,
    pub challenge: Option<String>,
    pub event: Option<EventDetails>,
}

#[derive(Deserialize, Debug, FromForm)]
pub struct SlackSlashEvent {
    pub token: String,
    pub response_url: String,
    pub trigger_id: String,
    pub user_id: String,
}

#[derive(Deserialize, Debug)]
pub struct EventDetails {
    pub text: String,
    pub user: String,
    pub channel: String,
    pub r#type: String,
    pub bot_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SlackConfigResource {
    pub id: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SlackConfigSubmission {
    pub reminder: Option<String>,
    pub channel: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SlackConfig {
    pub user: SlackConfigResource,
    pub channel: SlackConfigResource,
    pub submission: SlackConfigSubmission,
    pub response_url: String,
}

#[derive(Debug, FromForm)]
pub struct SlackConfigResponse {
    pub payload: String,
}

#[derive(Debug)]
pub struct User {
    username: String,
    channel: Option<String>,
    reminder: Option<DateTime<Utc>>,
    real_name: Option<String>,
    avatar_url: Option<String>,
}

impl User {
    pub fn new(username: &str) -> User {
        User {
            username: String::from(username),
            channel: None,
            reminder: None,
            real_name: None,
            avatar_url: None,
        }
    }

    pub fn update_config(&mut self, config: &SlackConfig) {
        self.channel = config.submission.channel.clone();
        //self.reminder = Some(config.submission.reminder);
    }
}

pub type UserList = HashMap<String, User>;

pub enum StandupState {
    PrevDay,
    Today,
    Blocker,
    Complete,
}

#[derive(Debug)]
pub struct Standup {
    user: String,
    date: Date<Utc>,

    // not sure if it would be better to encapsulate this?
    prev_day: Option<String>,
    day: Option<String>,
    blocker: Option<String>,
}

impl Standup {
    pub fn new(user: &str) -> Standup {
        Standup {
            user: String::from(user),
            date: Utc::now().date(),
            prev_day: None,
            day: None,
            blocker: None,
        }
    }

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

    pub fn add_content(&mut self, content: &str) {
        match self.get_state() {
            StandupState::PrevDay => self.prev_day = Some(content.to_string()),
            StandupState::Today => self.day = Some(content.to_string()),
            StandupState::Blocker => self.blocker = Some(content.to_string()),
            _ => (),
        }
    }

    pub fn get_copy(&self, channel: &Option<String>) -> String {
        match self.get_state() {
            StandupState::PrevDay => {
                ":two: What are you going to be focusing on *today*?".to_string()
            }
            StandupState::Today => ":three: Any blockers impacting your work?".to_string(),
            StandupState::Blocker => {
                let extra = match channel {
                    None => String::from(""),
                    Some(channel) => format!(
                        "Additionally, I've shared the standup notes to <#{}>.",
                        channel
                    ),
                };

                format!(":white_check_mark: *All done here!* {}\n\n Thank you, have a great day and talk to you {}.",
                    extra, "tomorrow"
                )
            }
            StandupState::Complete => {
                "You're done for today, off to work you go now! :nerd_face:".to_string()
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct StandupList {
    list: Vec<Standup>,
}

impl StandupList {
    pub fn new() -> StandupList {
        StandupList::default()
    }

    pub fn add_standup(&mut self, s: Standup) {
        self.list.push(s);
    }

    pub fn get_todays_mut(&mut self, user: &str) -> Option<&mut Standup> {
        let today = Utc::now().date();
        self.list
            .iter_mut()
            .filter(|standup| standup.user == user && standup.date == today)
            .take(1)
            .next()
    }

    pub fn get_latest(&self, user: &str) -> Option<&Standup> {
        let mut lower_date = Utc.ymd(1990, 1, 1);
        self.list
            .iter()
            .filter(|standup| standup.user == user)
            .fold(None, |acc, x| {
                if x.date > lower_date {
                    lower_date = x.date;
                    Some(x)
                } else {
                    acc
                }
            })
    }

    pub fn remove_todays_from_user(&mut self, user: &str) {
        let today = Utc::now().date();
        self.list.retain(|s| s.user != user && s.date != today);
    }
}

#[cfg(test)]
mod test {
    use crate::{Standup, StandupList};
    use chrono::prelude::*;

    #[test]
    fn create_standup_list() {
        let sl = StandupList::new();
        assert_eq!(sl.list.len(), 0);
    }

    #[test]
    fn add_standup_to_list() {
        let user = "ruiramos";
        let mut sl = StandupList::new();
        let s = Standup::new(user);

        sl.add_standup(s);

        assert_eq!(sl.list.len(), 1);
    }

    #[test]
    fn get_todays_standup() {
        let user = "ruiramos";
        let user2 = "ruiramos2";

        let mut sl = StandupList::new();

        let s = Standup::new(user);
        let s2 = Standup::new(user);
        let mut s3 = Standup::new(user);
        let s2_1 = Standup::new(user2);
        let s2_2 = Standup::new(user2);

        s3.date = Utc.ymd(2020, 1, 15);

        sl.add_standup(s);
        sl.add_standup(s2);
        sl.add_standup(s3);
        sl.add_standup(s2_1);
        sl.add_standup(s2_2);

        let result = sl.get_todays_mut(user).unwrap();

        assert_eq!(result.date, Utc::now().date());
    }

    #[test]
    fn get_latest_standup() {
        let user = "ruiramos";
        let user2 = "ruiramos2";

        let mut sl = StandupList::new();

        let mut s = Standup::new(user);
        let mut s2 = Standup::new(user);
        let mut s3 = Standup::new(user);
        let s2_1 = Standup::new(user2);
        let s2_2 = Standup::new(user2);

        s.date = Utc.ymd(2011, 1, 1);
        // this is actually the latest one:
        s2.date = Utc.ymd(2011, 2, 1);
        s3.date = Utc.ymd(2011, 1, 15);

        sl.add_standup(s);
        sl.add_standup(s2);
        sl.add_standup(s3);
        sl.add_standup(s2_1);
        sl.add_standup(s2_2);

        let result = sl.get_latest(user).unwrap();

        assert_eq!(result.date, Utc.ymd(2011, 2, 1));
    }
}
