diesel::table! {
    albums (id) {
        id -> Integer,
        title -> Text,
        artist -> Text,
        cover_url -> Nullable<Text>,
        created_at -> Nullable<Timestamp>,
    }
}
