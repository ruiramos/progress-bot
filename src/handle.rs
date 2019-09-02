use crate::slack;
use crate::{
    create_standup, create_user, get_bot_token_for_team, get_latest_standup_for_user,
    get_todays_standup_for_user, get_user, remove_todays_standup_for_user, update_standup,
    update_user,
};
use crate::{EventDetails, SlackConfig, Standup, StandupState, User};
use chrono::Local;

pub fn challenge(c: String) -> String {
    c
}

pub fn event(evt: EventDetails, team_id: &str, conn: &diesel::PgConnection) -> (String, String) {
    let user = match get_user(&evt.user, conn) {
        Some(user) => user,
        None => create_user(&evt.user, team_id, conn),
    };

    if evt.r#type == "message" {
        react(evt, user, conn)
    } else {
        react_notification(evt, user, conn)
    }
}

pub fn react(evt: EventDetails, user: User, conn: &diesel::PgConnection) -> (String, String) {
    let msg = evt.text;
    let todays = get_todays_standup_for_user(&evt.user, conn);

    let copy = match todays {
        None => {
            let latest = get_latest_standup_for_user(&evt.user, conn);
            let todays = create_standup(&evt.user, conn);
            gen_standup_copy(latest, todays, &user.channel)
        }
        Some(mut todays) => {
            if let StandupState::Complete = todays.get_state() {
                "You're done for today, off to work you go now! :nerd_face:".to_string()
            } else {
                match todays.get_state() {
                    StandupState::Blocker => {
                        todays.add_content(&msg);
                        if user.channel.is_some() {
                            share_standup(&user, &todays, &conn);
                        }
                    }
                    _ => todays.add_content(&msg),
                }

                update_standup(&todays, conn);

                todays.get_copy(&user.channel)
            }
        }
    };

    (copy, evt.user)
}

pub fn react_notification(
    evt: EventDetails,
    user: User,
    conn: &diesel::PgConnection,
) -> (String, String) {
    let _msg = evt.text;
    let todays = get_todays_standup_for_user(&evt.user, conn);
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
    (copy, evt.user)
}

pub fn share_standup(user: &User, standup: &Standup, conn: &diesel::PgConnection) {
    let msg = ":newspaper: Here's the latest:";
    slack::send_standup_to_channel(
        user.channel.as_ref().unwrap(),
        msg,
        Local::now().timestamp(),
        standup,
        user,
        get_bot_token_for_team(&user.team_id, conn),
    )
    .unwrap();
}

pub fn config(config: &SlackConfig, conn: &diesel::PgConnection) -> String {
    let mut user = match get_user(&config.user.id, conn) {
        Some(user) => user,
        None => create_user(&config.user.id, &config.team.id, conn),
    };

    user.channel = config.submission.channel.clone();
    //user.reminder = Some(config.submission.reminder);

    update_user(&mut user, conn);

    let copy = match (&config.submission.reminder, &config.submission.channel) {
        (None, None) => "Will not remind you or post your standups anywhere else.".to_string(),
        (None, Some(c)) => format!("Will post your standups in <#{}>.", c),
        (Some(r), None) => format!("Will remind you daily at {}.", r),
        (Some(r), Some(c)) => format!(
            "Will post your standups in <#{}> and remind you daily at {}.",
            c, r
        ),
    };

    copy
}

pub fn remove_todays(user_id: &str, conn: &diesel::PgConnection) -> String {
    remove_todays_standup_for_user(user_id, conn);
    ":shrug: Just forgot all about today's standup, feel free to try again.".to_string()
}

// copy fns

fn gen_standup_copy(latest: Option<Standup>, todays: Standup, channel: &Option<String>) -> String {
    let mut text = String::from("*:wave: Thanks for checking in today.*\n");

    if let Some(standup) = &latest {
        text.push_str("Here's what you were busy with last time we met:\n\n");
        text.push_str(&format!(
            "> :calendar: {}\n\n",
            standup.date.format("%A, %d %B %Y, around %I%P")
        ));

        if let Some(prev) = &standup.prev_day {
            text.push_str(&format!("> *Previous day*: {}\n", &prev));
        }

        if let Some(day) = &standup.day {
            text.push_str(&format!("> *That day*: {}\n", &day));
        }

        if let Some(blockers) = &standup.blocker {
            text.push_str(&format!("> *Blockers*: {}\n", &blockers));
        }
    } else {
        text.push_str("This is your first time using _@progress_, welcome! We'll make this super quick for you.\n\n")
    }

    text.push_str(&format!("\n{}", todays.get_copy(channel)));

    text
}
