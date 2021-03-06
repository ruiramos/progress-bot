#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_contrib;

use dotenv::dotenv;
use progress_bot::{
    create_or_update_team_info, get_bot_token_for_team, get_user, handle, slack, SlackConfig,
    SlackConfigAction, SlackConfigResponse, SlackEvent, SlackSlashEvent,
};
use rocket::config::{Config, Environment, Value};
use rocket::http::uri::Absolute;
use rocket::request::Form;
use rocket::request::LenientForm;
use rocket::response::Redirect;
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
fn oauth(code: String, conn: DbConn) -> Redirect {
    let oauth_response = slack::get_token_with_code(code).unwrap();
    create_or_update_team_info(oauth_response, &*conn);
    let uri = Absolute::parse("https://progress.bot/success").expect("valid URI");
    Redirect::to(uri)
}

#[get("/oauth?<error>", rank = 2)]
fn oauth_error(error: String) -> Redirect {
    let uri = Absolute::parse("https://progress.bot/error").expect("valid URI");
    Redirect::to(uri)
}

#[post("/show-config", data = "<content>")]
fn command_show_config(content: LenientForm<SlackSlashEvent>, conn: DbConn) -> String {
    let content = content.into_inner();
    let user = get_user(&content.user_id, &*conn);
    let token = get_bot_token_for_team(&content.team_id, &*conn);
    slack::send_config_dialog(content, user, token).unwrap();
    "".to_string()
}

#[post("/config", data = "<config>")]
fn post_config(config: Form<SlackConfigResponse>, conn: DbConn) -> JsonValue {
    println!("{:?}", config);
    let config: SlackConfig = serde_json::from_str(&config.payload).unwrap();
    if config.r#type == "dialog_submission" {
        let copy = handle::config(&config, &*conn);
        let token = get_bot_token_for_team(&config.team.id, &*conn);

        thread::spawn(move || {
            slack::send_response(&copy, &config.response_url, token).unwrap();
        });

        json!({})
    } else if config.r#type == "block_actions" {
        // clicking on the standup intro message setting things as done
        let token = get_bot_token_for_team(&config.team.id, &*conn);
        let actions: Vec<SlackConfigAction> = config.actions.unwrap();
        let action = &actions[0];
        let split: Vec<&str> = action.value.split('-').collect();
        let (task_id, standup_id): (i32, i32) =
            (split[0].parse().unwrap(), split[1].parse().unwrap());

        if action.action_id == "set-task-done" {
            handle::set_task_done(task_id, standup_id, &conn);
        } else if action.action_id == "set-task-not-done" {
            handle::set_task_not_done(task_id, standup_id, &conn);
        }

        // edit message with new copy
        let original = config.message.expect("Error unwraping message from config");
        let user = get_user(&config.user.id, &conn).expect("Error unwrapping user");
        let new_blocks = handle::get_standup_intro_copy(&user, &conn);
        slack::update_intro_message(&original.ts, &config.channel.id, new_blocks.0, &token)
            .expect("Failed to update standup intro copy");
        json!({})
    } else {
        json!({})
    }
    /* this was a test with the /today command, not really intentded functionality

     else if config.r#type == "interactive_message" && config.callback_id.is_some()
    /*== "set_task_status"*/
    {
        let actions: Vec<SlackConfigAction> = config.actions.unwrap();
        let action = &actions[0];
        if action.action_id == "do-task" {
            handle::set_task_done(
                action.value.parse().unwrap(),
                &config.user.id,
                &config.team.id,
                &conn,
            );
        } else {
            handle::set_task_not_done(
                action.value.parse().unwrap(),
                &config.user.id,
                &config.team.id,
                &conn,
            );
        }
        let (text, tasks) = handle::get_todays_tasks(&config.user.id, &config.team.id, &conn);
        json!({
            "text": text,
            "attachments": tasks.unwrap().iter().enumerate().map(|(i, task)| {
                let button_name = if task.done { "undo-task" } else { "do-task" };
                let button_text = if task.done { "Mark as not done" } else { "Mark as done" };
                let button_style = if task.done { "" } else { "primary" };
                json!({
                    "text": task.to_string(),
                    "callback_id": "set_task_status",
                    "attachment_type": "default",
                    "actions": [{
                        "name": button_name,
                        "text": button_text,
                        "type": "button",
                        "value": i+1,
                        "style": button_style
                    }]
                })
            }).collect::<Vec<JsonValue>>()
        })
        */
}

#[post("/remove", data = "<content>")]
fn command_remove_todays(content: LenientForm<SlackSlashEvent>, conn: DbConn) -> JsonValue {
    let data = content.into_inner();
    let user_id = data.user_id;
    let team_id = data.team_id;
    json!({ "text": handle::remove_todays(&user_id, &team_id, &*conn) })
}

#[post("/help", data = "<_content>")]
fn command_help(_content: LenientForm<SlackSlashEvent>) -> JsonValue {
    json!({ "text": "Hi, I'm the @progress bot and I'm here to help you with your daily standups and task management!
\n:one: *Standups*
You can mention me (@progress) from a channel or send me a private message at any time to start your daily standup. If you want to post your standups in a channel or set a daily reminder, run `/progress-config`. Create multiple tasks with Slack's multiline messages, by using _shift+enter_.
If you got something wrong you can either edit the messages you sent me or type `/progress-forget` which will delete your standup for the day and allow you to try again.
\n:two: *Tasks*
Check what you have in store for the day, after completing your standup, by typing `/td` (`/progress-today`). From here, you can mark tasks as completed with `/d task_id` (`/progress-done`) or undo them with `/ud` (`/progress-undo`).
\n:three: *Web UI*
All your daily standups become available in https://web.progress.bot as well so you can track your progress. 
\nEnjoy! :pray:" })
}

#[post("/today", data = "<content>")]
fn command_today(content: LenientForm<SlackSlashEvent>, conn: DbConn) -> JsonValue {
    let data = content.into_inner();
    let (text, tasks) = handle::get_todays_tasks(&data.user_id, &data.team_id, &conn);
    if tasks.is_none() {
        json!({ "text": text })
    } else {
        json!({
            "text":
                format!(
                    "{}\n{}\n\n{}",
                    text,
                    handle::print_tasks(tasks.expect("Error unwraping tasks")),
                    "Mark tasks as done with `/d task_number`, undo with `/u task_number`."
                )
        })
    }
}

#[post("/done", data = "<content>")]
fn command_done(content: LenientForm<SlackSlashEvent>, conn: DbConn) -> JsonValue {
    let data = content.into_inner();
    let text = data.text;

    if text.is_none() {
        json!({ "text": ":warning: You need to specify the task number to set as done. Run `/progress-today` to get the list of tasks." })
    } else {
        let content = text.unwrap();
        match content.parse::<i32>() {
            Ok(task) => {
                let copy = handle::set_todays_task_done(task, &data.user_id, &conn);
                json!({ "text": copy })
            }
            _ => json!({ "text": ":warning: Please include the task number to set as done. Run `/progress-today` to get the list of tasks." }),
        }
    }
}

#[post("/undo", data = "<content>")]
fn command_undo(content: LenientForm<SlackSlashEvent>, conn: DbConn) -> JsonValue {
    let data = content.into_inner();
    let text = data.text;

    if text.is_none() {
        json!({ "text": ":warning: You need to specify the task number to mark as not done. Run `/progress-today` to get the list of tasks." })
    } else {
        let content = text.unwrap();
        match content.parse::<i32>() {
            Ok(task) => {
                let copy = handle::set_todays_task_not_done(task, &data.user_id, &conn);
                json!({ "text": copy })
            }
            _ => json!({ "text": ":warning: Please include the task number to set as not done. Run `/progress-today` to get the list of tasks." }),
        }
    }
}

#[post("/add", data = "<content>")]
fn command_add(content: LenientForm<SlackSlashEvent>, conn: DbConn) -> JsonValue {
    let data = content.into_inner();
    let text = data.text;

    if text.is_none() {
        json!({ "text": ":warning: You have to include the task to add." })
    } else {
        let content = text.unwrap();
        let result = handle::add_task_to_today(&content, &data.user_id, &conn);
        if let Err(msg) = result {
            json!({ "text": msg })
        } else {
            json!({ "text": ":white_check_mark: Task added." })
        }
    }
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
            let token = get_bot_token_for_team(data.team_id.as_ref().unwrap(), &*conn);
            if let Some((resp, user)) = handle::event(e, data.team_id.as_ref().unwrap(), &*conn) {
                // the .0 converts the Rocket JsonValue into the underlying serde_json
                slack::send_message(resp.0, user, token).unwrap();
            }
        }
        "".to_string()
    } else {
        "no idea".to_string()
    }
}

fn main() {
    dotenv().ok();
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
                .port(8800)
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
                command_show_config,
                post_config,
                post_event,
                command_remove_todays,
                command_help,
                command_today,
                command_done,
                command_undo,
                command_add,
                oauth,
                oauth_error
            ],
        )
        .launch();
}
