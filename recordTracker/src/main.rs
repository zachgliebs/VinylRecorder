use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    routing::{delete, get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePoolOptions, Pool, Row, Sqlite};
use std::sync::Arc;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
    let db_url = "sqlite://sqlite.db";
    println!("Using database at {}", db_url);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(db_url)
        .await
        .expect("Failed to connect to database");

    if let Err(e) = create_schema(&pool).await {
        eprintln!("Failed to create schema: {}", e);
        return;
    }

    let app_state = AppState { pool: Arc::new(pool) };

    let app = Router::new()
        .route("/albums", post(add_album).get(get_albums))
        .route("/albums/:album_id", delete(delete_album)) // Add DELETE route
        .route("/play_history/:album_id", get(get_play_history))
        .nest_service("/", ServeDir::new("src/static"))
        .with_state(app_state);

    println!("Server running on http://localhost:3000");
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[derive(Clone)]
struct AppState {
    pool: Arc<Pool<Sqlite>>,
}

async fn create_schema(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    println!("Creating albums table...");
    
	sqlx::query(
		"CREATE TABLE IF NOT EXISTS albums (
			album_id INTEGER PRIMARY KEY AUTOINCREMENT,
			title TEXT NOT NULL,
			artist TEXT NOT NULL,
			cover_url TEXT DEFAULT 'default-cover.jpg',
			created_on DATETIME DEFAULT (datetime('now', 'localtime'))
		);"
	)
	.execute(pool)
	.await?;

	sqlx::query(
		"CREATE TABLE IF NOT EXISTS play_history (
			play_id INTEGER PRIMARY KEY AUTOINCREMENT,
			album_id INTEGER NOT NULL,
			played_on DATETIME DEFAULT (datetime('now', 'localtime')),
			FOREIGN KEY (album_id) REFERENCES albums (album_id) ON DELETE CASCADE
		);"
	)
	.execute(pool)
	.await?;

	println!("Schema creation complete.");
	return Ok(());
}

#[derive(Deserialize)]
struct AddAlbumRequest {
    title: String,
    artist: String,
    cover_url: Option<String>,
}

#[derive(Serialize)]
struct AlbumResponse {
    album_id: i64,
    title: String,
    artist: String,
    cover_url: String,
}

async fn add_album(
    State(state): State<AppState>,
    Json(payload): Json<AddAlbumRequest>,
) -> Result<StatusCode, StatusCode> {
    let cover_url = payload.cover_url.unwrap_or_else(|| "default-cover.jpg".to_string());

	sqlx::query(
		"INSERT INTO albums (title, artist, cover_url) VALUES (?1, ?2, ?3)"
	)
	.bind(&payload.title)
	.bind(&payload.artist)
	.bind(&cover_url)
	.execute(&*state.pool)
	.await
	.map_err(|e| {
	    eprintln!("Failed to insert album: {}", e);
	    StatusCode::INTERNAL_SERVER_ERROR
	})?;

	return Ok(StatusCode::CREATED);
}

async fn get_albums(State(state): State<AppState>) -> Result<Json<Vec<AlbumResponse>>, StatusCode> {
	let rows = sqlx::query("SELECT album_id, title, artist, cover_url FROM albums")
	    .fetch_all(&*state.pool)
	    .await
	    .map_err(|e| {
	        eprintln!("Failed to fetch albums: {}", e);
	        StatusCode::INTERNAL_SERVER_ERROR
	    })?;

	let albums = rows.into_iter()
	    .map(|row| AlbumResponse {
	        album_id: row.get("album_id"),
	        title: row.get("title"),
	        artist: row.get("artist"),
	        cover_url: row.get::<Option<String>, _>("cover_url").unwrap_or_else(|| "default-cover.jpg".to_string()),
	    })
	    .collect();

	return Ok(Json(albums));
}

// New function to delete an album by ID
async fn delete_album(
    State(state): State<AppState>,
    Path(album_id): Path<i64>,
) -> Result<StatusCode, StatusCode> {
    sqlx::query("DELETE FROM albums WHERE album_id = ?1")
        .bind(album_id)
        .execute(&*state.pool)
        .await
        .map_err(|e| {
            eprintln!("Failed to delete album with id {}: {}", album_id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(StatusCode::NO_CONTENT) // Return 204 No Content on success
}

#[derive(Serialize)]
struct PlayHistoryResponse {
	play_id: i64,
	played_on: String,
}

async fn get_play_history(
	State(state): State<AppState>,
	Path(album_id): Path<i64>,
) -> Result<Json<Vec<PlayHistoryResponse>>, StatusCode> {
	let rows = sqlx::query("SELECT play_id, played_on FROM play_history WHERE album_id = ?1")
	    .bind(album_id)
	    .fetch_all(&*state.pool)
	    .await
	    .map_err(|e| {
	        eprintln!("Failed to fetch play history: {}", e);
	        StatusCode::INTERNAL_SERVER_ERROR
	    })?;

	let history = rows.into_iter()
	    .map(|row| PlayHistoryResponse {
	        play_id: row.get("play_id"),
	        played_on: row.get("played_on"),
	    })
	    .collect();

	return Ok(Json(history));
}
