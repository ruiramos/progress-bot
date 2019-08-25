extern crate reqwest;
use crate::{SlackSlashEvent, Standup, User};
use reqwest::header::AUTHORIZATION;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;

const TOKEN: &str = env!("SLACK_TOKEN"); //.expect("Error: SLACK_TOKEN is a required env variable.");
const POST_MESSAGE_URL: &str = "https://slack.com/api/chat.postMessage";
const POST_DIALOG_URL: &str = "https://slack.com/api/dialog.open";
const USER_DETAILS_URL: &str = "https://slack.com/api/users.info";

pub fn send_message(message: String, channel: String) -> Result<(), Box<dyn std::error::Error>> {
    let payload = json!({
        "text": message,
        "channel": channel,
        "as_user": true
    });

    let client = reqwest::Client::new();
    client
        .post(POST_MESSAGE_URL)
        .json(&payload)
        .header(AUTHORIZATION, format!("Bearer {}", TOKEN))
        .send()?;

    Ok(())
}

pub fn send_standup_to_channel(
    channel: &str,
    message: &str,
    standup: &Standup,
    user: &User,
) -> Result<(), Box<dyn std::error::Error>> {
    let payload = json!({
        "channel": channel,
        "attachments": [{
            "pretext": message,
            "author_name": user.real_name.as_ref().unwrap_or(&user.username),
            "author_icon": user.avatar_url.as_ref().unwrap_or(&"".to_string()),
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
        .post(POST_MESSAGE_URL)
        .json(&payload)
        .header(AUTHORIZATION, format!("Bearer {}", TOKEN))
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
    let body = reqwest::get(&format!(
        "{}?user={}&token={}",
        USER_DETAILS_URL, username, TOKEN
    ))?
    .text()?;

    let user: SlackResponse = serde_json::from_str(&body).unwrap();
    Ok(user.user.profile)
}

pub fn send_config_dialog(event: SlackSlashEvent) -> Result<(), Box<dyn std::error::Error>> {
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
                    "data_source": "conversations"
                },
                {
                    "type": "select",
                    "optional": "true",
                    "label": "Reminder",
                    "name": "reminder",
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
    let mut resp = client
        .post(POST_DIALOG_URL)
        .json(&payload)
        .header(AUTHORIZATION, format!("Bearer {}", TOKEN))
        .send()?;

    Ok(())
}
