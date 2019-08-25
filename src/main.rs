#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_contrib;

use progress_bot::{
    handle, slack, SlackConfig, SlackConfigResponse, SlackEvent, SlackSlashEvent, StandupList,
    UserList,
};
use rocket::request::Form;
use rocket::request::LenientForm;
use rocket::State;
use rocket_contrib::databases::diesel;
use rocket_contrib::json::Json;
use std::sync::{Arc, Mutex};

#[database("postgres")]
struct DbConn(diesel::PgConnection);

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[post("/show-config", data = "<content>")]
fn post_show_config(content: LenientForm<SlackSlashEvent>) -> String {
    slack::send_config_dialog(content.into_inner()).unwrap();
    "".to_string()
}

#[post("/config", data = "<content>")]
fn post_config(content: Form<SlackConfigResponse>, users: State<Arc<Mutex<UserList>>>) -> String {
    let config: SlackConfig = serde_json::from_str(&content.payload).unwrap();
    handle::config(config, users);
    "".to_string()
}

#[post("/", data = "<event>")]
fn post_event(
    standups: State<Arc<Mutex<StandupList>>>,
    users: State<Arc<Mutex<UserList>>>,
    event: Json<SlackEvent>,
) -> String {
    let data = event.into_inner();

    if let Some(c) = data.challenge {
        handle::challenge(c)
    } else if let Some(e) = data.event {
        // filtering out my own messages this way, we should be more specific but
        // I cant find a way to know my own bot id. This guarantees we only reply to users
        if e.bot_id.is_none() {
            let (resp, user) = handle::event(e, standups, users);
            slack::send_message(resp, user).unwrap();
        }
        "".to_string()
    } else {
        "no idea".to_string()
    }
}

fn main() {
    rocket::ignite()
        // .attach(DbConn::fairing())
        .manage(Arc::new(Mutex::new(StandupList::new())))
        .manage(Arc::new(Mutex::new(UserList::new())))
        .mount(
            "/",
            routes![index, post_show_config, post_config, post_event],
        )
        .launch();
}
