extern crate chrono;
extern crate diesel;
extern crate dotenv;

use chrono::prelude::*;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::sql_query;
use dotenv::dotenv;
use std::env;

use progress_bot::get_bot_token_for_team;
use progress_bot::models::User;
use progress_bot::schema::users;
use progress_bot::slack;

fn main() {
    dotenv().ok();

    let conn = establish_connection();

    let users = sql_query(
        "SELECT * FROM users \
         WHERE reminder IS NOT NULL \
         AND extract('hour' from reminder) = extract('hour' from now()) \
         AND (last_notified IS NULL \
         OR date_trunc('day', last_notified) != date_trunc('day', now())); ",
    )
    .load::<User>(&conn)
    .expect("Error loading users");

    println!("{:?}", users);

    for user in users.iter() {
        notify_user(user, &conn);
        set_last_notified(user, &conn);
    }
}

pub fn establish_connection() -> PgConnection {
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    PgConnection::establish(&database_url).expect(&format!("Error connecting to {}", database_url))
}

pub fn notify_user(user: &User, conn: &PgConnection) {
    println!("notify! {}", user.username);
    let message = format!(
        "Hey <@{}>, is this a good time for your standup today? :)",
        user.username
    );
    let token = get_bot_token_for_team(&user.team_id, conn);
    slack::send_message(message, user.username.to_string(), token)
        .expect(&format!("Failed to notify user {}", user.username));
}

pub fn set_last_notified(user: &User, conn: &PgConnection) {
    diesel::update(users::table.find(user.id))
        .set(users::last_notified.eq(Utc::now().naive_utc()))
        .get_result::<User>(conn)
        .expect("Error updating User");
}
