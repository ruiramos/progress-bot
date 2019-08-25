use crate::slack;
use crate::{EventDetails, SlackConfig, Standup, StandupList, User, UserList, UserState};
use chrono::Local;
use rocket::State;
use std::sync::{Arc, Mutex};

pub fn challenge(c: String) -> String {
    c
}

pub fn event(
    evt: EventDetails,
    standups: State<Arc<Mutex<StandupList>>>,
    users: State<Arc<Mutex<UserList>>>,
) -> (String, String) {
    let user_list = &mut *users.lock().unwrap();
    let user = user_list.find_user(&evt.user);

    let the_user = match user {
        Some(user) => user,
        None => {
            let user = create_user(&evt.user);
            user_list.add_user(user);
            user_list.list.last_mut().unwrap()
        }
    };

    if evt.r#type == "message" {
        react(evt, the_user, standups)
    } else {
        react_notification(evt, the_user, standups)
    }
}

pub fn react(
    evt: EventDetails,
    user: &mut User,
    standups: State<Arc<Mutex<StandupList>>>,
) -> (String, String) {
    let msg = evt.text;
    let standup_list = &mut *standups.lock().unwrap();

    let copy = match &user.state {
        UserState::Idle => {
            let latest = standup_list.get_latest(&evt.user);
            let result = get_init_standup_copy(latest);
            let standup = Standup::new(&evt.user);
            standup_list.add_standup(standup);
            user.state = UserState::AddPrevDay;
            result
        }
        UserState::AddPrevDay => {
            let standup = standup_list.get_todays_mut(&evt.user).unwrap();
            standup.prev_day = Some(msg);
            user.state = UserState::AddDay;
            get_about_day_copy()
        }
        UserState::AddDay => {
            let standup = standup_list.get_todays_mut(&evt.user).unwrap();
            standup.day = Some(msg);
            user.state = UserState::AddBlocker;
            get_about_blocker_copy()
        }
        UserState::AddBlocker => {
            let standup = standup_list.get_todays_mut(&evt.user).unwrap();
            standup.blocker = Some(msg);
            user.state = UserState::Complete;
            if let Some(_) = user.channel {
                share_standup(&user, &standup);
            }
            get_done_copy()
        }
        UserState::Complete => get_complete_copy(),
    };

    (copy, evt.user)
}

pub fn react_notification(
    evt: EventDetails,
    user: &mut User,
    standups: State<Arc<Mutex<StandupList>>>,
) -> (String, String) {
    let msg = evt.text;
    ("hi there".to_string(), evt.user)
}

pub fn share_standup(user: &User, standup: &Standup) {
    let msg = ":baguette_bread: Here's a fresh new standup for y'all:";
    slack::send_standup_to_channel(
        user.channel.as_ref().unwrap(),
        msg,
        Local::now().timestamp(),
        standup,
        user,
    )
    .unwrap();
}

pub fn config(config: SlackConfig, users: State<Arc<Mutex<UserList>>>) {
    let user_list = &mut *users.lock().unwrap();
    let user = user_list.find_user(&config.user.id);

    if let Some(user) = user {
        user.update_config(config);
    } else {
        let mut user = create_user(&config.user.id);
        user.update_config(config);
        user_list.add_user(user);
    }
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
    let mut text = String::from("*Hello! :wave: Thanks for checking in today.*\n");

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

fn get_about_prev_day_copy() -> String {
    format!(":one: How did *yesterday* go?")
}

fn get_about_day_copy() -> String {
    format!(":two: What are you going to be focusing on *today*?")
}

fn get_about_blocker_copy() -> String {
    format!(":three: Any blockers impacting your work?")
}

fn get_done_copy() -> String {
    format!(
        ":white_check_mark: *All done here!* \n\n Thank you, have a great day, talk to you {}.",
        "tomorrow"
    )
}

fn get_complete_copy() -> String {
    // randomize funny quotes
    format!("You're done for today, off to work you go now! :nerd_face:")
}
