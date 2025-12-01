use dotenvy::dotenv;
use progress_bot::{
    create_or_update_team_info, get_bot_token_for_team, get_user, handle, slack, SlackConfig,
    SlackConfigAction, SlackConfigResponse, SlackEvent, SlackSlashEvent,
};
use rocket::form::Form;
use rocket::http::uri::Absolute;
use rocket::response::Redirect;
use rocket::serde::json::{serde_json, Json, Value as JsonValue};
use rocket::tokio;
use rocket_sync_db_pools::{database, diesel};

#[database("postgres")]
pub struct DbConn(diesel::PgConnection);

#[rocket::get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[rocket::get("/oauth?<code>")]
async fn oauth(code: String, conn: DbConn) -> Redirect {
    conn.run(move |c| {
        let oauth_response = slack::get_token_with_code(code).unwrap();
        create_or_update_team_info(oauth_response, c);
    }).await;
    let uri = Absolute::parse("https://progress.bot/success").expect("valid URI");
    Redirect::to(uri)
}

#[rocket::get("/oauth?<error>", rank = 2)]
fn oauth_error(_error: String) -> Redirect {
    let uri = Absolute::parse("https://progress.bot/error").expect("valid URI");
    Redirect::to(uri)
}

#[rocket::post("/show-config", data = "<content>")]
async fn command_show_config(content: Form<SlackSlashEvent>, conn: DbConn) -> String {
    let content = content.into_inner();
    conn.run(move |c| {
        let user = get_user(&content.user_id, c);
        let token = get_bot_token_for_team(&content.team_id, c);
        slack::send_config_dialog(content, user, token).unwrap();
    }).await;
    "".to_string()
}

#[rocket::post("/config", data = "<config>")]
async fn post_config(config: Form<SlackConfigResponse>, conn: DbConn) -> JsonValue {
    println!("{:?}", config);
    let config: SlackConfig = serde_json::from_str(&config.payload).unwrap();
    if config.r#type == "dialog_submission" {
        let copy = handle::config(&config, &mut *conn);
        let token = get_bot_token_for_team(&config.team.id, &mut *conn);

        tokio::spawn(async move {
            slack::send_response(&copy, &config.response_url, token).unwrap();
        });

        serde_json::json!({})
    } else if config.r#type == "block_actions" {
        // clicking on the standup intro message setting things as done
        let token = get_bot_token_for_team(&config.team.id, &mut *conn);
        let actions: Vec<SlackConfigAction> = config.actions.unwrap();
        let action = &actions[0];
        let split: Vec<&str> = action.value.split('-').collect();
        let (task_id, standup_id): (i32, i32) =
            (split[0].parse().unwrap(), split[1].parse().unwrap());

        if action.action_id == "set-task-done" {
            handle::set_task_done(task_id, standup_id, &mut *conn);
        } else if action.action_id == "set-task-not-done" {
            handle::set_task_not_done(task_id, standup_id, &mut *conn);
        }

        // edit message with new copy
        let original = config.message.expect("Error unwraping message from config");
        let user = get_user(&config.user.id, &mut *conn).expect("Error unwrapping user");
        let new_blocks = handle::get_standup_intro_copy(&user, &mut *conn);
        slack::update_intro_message(&original.ts, &config.channel.id, new_blocks, &token)
            .expect("Failed to update standup intro copy");
        serde_json::json!({})
    } else {
        serde_json::json!({})
    }
}

#[rocket::post("/remove", data = "<content>")]
async fn command_remove_todays(content: Form<SlackSlashEvent>, conn: DbConn) -> JsonValue {
    let data = content.into_inner();
    let user_id = data.user_id;
    let team_id = data.team_id;
    serde_json::json!({ "text": handle::remove_todays(&user_id, &team_id, &mut *conn) })
}

#[rocket::post("/help", data = "<_content>")]
fn command_help(_content: Form<SlackSlashEvent>) -> JsonValue {
    serde_json::json!({ "text": "Hi, I'm the @progress bot and I'm here to help you with your daily standups and task management!
\n:one: *Standups*
You can mention me (@progress) from a channel or send me a private message at any time to start your daily standup. If you want to post your standups in a channel or set a daily reminder, run `/progress-config`. Create multiple tasks with Slack's multiline messages, by using _shift+enter_.
If you got something wrong you can either edit the messages you sent me or type `/progress-forget` which will delete your standup for the day and allow you to try again.
\n:two: *Tasks*
Check what you have in store for the day, after completing your standup, by typing `/td` (`/progress-today`). From here, you can mark tasks as completed with `/d task_id` (`/progress-done`) or undo them with `/ud` (`/progress-undo`).
\n:three: *Web UI*
All your daily standups become available in https://web.progress.bot as well so you can track your progress. 
\nEnjoy! :pray:" })
}

#[rocket::post("/today", data = "<content>")]
async fn command_today(content: Form<SlackSlashEvent>, conn: DbConn) -> JsonValue {
    let data = content.into_inner();
    let (text, tasks) = handle::get_todays_tasks(&data.user_id, &data.team_id, &mut *conn);
    if tasks.is_none() {
        serde_json::json!({ "text": text })
    } else {
        serde_json::json!({
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

#[rocket::post("/done", data = "<content>")]
async fn command_done(content: Form<SlackSlashEvent>, conn: DbConn) -> JsonValue {
    let data = content.into_inner();
    let text = data.text;

    if text.is_none() {
        serde_json::json!({ "text": ":warning: You need to specify the task number to set as done. Run `/progress-today` to get the list of tasks." })
    } else {
        let content = text.unwrap();
        match content.parse::<i32>() {
            Ok(task) => {
                let copy = handle::set_todays_task_done(task, &data.user_id, &mut *conn);
                serde_json::json!({ "text": copy })
            }
            _ => serde_json::json!({ "text": ":warning: Please include the task number to set as done. Run `/progress-today` to get the list of tasks." }),
        }
    }
}

#[rocket::post("/undo", data = "<content>")]
async fn command_undo(content: Form<SlackSlashEvent>, conn: DbConn) -> JsonValue {
    let data = content.into_inner();
    let text = data.text;

    if text.is_none() {
        serde_json::json!({ "text": ":warning: You need to specify the task number to mark as not done. Run `/progress-today` to get the list of tasks." })
    } else {
        let content = text.unwrap();
        match content.parse::<i32>() {
            Ok(task) => {
                let copy = handle::set_todays_task_not_done(task, &data.user_id, &mut *conn);
                serde_json::json!({ "text": copy })
            }
            _ => serde_json::json!({ "text": ":warning: Please include the task number to set as not done. Run `/progress-today` to get the list of tasks." }),
        }
    }
}

#[rocket::post("/add", data = "<content>")]
async fn command_add(content: Form<SlackSlashEvent>, conn: DbConn) -> JsonValue {
    let data = content.into_inner();
    let text = data.text;

    if text.is_none() {
        serde_json::json!({ "text": ":warning: You have to include the task to add." })
    } else {
        let content = text.unwrap();
        let result = handle::add_task_to_today(&content, &data.user_id, &mut *conn);
        if let Err(msg) = result {
            serde_json::json!({ "text": msg })
        } else {
            serde_json::json!({ "text": ":white_check_mark: Task added." })
        }
    }
}

#[rocket::post("/", data = "<event>")]
async fn post_event(event: Json<SlackEvent>, conn: DbConn) -> String {
    let data = event.into_inner();

    if let Some(c) = data.challenge {
        handle::challenge(c)
    } else if let Some(e) = data.event {
        // filtering out my own messages this way, we should be more specific but
        // I cant find a way to know my own bot id. This guarantees we only reply to users
        if e.bot_id.is_none() {
            let token = get_bot_token_for_team(data.team_id.as_ref().unwrap(), &mut *conn);
            if let Some((resp, user)) = handle::event(e, data.team_id.as_ref().unwrap(), &mut *conn) {
                slack::send_message(resp, user, token).unwrap();
            }
        }
        "".to_string()
    } else {
        "no idea".to_string()
    }
}

#[rocket::launch]
fn rocket() -> _ {
    dotenv().ok();
    
    let figment = rocket::Config::figment()
        .merge(("port", std::env::var("PORT").unwrap_or_else(|_| "8800".to_string()).parse::<u16>().unwrap()))
        .merge(("databases.postgres.url", std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://diesel:password@localhost:5433/diesel".to_string())));

    rocket::custom(figment)
        .attach(DbConn::fairing())
        .mount(
            "/",
            rocket::routes![
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
}
