table! {
    standups (id) {
        id -> Int4,
        username -> Varchar,
        date -> Timestamp,
        prev_day -> Nullable<Varchar>,
        day -> Nullable<Varchar>,
        blocker -> Nullable<Varchar>,
    }
}

table! {
    teams (id) {
        id -> Int4,
        access_token -> Varchar,
        team_id -> Varchar,
        team_name -> Varchar,
        bot_user_id -> Varchar,
        bot_access_token -> Varchar,
    }
}

table! {
    users (id) {
        id -> Int4,
        username -> Varchar,
        channel -> Nullable<Varchar>,
        reminder -> Nullable<Timestamp>,
        real_name -> Varchar,
        avatar_url -> Varchar,
        team_id -> Varchar,
    }
}

allow_tables_to_appear_in_same_query!(
    standups,
    teams,
    users,
);
