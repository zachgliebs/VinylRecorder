#[macro_use]
extern crate diesel;

use actix_web::{web, App, HttpServer, Responder};
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use dotenvy::dotenv;
use serde::Deserialize;
use std::env;

mod schema;
mod models;

use crate::models::{Album, NewAlbum};

// Type alias for Diesel connection pool
type DbPool = r2d2::Pool<ConnectionManager<SqliteConnection>>;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    let pool = r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create database connection pool.");

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(pool.clone()))
            .route("/albums", web::get().to(get_albums))
            .route("/albums", web::post().to(add_album))
            .route("/albums/{id}", web::delete().to(delete_album))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}

async fn get_albums(pool: web::Data<DbPool>) -> impl Responder {
    use crate::schema::albums::dsl::*;

    let mut conn = pool.get().expect("Failed to get DB connection");
    let results = albums
        .select((
            id,
            title,
            artist,
            cover_url,
            created_at,
        ))
        .load::<Album>(&mut conn)
        .expect("Error loading albums");

    web::Json(results)
}

#[derive(Deserialize)]
struct AlbumInput {
    title: String,
    artist: String,
    cover_url: Option<String>,
}

async fn add_album(pool: web::Data<DbPool>, album_input: web::Json<AlbumInput>) -> impl Responder {
    use crate::schema::albums;

    let mut conn = pool.get().expect("Failed to get DB connection");
    let new_album = NewAlbum {
        title: &album_input.title,
        artist: &album_input.artist,
        cover_url: album_input.cover_url.as_deref(),
    };

    diesel::insert_into(albums::table)
        .values(&new_album)
        .execute(&mut conn)
        .expect("Error adding album");

    "Album added successfully"
}

async fn delete_album(pool: web::Data<DbPool>, id: web::Path<i32>) -> impl Responder {
    use crate::schema::albums::dsl::*;

    let mut conn = pool.get().expect("Failed to get DB connection");
    diesel::delete(albums.filter(id.eq(*id)))
        .execute(&mut conn)
        .expect("Error deleting album");

    "Album deleted successfully"
}
