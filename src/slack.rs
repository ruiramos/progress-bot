extern crate reqwest;
use crate::{SlackSlashEvent, Standup, User};
use reqwest::header::AUTHORIZATION;
use serde::{Deserialize, Serialize};
use serde_json::json;

const SLACK_HOST: &str = "https://slack.com";
const POST_MESSAGE: &str = "/api/chat.postMessage";
const POST_DIALOG: &str = "/api/dialog.open";
const USER_DETAILS: &str = "/api/users.info";

pub fn send_message(message: String, channel: String) -> Result<(), Box<dyn std::error::Error>> {
    let token = std::env::var("SLACK_TOKEN").unwrap();
    let payload = json!({
        "text": message,
        "channel": channel,
        "as_user": true
    });

    let client = reqwest::Client::new();
    client
        .post(&format!("{}{}", SLACK_HOST, POST_MESSAGE))
        .json(&payload)
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()?;

    Ok(())
}

pub fn send_standup_to_channel(
    channel: &str,
    message: &str,
    ts: i64,
    standup: &Standup,
    user: &User,
) -> Result<(), Box<dyn std::error::Error>> {
    let token = std::env::var("SLACK_TOKEN").unwrap();
    let payload = json!({
        "channel": channel,
        "attachments": [{
            "pretext": message,
            "author_name": user.real_name,
            "author_icon": user.avatar_url,
            "footer": "@progress",
            "ts": ts,
            "fields": [{
                "title": "Yesterday:",
                "value": standup.prev_day.as_ref().unwrap(),
                "short": false
            }, {
                "title": "Today:",
                "value": standup.day.as_ref().unwrap(),
                "short": false
            }, {
                "title": "Blockers:",
                "value": standup.blocker.as_ref().unwrap(),
                "short": false
            }]
        }]
    });

    let client = reqwest::Client::new();
    client
        .post(&format!("{}{}", SLACK_HOST, POST_MESSAGE))
        .json(&payload)
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()?;

    Ok(())
}

#[derive(Serialize, Deserialize)]
pub struct SlackResponse {
    user: SlackUserInfo,
}

#[derive(Serialize, Deserialize)]
pub struct SlackUserInfo {
    profile: UserProfile,
}

#[derive(Serialize, Deserialize)]
pub struct UserProfile {
    pub image_48: String,
    pub real_name: String,
}

pub fn get_user_details(username: &str) -> Result<UserProfile, Box<dyn std::error::Error>> {
    let token = std::env::var("SLACK_TOKEN")?;
    let body = reqwest::get(&format!(
        "{}{}?user={}&token={}",
        SLACK_HOST, USER_DETAILS, username, token
    ))?
    .text()?;

    let user: SlackResponse = serde_json::from_str(&body).unwrap();
    Ok(user.user.profile)
}

pub fn send_config_dialog(
    event: SlackSlashEvent,
    user: Option<User>,
) -> Result<(), Box<dyn std::error::Error>> {
    let token = std::env::var("SLACK_TOKEN").unwrap();

    let (channel, reminder) = match user {
        Some(u) => {
            let channel = u.channel.unwrap_or(String::from(""));
            let reminder = match u.reminder {
                Some(date) => date.to_string(),
                None => String::from(""),
            };

            (channel, reminder)
        }
        None => (String::from(""), String::from("")),
    };

    let payload = json!({
        "trigger_id": event.trigger_id,
        "dialog": {
            "callback_id": "@TODO???",
            "title": "Configure @progress",
            "submit_label": "Save",
            "notify_on_cancel": false,
            "elements": [
                {
                    "type": "select",
                    "optional": "true",
                    "label": "Channel to notify",
                    "name": "channel",
                    "data_source": "conversations",
                    "value": channel
                },
                {
                    "type": "select",
                    "optional": "true",
                    "label": "Reminder",
                    "name": "reminder",
                    "value": reminder,
                    "options": [{
                        "label": "08:00",
                        "value": "8"
                    },{
                        "label": "09:00",
                        "value": "9"
                    },{
                        "label": "10:00",
                        "value": "10"
                    },{
                        "label": "11:00",
                        "value": "11"
                    },{
                        "label": "12:00",
                        "value": "12"
                    },{
                        "label": "12:34",
                        "value": "1234"
                    }]
                }
            ]
        }
    });

    let client = reqwest::Client::new();
    client
        .post(&format!("{}{}", SLACK_HOST, POST_DIALOG))
        .json(&payload)
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()?;

    Ok(())
}

pub fn send_response(copy: &str, response_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let token = std::env::var("SLACK_TOKEN").unwrap();
    let payload = json!({
        "text": copy.to_string(),
        "response_type": "ephemeral"
    });

    let client = reqwest::Client::new();
    client
        .post(response_url)
        .json(&payload)
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()?;

    Ok(())
}
