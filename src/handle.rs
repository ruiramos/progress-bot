use crate::schema::standups;
use crate::slack;
use crate::{
    create_standup, create_user, get_bot_token_for_team, get_latest_standup_for_user,
    get_number_emoji, get_todays_standup_for_user, get_user, remove_todays_standup_for_user,
    update_standup, update_user,
};
use crate::{EventDetails, SlackConfig, Standup, StandupState, User};
use chrono::{Datelike, Local, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use diesel::prelude::*;

pub fn challenge(c: String) -> String {
    c
}

pub fn event(
    evt: EventDetails,
    team_id: &str,
    conn: &diesel::PgConnection,
) -> Option<(String, String)> {
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

pub fn react(evt: EventDetails, team_id: &str, conn: &diesel::PgConnection) -> (String, String) {
    let user = match get_user(&evt.user.as_ref().unwrap(), conn) {
        Some(user) => user,
        None => create_user(&evt.user.as_ref().unwrap(), team_id, conn),
    };

    let msg = evt.text.as_ref().unwrap();
    let todays = get_todays_standup_for_user(&user.username, conn);

    let copy = match todays {
        None => {
            let latest = get_latest_standup_for_user(&user.username, conn);
            let todays = create_standup(&user.username, &user.team_id, conn);
            gen_standup_copy(latest, todays, &user.channel)
        }
        Some(mut todays) => {
            if let StandupState::Complete = todays.get_state() {
                "You're done for today, off to work you go now! :nerd_face:".to_string()
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

                todays.get_copy(&user.channel)
            }
        }
    };

    (copy, user.username)
}

pub fn react_message_edit(
    evt: EventDetails,
    conn: &diesel::PgConnection,
) -> Option<(String, String)> {
    let previous_message = evt.previous_message.unwrap();
    let new_message = evt.message.unwrap();
    let username = previous_message.user;
    let user = get_user(&username, conn);

    if user.is_none() {
        return Some((
            "Very weird error, couldn't find your user, sorry.".to_string(),
            username,
        ));
    }

    let todays = get_todays_standup_for_user(&username, conn);

    match todays {
        None => Some((
            "Very weird error, couldn't find the standup you just edited, sorry.".to_string(),
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
                return None;
            }

            if standup.channel.is_some() {
                let user = user.unwrap();
                let ack = slack::update_standup_in_channel(
                    &standup,
                    &user,
                    Local::now().timestamp(),
                    get_bot_token_for_team(&user.team_id, conn),
                );

                standup.message_ts = Some(ack.unwrap().ts);
            }

            update_standup(&standup, conn);
            Some((
                ":white_check_mark: Standup updated, thanks!".to_string(),
                username,
            ))
        }
    }
}

pub fn react_notification(
    evt: EventDetails,
    team_id: &str,
    conn: &diesel::PgConnection,
) -> (String, String) {
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
            }
        }
    };
    (copy, user.username)
}

pub fn react_app_home_open(
    evt: EventDetails,
    team_id: &str,
    conn: &diesel::PgConnection,
) -> Option<(String, String)> {
    let user = match get_user(&evt.user.as_ref().unwrap(), conn) {
        Some(user) => user,
        None => create_user(&evt.user.as_ref().unwrap(), team_id, conn),
    };
    let todays = get_todays_standup_for_user(&user.username, conn);

    println!("app_home_open recieved: {:?}", evt);

    if todays.is_none() {
        Some((
            String::from("Hey there! Let me know if this is a good time for your standup today."),
            user.username,
        ))
    } else {
        None
    }
}

pub fn share_standup(user: &User, standup: &Standup, conn: &diesel::PgConnection) {
    let msg = ":newspaper: Here's the latest:";
    let ack = slack::send_standup_to_channel(
        user.channel.as_ref().unwrap(),
        msg,
        Local::now().timestamp(),
        standup,
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

    user.channel = config.submission.channel.clone();

    if let Some(reminder) = &config.submission.reminder {
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

    let copy = match (&config.submission.reminder, &config.submission.channel) {
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

pub fn get_todays_tasks(user_id: &str, team_id: &str, conn: &diesel::PgConnection) -> String {
    let user = match get_user(&user_id, conn) {
        Some(user) => user,
        None => create_user(&user_id, team_id, conn),
    };
    let todays = get_todays_standup_for_user(user_id, conn);
    if let Some(standup) = todays {
        if standup.day.is_some() {
            let tasks: String = standup
                .day
                .unwrap()
                .split('\n')
                .enumerate()
                .map(|(i, x)| format!("> {} {}", get_number_emoji(i + 1), x))
                .collect::<Vec<String>>()
                .join("\n");

            format!(
                "Hey {}, here's what you have in store for *today*: \n{}",
                user.real_name, tasks
            )
        } else {
            format!("You still haven't told me what you'll be doing today! Please finish your standup first. \n {}", standup.get_copy(&user.channel))
        }
    } else {
        "Couldn't find todays standup, sorry. Mention @progress or send me a message to start the standup flow.".to_string()
    }
}

// copy fns
fn gen_standup_copy(latest: Option<Standup>, todays: Standup, channel: &Option<String>) -> String {
    let mut text = String::from("*:wave: Thanks for checking in today.*\n");

    if let Some(standup) = &latest {
        text.push_str("Here's what you were busy with last time we met:\n\n");
        text.push_str(&format!(
            "> *:calendar:  {}*\n\n",
            format_date(standup.date)
        ));

        if let Some(prev) = &standup.prev_day {
            text.push_str(&format!(
                "> *Day before*: \n> {}\n",
                &prev.replace("\n", "\n>")
            ));
        }

        if let Some(day) = &standup.day {
            text.push_str(&format!(
                "> *That day*: \n> {}\n",
                &day.replace("\n", "\n>")
            ));
        }

        if let Some(blockers) = &standup.blocker {
            text.push_str(&format!(
                "> *Blockers*: \n> {}\n",
                &blockers.replace("\n", "\n>")
            ));
        }
    } else {
        text.push_str("This is your first time using _@progress_, welcome! We'll make this super quick for you.\n\n")
    }

    text.push_str(&format!("\n{}", todays.get_copy(channel)));

    text
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
