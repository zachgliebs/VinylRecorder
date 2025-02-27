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
use chrono::{NaiveDateTime, Duration};

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
        .route("/albums/:album_id", delete(delete_album))
        //.route("/albums/barcode/:barcode", get(get_album_by_barcode))
        .route("/play_history", get(get_all_play_history).post(log_play)) // Play history routes
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
    println!("Creating albums and play_history tables...");
    
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS albums (
            album_id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            artist TEXT NOT NULL,
            cover_url TEXT DEFAULT 'default-cover.jpg',
            barcode TEXT UNIQUE,
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
    		finished_on DATETIME DEFAULT NULL,
    		FOREIGN KEY (album_id) REFERENCES albums (album_id) ON DELETE CASCADE
        );"
    )
    .execute(pool)
    .await?;

    println!("Schema creation complete.");
    Ok(())
}

#[derive(Deserialize)]
struct AddAlbumRequest {
    title: String,
    artist: String,
    cover_url: Option<String>,
    barcode: Option<String>,
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
		"INSERT INTO albums (title, artist, cover_url, barcode) VALUES (?1, ?2, ?3, ?4)"
	)
	.bind(&payload.title)
	.bind(&payload.artist)
	.bind(&cover_url)
	.bind(&payload.barcode)
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

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct LogPlayRequest {
    album_id: i64,
    finished_on: Option<String>, // Optional end time
}

#[derive(Serialize)]
struct PlayHistoryItem {
    album_id: i64,
    title: String,
    artist: String,
    cover_url: String,
    played_on: String,
    duration: Option<String>, // Duration in Xhr, Ymin, Zsec format
}

async fn log_play(
	State(state): State<AppState>,
	Json(payload): Json<LogPlayRequest>,
) -> Result<StatusCode, StatusCode> {
	sqlx::query(
		"INSERT INTO play_history (album_id, finished_on) VALUES (?1, ?2)"
	)
	.bind(payload.album_id)
	.bind(payload.finished_on)
	.execute(&*state.pool)
	.await
	.map_err(|e| {
	    eprintln!("Failed to log play history: {}", e);
	    StatusCode::INTERNAL_SERVER_ERROR
	})?;

	return Ok(StatusCode::CREATED);
}

async fn get_all_play_history(State(state): State<AppState>) -> Result<Json<Vec<PlayHistoryItem>>, StatusCode> {
    let rows = sqlx::query(
        "SELECT ph.album_id, a.title, a.artist, a.cover_url, ph.played_on, ph.finished_on 
         FROM play_history ph 
         JOIN albums a ON ph.album_id = a.album_id 
         ORDER BY ph.played_on DESC"
    )
    .fetch_all(&*state.pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to fetch play history: {}", e); // Log the error
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if rows.is_empty() {
        return Ok(Json(vec![])); // Return an empty array if no data exists
    }

    let history = rows.into_iter().map(|row| {
        let played_on: String = row.get("played_on");
        let finished_on: Option<String> = row.get("finished_on");

        // Calculate duration if finished_on is present
        let duration = if let Some(finished) = &finished_on {
            let start = chrono::NaiveDateTime::parse_from_str(&played_on, "%Y-%m-%d %H:%M:%S").unwrap();
            let end = chrono::NaiveDateTime::parse_from_str(finished, "%Y-%m-%d %H:%M:%S").unwrap();
            let duration_secs = (end - start).num_seconds();
            
            Some(format!(
                "{}hr, {}min, {}sec",
                duration_secs / 3600,
                (duration_secs % 3600) / 60,
                duration_secs % 60
            ))
        } else {
            None
        };

        PlayHistoryItem {
            album_id: row.get("album_id"),
            title: row.get("title"),
            artist: row.get("artist"),
            cover_url: row.get::<Option<String>, _>("cover_url").unwrap_or_else(|| "default-cover.jpg".to_string()),
            played_on,
            duration,
        }
    }).collect();

    Ok(Json(history))
}
