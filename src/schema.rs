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
    users (id) {
        id -> Int4,
        username -> Varchar,
        channel -> Nullable<Varchar>,
        reminder -> Nullable<Timestamp>,
        real_name -> Varchar,
        avatar_url -> Varchar,
    }
}

allow_tables_to_appear_in_same_query!(
    standups,
    users,
);
