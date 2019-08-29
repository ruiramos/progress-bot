use crate::slack;
use crate::{EventDetails, SlackConfig, Standup, StandupList, StandupState, User, UserList};
use chrono::Local;

pub fn challenge(c: String) -> String {
    c
}

pub fn event(
    evt: EventDetails,
    standups: &mut StandupList,
    users: &mut UserList,
) -> (String, String) {
    let user = match users.get_mut(&evt.user) {
        Some(user) => user,
        None => {
            let tmp = create_user(&evt.user);
            users.insert(evt.user.clone(), tmp);
            users.get_mut(&evt.user).unwrap()
        }
    };

    if evt.r#type == "message" {
        react(evt, user, standups)
    } else {
        react_notification(evt, user, standups)
    }
}

pub fn react(evt: EventDetails, user: &mut User, standups: &mut StandupList) -> (String, String) {
    let msg = evt.text;
    let todays = standups.get_todays_mut(&evt.user);

    let copy = match todays {
        None => {
            let latest = standups.get_latest(&evt.user);
            let result = get_init_standup_copy(latest);
            let standup = Standup::new(&evt.user);
            standups.add_standup(standup);
            result
        }
        Some(todays) => {
            let copy = todays.get_copy(&user.channel);
            match todays.get_state() {
                StandupState::Blocker => {
                    todays.add_content(&msg);
                    if user.channel.is_some() {
                        share_standup(&user, &todays);
                    }
                }
                _ => todays.add_content(&msg),
            }

            copy
        }
    };

    (copy, evt.user)
}

pub fn react_notification(
    evt: EventDetails,
    user: &mut User,
    standups: &mut StandupList,
) -> (String, String) {
    let _msg = evt.text;
    let todays = standups.get_todays_mut(&evt.user);
    let copy = match todays {
        None => "Hi there!".to_string(),
        Some(s) => s.get_copy(&user.channel),
    };
    (copy, evt.user)
}

pub fn share_standup(user: &User, standup: &Standup) {
    let msg = ":newspaper: Here's the latest:";
    slack::send_standup_to_channel(
        user.channel.as_ref().unwrap(),
        msg,
        Local::now().timestamp(),
        standup,
        user,
    )
    .unwrap();
}

pub fn config(config: &SlackConfig, users: &mut UserList) -> String {
    let user = users.get_mut(&config.user.id);

    if let Some(user) = user {
        user.update_config(&config);
    } else {
        let mut user = create_user(&config.user.id);
        user.update_config(&config);
        users.insert(user.username.clone(), user);
    }

    let copy = match (&config.submission.reminder, &config.submission.channel) {
        (None, None) => "No changes made.".to_string(),
        (None, Some(c)) => format!("Will now post your standups in <#{}>.", c),
        (Some(r), None) => format!("Will now remind you daily at {}.", r),
        (Some(r), Some(c)) => format!(
            "Will now post your standups in <#{}> and remind you daily at {}.",
            c, r
        ),
    };

    copy
}

pub fn remove_todays(user_id: &str, standups: &mut StandupList) {
    standups.remove_todays_from_user(user_id);
}

fn create_user(username: &str) -> User {
    let mut temp = User::new(username);

    if let Ok(details) = slack::get_user_details(username) {
        temp.real_name = Some(details.real_name);
        temp.avatar_url = Some(details.image_48);
    }

    temp
}

// copy fns

fn get_init_standup_copy(latest: Option<&Standup>) -> String {
    let mut text = String::from("*:wave: Thanks for checking in today.*\n");

    if let Some(standup) = &latest {
        text.push_str("Here's what you were busy with last time we met:\n\n");
        text.push_str(&format!("> :calendar: {}\n\n", standup.date));

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

    text.push_str(
        "\n:one: Firstly how did *yesterday* go? In one line, what were you able to achieve?",
    );

    text
}
