#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_contrib;

use progress_bot::{
    create_or_update_team_info, get_bot_token_for_team, get_user, handle, slack, SlackConfig,
    SlackConfigResponse, SlackEvent, SlackSlashEvent,
};
use rocket::request::Form;
use rocket::request::LenientForm;
use rocket_contrib::databases::diesel;
use rocket_contrib::json::Json;
use rocket_contrib::json::JsonValue;
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
    "".to_string()
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
    rocket::ignite()
        .attach(DbConn::fairing())
        .mount(
            "/",
            routes![
                index,
                post_show_config,
                post_config,
                post_event,
                post_remove_todays,
                oauth
            ],
        )
        .launch();
}
