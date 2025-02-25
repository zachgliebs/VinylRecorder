use super::schema::*;
use serde::{Serialize};
use chrono::{NaiveDateTime};

#[derive(Queryable, Serialize)]
pub struct Album {
    pub id: i32,
    pub title: String,
    pub artist: String,
    pub cover_url: Option<String>,
    pub created_at: Option<NaiveDateTime>,
}

#[derive(Insertable)]
#[diesel(table_name = albums)]
pub struct NewAlbum<'a> {
    pub title: &'a str,
    pub artist: &'a str,
    pub cover_url: Option<&'a str>,
}
