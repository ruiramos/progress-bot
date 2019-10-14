use crate::schema::standups;
use crate::slack;
use crate::{
    create_standup, create_user, get_bot_token_for_team, get_latest_standup_for_user,
    get_number_emoji, get_standup_before_provided, get_standup_by_id, get_todays_standup_for_user,
    get_user, remove_todays_standup_for_user, update_standup, update_user, Task,
};
use crate::{EventDetails, SlackConfig, Standup, StandupState, User};
use chrono::{Datelike, Local, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use diesel::prelude::*;
use rocket_contrib::json::JsonValue;

pub fn challenge(c: String) -> String {
    c
}

pub fn event(
    evt: EventDetails,
    team_id: &str,
    conn: &diesel::PgConnection,
) -> Option<(JsonValue, String)> {
    match evt.r#type.as_ref() {
        "message" => match evt.subtype.as_ref() {
            None => Some(react(evt, team_id, conn)),
            Some(s) if s == "message_changed" => react_message_edit(evt, conn),
            _ => None,
        },
        "app_mention" => Some(react_notification(evt, team_id, conn)),
        "app_home_opened" => react_app_home_open(evt, team_id, conn),
        _ => None,
    }
}

pub fn react(evt: EventDetails, team_id: &str, conn: &diesel::PgConnection) -> (JsonValue, String) {
    let user = match get_user(&evt.user.as_ref().unwrap(), conn) {
        Some(user) => user,
        None => create_user(&evt.user.as_ref().unwrap(), team_id, conn),
    };

    let msg = evt.text.as_ref().unwrap();
    let todays = get_todays_standup_for_user(&user.username, conn);

    let copy = match todays {
        None => {
            let blocks = get_standup_intro_copy(&user, conn);
            json!({ "blocks": blocks })
        }
        Some(mut todays) => {
            if let StandupState::Complete = todays.get_state() {
                json!({"text": "You're done for today, off to work you go now! :nerd_face:"})
            } else {
                match todays.get_state() {
                    StandupState::Blocker => {
                        todays.add_content(msg, &evt);
                        if user.channel.is_some() {
                            share_standup(&user, &todays, &conn);
                        }
                    }
                    _ => todays.add_content(msg, &evt),
                }

                update_standup(&todays, conn);

                json!({"text": todays.get_copy(&user.channel) })
            }
        }
    };

    (copy, user.username)
}

pub fn react_message_edit(
    evt: EventDetails,
    conn: &diesel::PgConnection,
) -> Option<(JsonValue, String)> {
    let previous_message = evt.previous_message.unwrap();
    let new_message = evt.message.unwrap();
    let username = previous_message.user;
    let user = get_user(&username, conn);

    if user.is_none() {
        return Some((
            json!({"text": "Very weird error, couldn't find your user, sorry."}),
            username,
        ));
    }

    let todays = get_todays_standup_for_user(&username, conn);

    match todays {
        None => Some((
            json!({"text": ":warning: Sorry but you can only really edit today's standup. (and you haven't created one yet! Ready to do that?)"}),
            username,
        )),
        Some(mut standup) => {
            if &previous_message.ts
                == standup
                    .prev_day_message_ts
                    .as_ref()
                    .unwrap_or(&String::new())
            {
                standup.prev_day = Some(new_message.text);
                standup.prev_day_message_ts = Some(new_message.ts);
            } else if &previous_message.ts
                == standup.day_message_ts.as_ref().unwrap_or(&String::new())
            {
                standup.day = Some(new_message.text);
                standup.day_message_ts = Some(new_message.ts);
            } else if &previous_message.ts
                == standup
                    .blocker_message_ts
                    .as_ref()
                    .unwrap_or(&String::new())
            {
                standup.blocker = Some(new_message.text);
                standup.blocker_message_ts = Some(new_message.ts);
            } else {
                return Some((
                    json!({"text": ":warning: Sorry but you can only really edit today's standup."}),
                    username,
                ));
            }

            if standup.channel.is_some() {
                update_standup_message_in_channel(&user.unwrap(), &mut standup, conn);
            }

            update_standup(&standup, conn);
            Some((
                json!({"text": ":white_check_mark: Standup updated, thanks!"}),
                username,
            ))
        }
    }
}

fn update_standup_message_in_channel(user: &User, standup: &mut Standup, conn: &PgConnection) {
    let prev = get_standup_before_provided(&user.username, &standup, conn);
    let completed_last = if let Some(ps) = prev {
        get_tasks_from_standup(ps)
            .iter()
            .filter(|task| task.done)
            .map(|task| format!(":white_check_mark: {}", task.content))
            .collect::<Vec<String>>()
            .join("\n")
    } else {
        String::from("")
    };
    let ack = slack::update_standup_in_channel(
        &standup,
        &user,
        Local::now().timestamp(),
        completed_last,
        get_bot_token_for_team(&user.team_id, conn),
    );

    standup.message_ts = Some(ack.unwrap().ts);
}

pub fn react_notification(
    evt: EventDetails,
    team_id: &str,
    conn: &diesel::PgConnection,
) -> (JsonValue, String) {
    let msg = evt.text;
    let user = match get_user(&evt.user.as_ref().unwrap(), conn) {
        Some(user) => user,
        None => create_user(&evt.user.as_ref().unwrap(), team_id, conn),
    };

    let todays = get_todays_standup_for_user(&user.username, conn);
    let copy = match todays {
        None => "I'm here! Ready for your standup today?".to_string(),
        Some(s) => {
            if let StandupState::Complete = s.get_state() {
                "You're done for today, off to work you go now! :nerd_face:".to_string()
            } else {
                s.get_copy(&user.channel)
                // @TODO get this user.channel param out of the above and move that here
            }
        }
    };
    (json!({ "text": copy }), user.username)
}

pub fn react_app_home_open(
    evt: EventDetails,
    team_id: &str,
    conn: &diesel::PgConnection,
) -> Option<(JsonValue, String)> {
    let user = match get_user(&evt.user.as_ref().unwrap(), conn) {
        Some(user) => user,
        None => create_user(&evt.user.as_ref().unwrap(), team_id, conn),
    };
    let latest = get_latest_standup_for_user(&user.username, conn);

    if latest.is_none() {
        Some((
            json!({"text": "Hey there and welcome to @progress! Let me know if this is a good time for your standup today.\nIf you want more information about how this works, `/progress-help` is a good place to start."}),
            user.username,
        ))
    } else {
        let todays = get_todays_standup_for_user(&user.username, conn);
        if todays.is_none() && user.reminder.is_none() {
            Some((
                json!({"text": "Hey there! Is this a good time for your standup today?"}),
                user.username,
            ))
        } else {
            None
        }
    }
}

pub fn share_standup(user: &User, standup: &Standup, conn: &diesel::PgConnection) {
    let msg = ":newspaper: Here's the latest:";
    let prev = get_standup_before_provided(&user.username, standup, conn);
    let completed_last = if let Some(ps) = prev {
        get_tasks_from_standup(ps)
            .iter()
            .filter(|task| task.done)
            .map(|task| format!(":white_check_mark: {}", task.content))
            .collect::<Vec<String>>()
            .join("\n")
    } else {
        String::from("")
    };

    let ack = slack::send_standup_to_channel(
        user.channel.as_ref().unwrap(),
        msg,
        Local::now().timestamp(),
        standup,
        completed_last,
        user,
        get_bot_token_for_team(&user.team_id, conn),
    )
    .unwrap();

    if ack.ok == true {
        diesel::update(standups::table.find(standup.id))
            .set((
                standups::message_ts.eq(ack.ts),
                standups::channel.eq(ack.channel),
            ))
            .execute(conn)
            .expect("Error updating User");
    }
}

pub fn config(config: &SlackConfig, conn: &diesel::PgConnection) -> String {
    let mut user = match get_user(&config.user.id, conn) {
        Some(user) => user,
        None => create_user(&config.user.id, &config.team.id, conn),
    };

    let submission = config.submission.as_ref().unwrap();
    let channel = &submission.channel;
    let reminder = submission.reminder.as_ref();

    user.channel = if channel.is_none() {
        None
    } else {
        Some(channel.as_ref().unwrap().clone())
    };

    if let Some(reminder) = reminder {
        let now = Utc::now();
        let d = NaiveDate::from_ymd(now.year(), now.month(), now.day());
        // @TODO timezones
        let h: u32 = reminder.parse().unwrap();
        let t = NaiveTime::from_hms_milli(h - 1, 0, 0, 0);
        let reminder_date = NaiveDateTime::new(d, t);
        user.reminder = Some(reminder_date);
    } else {
        user.reminder = None;
    }

    update_user(&mut user, conn);

    let copy = match (reminder, channel) {
        (None, None) => "Will not remind you or post your standups anywhere else!".to_string(),
        (None, Some(c)) => format!("Will post your standups in <#{}>.", c),
        (Some(r), None) => format!("Will remind you daily at {}.", r),
        (Some(r), Some(c)) => format!(
            "Will post your standups in <#{}> and remind you daily at {}.",
            c, r
        ),
    };

    copy
}

pub fn remove_todays(user_id: &str, team_id: &str, conn: &diesel::PgConnection) -> String {
    let todays = get_todays_standup_for_user(user_id, conn);

    if todays.is_none() {
        ":warning: Couldn't find your standup for today, so nothing to do here.".to_string()
    } else {
        let standup = todays.unwrap();
        if standup.channel.is_some() {
            slack::delete_message(
                standup.message_ts.as_ref().unwrap(),
                standup.channel.as_ref().unwrap(),
                &get_bot_token_for_team(team_id, conn),
            )
            .unwrap();
        }

        remove_todays_standup_for_user(user_id, conn);
        ":shrug: Just forgot all about today's standup, feel free to try again.".to_string()
    }
}

pub fn get_todays_tasks(
    user_id: &str,
    team_id: &str,
    conn: &diesel::PgConnection,
) -> (String, Option<Vec<Task>>) {
    let user = match get_user(&user_id, conn) {
        Some(user) => user,
        None => create_user(&user_id, team_id, conn),
    };
    let todays = get_todays_standup_for_user(user_id, conn);
    if let Some(standup) = todays {
        if standup.day.is_some() {
            let tasks = get_tasks_from_standup(standup);
            let num_tasks = tasks.len();
            let done = tasks
                .iter()
                .fold(0, |sum, task| sum + (if task.done { 1 } else { 0 }));

            let task_word = if num_tasks == 1 { "task" } else { "tasks" };

            let header = if num_tasks == done {
                format!(
                    "Hey {}, you've completed all your tasks for *today*, well done! :tada:",
                    user.real_name,
                )
            } else if done > 0 {
                format!(
                    "Hey {}, you've completed {}/{} tasks you have in store for *today*:",
                    user.real_name, done, num_tasks
                )
            } else {
                format!(
                    "Hey {}, you have {} {} in store for *today*:",
                    user.real_name, num_tasks, task_word
                )
            };

            (header, Some(tasks))
        } else {
            (format!("You still haven't told me what you'll be doing today! Please finish your standup first. \n {}", standup.get_copy(&user.channel)), None)
        }
    } else {
        ("Couldn't find todays standup, sorry. Mention @progress or send me a message to start the standup flow.".to_string(), None)
    }
}

fn get_tasks_from_standup(standup: Standup) -> Vec<Task> {
    let done = standup.done.unwrap_or(Vec::new());
    let id = standup.id;

    let tasks_vec = standup
        .day
        .unwrap()
        .split('\n')
        .enumerate()
        .map(|(i, x)| {
            if done.contains(&((i + 1) as i32)) {
                Task {
                    content: x.trim().to_string(),
                    done: true,
                    prefix: get_number_emoji(i + 1),
                    standup_id: id,
                }
            } else {
                Task {
                    content: x.trim().to_string(),
                    done: false,
                    prefix: get_number_emoji(i + 1),
                    standup_id: id,
                }
            }
        })
        .collect::<Vec<Task>>();

    tasks_vec
}

pub fn print_tasks(tasks: Vec<Task>) -> String {
    tasks
        .iter()
        .map(|task| format!("> {}", task))
        .collect::<Vec<String>>()
        .join("\n")
}

pub fn set_task_done(task: i32, standup_id: i32, conn: &diesel::PgConnection) -> String {
    let mut standup = get_standup_by_id(standup_id, conn);
    let mut done = standup.done.unwrap_or(Vec::new());

    // TODO validate if the task makes sense by getting the len() of split('\n') of the day

    if !done.contains(&task) {
        done.push(task);
        standup.done = Some(done);
        update_standup(&standup, conn);
        format!(
            "Got it, marked task {} as *done*. Here's today: \n{}",
            task,
            print_tasks(get_tasks_from_standup(standup))
        )
    } else {
        format!("Task {} was already done!", task)
    }
}

pub fn set_todays_task_done(task: i32, user_id: &str, conn: &diesel::PgConnection) -> String {
    let todays = get_todays_standup_for_user(user_id, conn);

    if todays.is_none() {
        return "Couldn't find todays standup, sorry. Mention @progress or send me a message to start the standup flow.".to_string();
    }

    set_task_done(task, todays.unwrap().id, conn)
}

pub fn set_task_not_done(task: i32, standup_id: i32, conn: &diesel::PgConnection) -> String {
    let mut standup = get_standup_by_id(standup_id, conn);

    let done = standup.done.unwrap_or(Vec::new());

    if done.contains(&task) {
        standup.done = Some(done.into_iter().filter(|i| *i != task).collect());
        update_standup(&standup, conn);
        format!(
            "Got it, marked task {} as *not done*. Here's today: \n{}",
            task,
            print_tasks(get_tasks_from_standup(standup))
        )
    } else {
        format!("Task {} was not marked as done yet.", task)
    }
}

pub fn set_todays_task_not_done(task: i32, user_id: &str, conn: &diesel::PgConnection) -> String {
    let todays = get_todays_standup_for_user(user_id, conn);

    if todays.is_none() {
        return "Couldn't find todays standup, sorry. Mention @progress or send me a message to start the standup flow.".to_string();
    }

    set_task_not_done(task, todays.unwrap().id, conn)
}

pub fn add_task_to_today(
    task: &str,
    user_id: &str,
    conn: &diesel::PgConnection,
) -> Result<String, String> {
    let todays = get_todays_standup_for_user(user_id, conn);
    let user = get_user(&user_id, conn);

    if let Some(mut standup) = todays {
        let new_content = format!("{}\n{}", standup.day.unwrap_or(String::new()), task);
        standup.day = Some(new_content.trim().to_string());

        if standup.channel.is_some() {
            update_standup_message_in_channel(&user.unwrap(), &mut standup, conn);
        }

        update_standup(&standup, conn);
        Ok(String::from(""))
    } else {
        Err("Couldn't find todays standup, sorry. Mention @progress or send me a message to start the standup flow.".to_string())
    }
}

pub fn get_standup_intro_copy(user: &User, conn: &diesel::PgConnection) -> JsonValue {
    let todays = get_todays_standup_for_user(&user.username, conn);
    match todays {
        None => {
            let latest = get_latest_standup_for_user(&user.username, conn);
            let todays = create_standup(&user.username, &user.team_id, conn);
            gen_standup_copy(latest, todays, &user.channel)
        }
        Some(todays) => {
            let latest = get_standup_before_provided(&user.username, &todays, conn);
            gen_standup_copy(latest, todays, &user.channel)
        }
    }
}

// copy fns
fn gen_standup_copy(
    latest: Option<Standup>,
    todays: Standup,
    channel: &Option<String>,
) -> JsonValue {
    let greet = String::from("*:wave: Thanks for checking in today.*");
    let empty_message = String::from("- _Empty_");
    let intro = String::from(
        "This is your first time using _@progress_, welcome! We'll make this super quick for you.",
    );

    if latest.is_none() {
        json!([{
            "type": "section",
            "text": {
                "type": "mrkdwn",
                "text": format!("{}\n{}\n\n{}", greet, intro, todays.get_copy(channel))
            }
        }])
    } else {
        let standup = latest.unwrap();
        let day_array = get_day_copy_from_standup(&standup);

        let mut all_blocks: Vec<JsonValue> = vec![
            json!({
                "type": "section",
                "text": {
                    "type": "mrkdwn",
                    "text": format!("{}\n{}", greet, "Here's what you were busy with last time we met:\n")
                }
            }),
            json!({
                "type": "section",
                "text": {
                    "type": "mrkdwn",
                    "text": format!("*:calendar:  {}*", format_date(standup.local_date.unwrap_or(standup.date)))
                }
            }),
            json!({
                "type": "divider",
            }),
        ];

        if day_array.len() > 0 {
            for (i, task) in day_array.iter().enumerate() {
                if task.done {
                    all_blocks.push(json!({
                        "type": "section",
                        "text": {
                            "type": "mrkdwn",
                            "text": format!(":white_check_mark: {}", task.content)
                        },
                        "accessory": {
                            "type": "button",
                            "text": {
                                "type": "plain_text",
                                "text": "Mark as not done",
                            },
                            "value": format!("{}-{}", (i+1), task.standup_id),
                            "action_id": "set-task-not-done"
                        }
                    }));
                } else {
                    all_blocks.push(json!({
                        "type": "section",
                        "text": {
                            "type": "mrkdwn",
                            "text": format!("{}", task.content)
                        },
                        "accessory": {
                            "type": "button",
                            "text": {
                                "type": "plain_text",
                                "text": "Mark as done",
                            },
                            "style": "primary",
                            "value": format!("{}-{}", (i+1), task.standup_id),
                            "action_id": "set-task-done"
                        }
                    }));
                }
            }
        } else {
            all_blocks.push(json!({
                "type": "section",
                "text": {
                    "type": "mrkdwn",
                    "text": format!("> {}", empty_message)
                }
            }))
        }

        all_blocks.append(&mut vec![
            json!({
                "type": "divider",
            }),
            json!({
                "type": "section",
                "text": {
                    "type": "mrkdwn",
                    "text": todays.get_copy(channel)
                }
            }),
        ]);

        json!(all_blocks)
    }
}

fn get_day_copy_from_standup(standup: &Standup) -> Vec<Task> {
    if standup.day.is_none() {
        return vec![];
    }

    let tasks = standup.day.as_ref().unwrap().split('\n');
    let mut done = &vec![];

    if standup.done.is_some() {
        done = standup.done.as_ref().unwrap();
    }

    tasks
        .enumerate()
        .map(|(i, task)| Task {
            content: task.trim().to_string(),
            done: done.contains(&((i + 1) as i32)),
            prefix: "".to_string(),
            standup_id: standup.id,
        })
        .collect::<Vec<Task>>()
}

fn format_date(date: NaiveDateTime) -> String {
    let now = Utc::now();
    if date.num_days_from_ce() + 1 == now.num_days_from_ce() {
        date.format("Yesterday, around %I%P").to_string()
    } else if date.num_days_from_ce() + 7 > now.num_days_from_ce() {
        if now.weekday().num_days_from_monday() > date.weekday().num_days_from_monday() {
            // same week
            date.format("This %A, around %I%P").to_string()
        } else {
            // last week
            date.format("Last %A, around %I%P").to_string()
        }
    } else {
        date.format("%A, %d %B %Y, around %I%P").to_string()
    }
}
