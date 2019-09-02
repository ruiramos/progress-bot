#[macro_use]
extern crate serde_derive;
extern crate chrono;
#[macro_use]
extern crate diesel;

pub mod handle;
pub mod models;
pub mod schema;
pub mod slack;

use self::models::{NewStandup, NewTeam, NewUser, Standup, Team, User};
use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use diesel::prelude::*;
use rocket::request::FromForm;
use schema::standups;
use schema::teams;
use schema::users;

#[derive(Deserialize, Debug)]
pub struct SlackEvent {
    pub token: String,
    pub team_id: String,
    pub challenge: Option<String>,
    pub event: Option<EventDetails>,
}

#[derive(Deserialize, Debug, FromForm)]
pub struct SlackSlashEvent {
    pub token: String,
    pub response_url: String,
    pub trigger_id: String,
    pub user_id: String,
    pub team_id: String,
}

#[derive(Deserialize, Debug)]
pub struct EventDetails {
    pub text: String,
    pub user: String,
    pub channel: String,
    pub r#type: String,
    pub bot_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SlackConfigResource {
    pub id: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SlackConfigTeam {
    pub id: String,
    pub domain: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SlackConfigSubmission {
    pub reminder: Option<String>,
    pub channel: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SlackConfig {
    pub user: SlackConfigResource,
    pub team: SlackConfigTeam,
    pub channel: SlackConfigResource,
    pub submission: SlackConfigSubmission,
    pub response_url: String,
}

#[derive(Debug, FromForm)]
pub struct SlackConfigResponse {
    pub payload: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SlackOauthResponse {
    pub access_token: String,
    pub team_id: String,
    pub team_name: String,
    pub bot: SlackOauthBotInfo,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SlackOauthBotInfo {
    pub bot_user_id: String,
    pub bot_access_token: String,
}

pub fn create_user(username: &str, team_id: &str, conn: &PgConnection) -> User {
    let token = get_bot_token_for_team(team_id, conn);
    let details = slack::get_user_details(username, token);

    match details {
        Ok(details) => {
            let new_user = NewUser {
                username,
                team_id,
                avatar_url: &details.image_48,
                real_name: &details.real_name,
            };

            diesel::insert_into(users::table)
                .values(&new_user)
                .get_result(conn)
                .expect("Error saving new User")
        }
        Err(_) => {
            let new_user = NewUser {
                username,
                team_id,
                avatar_url: &"".to_string(),
                real_name: &"".to_string(),
            };
            diesel::insert_into(users::table)
                .values(&new_user)
                .get_result(conn)
                .expect("Error saving new User")
        }
    }
}

pub fn get_user(un: &str, conn: &PgConnection) -> Option<User> {
    users::table
        .filter(users::username.eq(un))
        .first::<User>(conn)
        .optional()
        .expect("Error getting user")
}

pub fn update_user(user: &mut User, conn: &PgConnection) -> User {
    diesel::update(users::table.find(user.id))
        .set(&*user)
        .get_result(conn)
        .expect("Error updating User")
}

pub fn get_latest_standup_for_user(user: &str, conn: &PgConnection) -> Option<Standup> {
    standups::table
        .filter(standups::username.eq(user))
        .order_by(standups::date.desc())
        .first::<Standup>(conn)
        .optional()
        .expect("Error getting latest standup for user")
}

pub fn get_todays_standup_for_user(user: &str, conn: &PgConnection) -> Option<Standup> {
    let now = Utc::now();
    let d = NaiveDate::from_ymd(now.year(), now.month(), now.day());
    let t = NaiveTime::from_hms_milli(0, 0, 0, 0);
    let today = NaiveDateTime::new(d, t);

    standups::table
        .filter(standups::username.eq(user))
        .filter(standups::date.eq(today))
        .first::<Standup>(conn)
        .optional()
        .expect("Error getting latest standup for user")
}

pub fn remove_todays_standup_for_user(user: &str, conn: &PgConnection) {
    let now = Utc::now();
    let d = NaiveDate::from_ymd(now.year(), now.month(), now.day());
    let t = NaiveTime::from_hms_milli(0, 0, 0, 0);
    let today = NaiveDateTime::new(d, t);

    diesel::delete(
        standups::table
            .filter(standups::username.eq(user))
            .filter(standups::date.eq(today)),
    )
    .execute(conn)
    .expect("Error deleting standup");
}

pub fn create_standup(username: &str, conn: &PgConnection) -> Standup {
    let new_standup = NewStandup::new(username);
    diesel::insert_into(standups::table)
        .values(&new_standup)
        .get_result(conn)
        .expect("Error saving new Standup")
}

pub fn update_standup(standup: &Standup, conn: &PgConnection) -> Standup {
    diesel::update(standups::table.find(standup.id))
        .set(standup)
        .get_result(conn)
        .expect("Error updating Standup")
}

pub fn create_or_update_team_info(res: SlackOauthResponse, conn: &PgConnection) {
    let team = teams::table
        .filter(teams::team_id.eq(&res.team_id))
        .first::<Team>(conn)
        .optional()
        .expect("Error getting team");

    if team.is_none() {
        let new_team = NewTeam {
            access_token: res.access_token,
            team_id: res.team_id,
            team_name: res.team_name,
            bot_user_id: res.bot.bot_user_id,
            bot_access_token: res.bot.bot_access_token,
        };

        diesel::insert_into(teams::table)
            .values(&new_team)
            .get_result::<Team>(conn)
            .expect("Error creating a new team");
    } else {
        // ? not sure should we update or create a new one is there a point?
    }
}

pub fn get_bot_token_for_team(team_id: &str, conn: &PgConnection) -> String {
    let team = teams::table
        .filter(teams::team_id.eq(team_id))
        .first::<Team>(conn)
        .expect("Error getting team");

    team.bot_access_token
}

pub enum StandupState {
    PrevDay,
    Today,
    Blocker,
    Complete,
}

#[cfg(test)]
mod test {
    use crate::schema::standups;
    use crate::{
        create_standup, create_user, get_latest_standup_for_user, get_todays_standup_for_user,
        NewStandup, Standup,
    };
    use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime, Utc};
    use diesel::prelude::*;

    fn get_db() -> PgConnection {
        PgConnection::establish("postgres://diesel:password@localhost:5433/diesel").unwrap()
    }

    #[test]
    fn test_create_standup() {
        let username = "ruiramos";
        let conn = get_db();
        conn.begin_test_transaction().unwrap();

        let standup = create_standup(username, &conn);

        let now = Utc::now();
        let d = NaiveDate::from_ymd(now.year(), now.month(), now.day());
        let t = NaiveTime::from_hms_milli(0, 0, 0, 0);
        let today = NaiveDateTime::new(d, t);

        assert_eq!(standup.username, username);
        assert_eq!(standup.date, today);
    }

    #[test]
    fn test_create_user() {
        let username = "ruiramos";
        let team_id = "abc";
        let conn = get_db();
        conn.begin_test_transaction().unwrap();

        let user = create_user(username, team_id, &conn);

        assert_eq!(user.username, username);
    }

    #[test]
    fn test_get_todays_standup() {
        let username = "ruiramos";
        let conn = get_db();
        conn.begin_test_transaction().unwrap();

        let standup = create_standup(username, &conn);
        let result = get_todays_standup_for_user(username, &conn);

        assert_eq!(standup.id, result.unwrap().id);
    }

    #[test]
    fn test_get_latest_standup() {
        let username = "ruiramos";
        let t = NaiveTime::from_hms_milli(0, 0, 0, 0);
        let standup1 = NewStandup {
            username: username.to_string(),
            date: NaiveDateTime::new(NaiveDate::from_ymd(2011, 02, 05), t),
        };
        let standup2 = NewStandup {
            username: username.to_string(),
            date: NaiveDateTime::new(NaiveDate::from_ymd(2011, 01, 22), t),
        };

        let conn = get_db();
        conn.begin_test_transaction().unwrap();

        let s1_insert: Standup = diesel::insert_into(standups::table)
            .values(&standup1)
            .get_result(&conn)
            .expect("Error saving new Standup");

        let _s2_insert: Standup = diesel::insert_into(standups::table)
            .values(&standup2)
            .get_result(&conn)
            .expect("Error saving new Standup");

        let result = get_latest_standup_for_user(username, &conn);

        assert_eq!(result.unwrap().date, s1_insert.date);
    }

    #[test]
    fn test_get_todays_standup_not_found() {
        let username = "ruiramos";
        let conn = get_db();
        conn.begin_test_transaction().unwrap();

        let result = get_todays_standup_for_user(username, &conn);

        assert!(result.is_none());
    }

    #[test]
    fn test_get_todays_standup_not_found_2() {
        let username = "ruiramos";
        let t = NaiveTime::from_hms_milli(0, 0, 0, 0);

        let standup1 = NewStandup {
            username: username.to_string(),
            date: NaiveDateTime::new(NaiveDate::from_ymd(2011, 02, 05), t),
        };
        let standup2 = NewStandup {
            username: username.to_string(),
            date: NaiveDateTime::new(NaiveDate::from_ymd(2011, 01, 22), t),
        };

        let conn = get_db();
        conn.begin_test_transaction().unwrap();

        diesel::insert_into(standups::table)
            .values(&standup1)
            .get_result::<Standup>(&conn)
            .expect("Error saving new Standup");

        diesel::insert_into(standups::table)
            .values(&standup2)
            .get_result::<Standup>(&conn)
            .expect("Error saving new Standup");

        let result = get_todays_standup_for_user(username, &conn);

        assert!(result.is_none());
    }
}
