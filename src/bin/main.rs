#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_contrib;

use progress_bot::{
    create_or_update_team_info, get_bot_token_for_team, get_user, handle, slack, SlackConfig,
    SlackConfigResponse, SlackEvent, SlackSlashEvent,
};
use rocket::config::{Config, Environment, Value};
use rocket::request::Form;
use rocket::request::LenientForm;
use rocket_contrib::databases::diesel;
use rocket_contrib::json::Json;
use rocket_contrib::json::JsonValue;
use std::collections::HashMap;
use std::thread;

#[database("postgres")]
pub struct DbConn(diesel::PgConnection);

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[get("/oauth?<code>")]
fn oauth(code: String, conn: DbConn) -> String {
    let oauth_response = slack::get_token_with_code(code).unwrap();
    create_or_update_team_info(oauth_response, &*conn);
    "that worked".to_string()
}

#[post("/show-config", data = "<content>")]
fn post_show_config(content: LenientForm<SlackSlashEvent>, conn: DbConn) -> String {
    let content = content.into_inner();
    let user = get_user(&content.user_id, &*conn);
    let token = get_bot_token_for_team(&content.team_id, &*conn);
    slack::send_config_dialog(content, user, token).unwrap();
    "".to_string()
}

#[post("/config", data = "<config>")]
fn post_config(config: Form<SlackConfigResponse>, conn: DbConn) -> String {
    let config: SlackConfig = serde_json::from_str(&config.payload).unwrap();
    let copy = handle::config(&config, &*conn);
    let token = get_bot_token_for_team(&config.team.id, &*conn);

    thread::spawn(move || {
        slack::send_response(&copy, &config.response_url, token).unwrap();
    });

    "".to_string()
}

#[post("/remove", data = "<content>")]
fn post_remove_todays(content: LenientForm<SlackSlashEvent>, conn: DbConn) -> JsonValue {
    let user_id = content.into_inner().user_id;
    json!({ "text": handle::remove_todays(&user_id, &*conn) })
}

#[post("/help", data = "<_content>")]
fn post_help(_content: LenientForm<SlackSlashEvent>) -> JsonValue {
    json!({ "text": "Hi, I'm the @progress bot and I'm here to help you with your daily standups! :wave:
You can mention me or chat at any time to start telling me about your day. If you want me to post your standups in a channel or to set a daily reminder, run `/progress-config`.
If you get something wrong just run `/progress-forget` and try again. All your daily standups become available in https://web.progressbot.net as well so you can track your progress. Enjoy! :pray:" })
}

#[post("/", data = "<event>")]
fn post_event(event: Json<SlackEvent>, conn: DbConn) -> String {
    let data = event.into_inner();

    if let Some(c) = data.challenge {
        handle::challenge(c)
    } else if let Some(e) = data.event {
        // filtering out my own messages this way, we should be more specific but
        // I cant find a way to know my own bot id. This guarantees we only reply to users
        if e.bot_id.is_none() {
            let (resp, user) = handle::event(e, &data.team_id, &*conn);
            let token = get_bot_token_for_team(&data.team_id, &*conn);
            slack::send_message(resp, user, token).unwrap();
        }
        "".to_string()
    } else {
        "no idea".to_string()
    }
}

fn main() {
    let mut database_config = HashMap::new();
    let mut databases = HashMap::new();

    let env = std::env::var("ROCKET_ENV");

    let config = match env {
        Ok(ref s) if s == "production" => {
            let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL missing");
            let port = std::env::var("PORT").expect("PORT missing");

            database_config.insert("url", Value::from(db_url));
            databases.insert("postgres", Value::from(database_config));

            Config::build(Environment::Production)
                .port(port.parse().unwrap())
                .extra("databases", databases)
                .finalize()
                .unwrap()
        }
        _ => {
            database_config.insert(
                "url",
                Value::from("postgres://diesel:password@localhost:5433/diesel"),
            );
            databases.insert("postgres", Value::from(database_config));

            Config::build(Environment::Development)
                .extra("databases", databases)
                .finalize()
                .unwrap()
        }
    };

    rocket::custom(config)
        .attach(DbConn::fairing())
        .mount(
            "/",
            routes![
                index,
                post_show_config,
                post_config,
                post_event,
                post_remove_todays,
                post_help,
                oauth
            ],
        )
        .launch();
}
