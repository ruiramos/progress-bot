extern crate base64;
extern crate reqwest;

use crate::{SlackOauthResponse, SlackSlashEvent, Standup, User};
use chrono::Timelike;
use reqwest::header::AUTHORIZATION;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::value::Value;

const SLACK_HOST: &str = "https://slack.com";
const POST_MESSAGE: &str = "/api/chat.postMessage";
const UPDATE_MESSAGE: &str = "/api/chat.update";
const DELETE_MESSAGE: &str = "/api/chat.delete";
const POST_DIALOG: &str = "/api/dialog.open";
const USER_DETAILS: &str = "/api/users.info";
const OAUTH_ACCESS: &str = "/api/oauth.access";

pub fn send_message(
    message: Value,
    channel: String,
    token: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let payload = json!({
        "text": message.get("text"),
        "attachments": message.get("attachments"),
        "blocks": message.get("blocks"),
        "channel": channel,
        "as_user": true
    });

    let client = reqwest::Client::new();
    let res = client
        .post(&format!("{}{}", SLACK_HOST, POST_MESSAGE))
        .json(&payload)
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()?
        .error_for_status();

    if res.is_err() {
        println!("Error sending slack message: {:?}", res);
    }

    println!("{:?}", res.unwrap().text()?);

    Ok(())
}

pub fn send_standup_to_channel(
    channel: &str,
    message: &str,
    ts: i64,
    standup: &Standup,
    completed_last: String,
    user: &User,
    token: String,
) -> Result<SlackMessageAck, Box<dyn std::error::Error>> {
    // @TODO
    let prev_day_str = String::from("Yesterday");

    let payload = json!({
        "channel": channel,
        "attachments": [{
            "pretext": message,
            "author_name": user.real_name,
            "author_icon": user.avatar_url,
            "footer": "@progress",
            "ts": ts,
            "fields": [completed_last, {
                "title": format!("{}:", prev_day_str),
                "value": format!("{}\n{}", completed_last, standup.prev_day.as_ref().unwrap()),
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
    let res = client
        .post(&format!("{}{}", SLACK_HOST, POST_MESSAGE))
        .json(&payload)
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()?
        .text()?;

    let message_ack: SlackMessageAck = serde_json::from_str(&res)?;

    Ok(message_ack)
}

pub fn update_standup_in_channel(
    standup: &Standup,
    user: &User,
    ts: i64,
    completed_last: String,
    token: String,
) -> Result<SlackMessageAck, Box<dyn std::error::Error>> {
    let original_ts = standup.message_ts.as_ref().unwrap();
    let message = ":newspaper: Here's the latest:";
    let prev_day_str = String::from("Yesterday");

    let payload = json!({
        "token": token,
        "channel": standup.channel.as_ref().unwrap(),
        "ts": original_ts,
        "as_user": true,
        "attachments": [{
            "pretext": message,
            "author_name": user.real_name,
            "author_icon": user.avatar_url,
            "footer": "@progress",
            "ts": ts,
            "fields": [{
                "title": format!("{}:", prev_day_str),
                "value": format!("{}\n{}", completed_last, standup.prev_day.as_ref().unwrap()),
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
        }],
    });

    let client = reqwest::Client::new();

    let res = client
        .post(&format!("{}{}", SLACK_HOST, UPDATE_MESSAGE))
        .json(&payload)
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()?
        .text()?;

    let message_ack: SlackMessageAck = serde_json::from_str(&res)?;

    Ok(message_ack)
}

pub fn update_intro_message(
    ts: &str,
    user: &str,
    new_blocks: Value,
    token: &str,
) -> Result<SlackMessageAck, Box<dyn std::error::Error>> {
    let payload = json!({
        "token": token,
        "channel": user,
        "ts": ts,
        "as_user": true,
        "blocks": new_blocks
    });

    let client = reqwest::Client::new();

    let res = client
        .post(&format!("{}{}", SLACK_HOST, UPDATE_MESSAGE))
        .json(&payload)
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()?
        .text()?;

    let message_ack: SlackMessageAck = serde_json::from_str(&res)?;

    Ok(message_ack)
}

// @TODO
fn generate_standup_message(standup: &Standup) -> Value {
    json!({})
}

pub fn delete_message(
    ts: &str,
    channel: &str,
    token: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let payload = json!({
        "ts": ts,
        "channel": channel,
    });

    client
        .post(&format!("{}{}", SLACK_HOST, DELETE_MESSAGE))
        .json(&payload)
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()?
        .text()?;

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

#[derive(Serialize, Deserialize, Debug)]
pub struct SlackMessageAck {
    pub ok: bool,
    pub channel: String,
    pub ts: String,
}

#[derive(Serialize, Deserialize)]
pub struct UserProfile {
    pub image_48: String,
    pub real_name: String,
}

pub fn get_user_details(
    username: &str,
    token: String,
) -> Result<UserProfile, Box<dyn std::error::Error>> {
    let body = reqwest::get(&format!(
        "{}{}?user={}&token={}",
        SLACK_HOST, USER_DETAILS, username, token
    ))?
    .text()?;

    let user: SlackResponse = serde_json::from_str(&body)?;

    Ok(user.user.profile)
}

pub fn send_config_dialog(
    event: SlackSlashEvent,
    user: Option<User>,
    token: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let (channel, reminder) = match user {
        Some(u) => {
            let channel = u.channel.unwrap_or(String::from(""));
            let reminder = match u.reminder {
                // @TODO timezones
                Some(date) => (date.hour() + 1).to_string(),
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
                        "label": "07:00",
                        "value": "7"
                    },{
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
                        "label": "13:00",
                        "value": "13"
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

pub fn send_response(
    copy: &str,
    response_url: &str,
    token: String,
) -> Result<(), Box<dyn std::error::Error>> {
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

pub fn get_token_with_code(code: String) -> Result<SlackOauthResponse, Box<dyn std::error::Error>> {
    let client_id = std::env::var("CLIENT_ID").expect("CLIENT_ID missing");
    let client_secret = std::env::var("CLIENT_SECRET").expect("CLIENT_SECRET missing");

    let payload = [("code", code)];

    let client = reqwest::Client::new();
    let body = client
        .post(&format!("{}{}", SLACK_HOST, OAUTH_ACCESS))
        .basic_auth(client_id, Some(client_secret))
        .form(&payload)
        .send()?
        .text()?;

    let res: SlackOauthResponse = serde_json::from_str(&body).unwrap();

    Ok(res)
}
