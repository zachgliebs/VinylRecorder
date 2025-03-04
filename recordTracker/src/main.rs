use axum::{
    Router,
    extract::{Json, Path, State},
    http::{StatusCode, Method},
    routing::{delete, get, post},
};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Row, Sqlite, sqlite::SqlitePoolOptions};
use std::sync::Arc;
use tower_http::services::ServeDir;
use tower_http::cors::{CorsLayer, Any};
use chrono::Utc;

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

    let app_state = AppState {
        pool: Arc::new(pool),
    };

    // Configure CORS to allow requests from any origin
    let cors = CorsLayer::new()
        .allow_origin(Any) // Allow all origins
        .allow_methods([Method::GET, Method::POST, Method::DELETE]) // Allow specific HTTP methods
        .allow_headers(Any); // Allow all headers

    let app = Router::new()
        .route("/albums", post(add_album).get(get_albums))
        .route("/albums/:album_id", delete(delete_album))
        .route("/play_history", get(get_all_play_history).post(log_play))
        .nest_service("/", ServeDir::new("src/static").append_index_html_on_directories(true)) // Serve static files correctly
        .layer(cors) // Apply CORS middleware
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
            played_on DATETIME DEFAULT NULL,
            finished_on DATETIME DEFAULT NULL,
            FOREIGN KEY (album_id) REFERENCES albums (album_id) ON DELETE CASCADE
        );"
    )
    .execute(pool)
    .await?;

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

    sqlx::query("INSERT INTO albums (title, artist, cover_url, barcode) VALUES (?1, ?2, ?3, ?4)")
        .bind(&payload.title)
        .bind(&payload.artist)
        .bind(&cover_url)
        .bind(&payload.barcode)
        .execute(&*state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::CREATED)
}

async fn get_albums(State(state): State<AppState>) -> Result<Json<Vec<AlbumResponse>>, StatusCode> {
    let rows = sqlx::query("SELECT album_id, title, artist, cover_url FROM albums")
        .fetch_all(&*state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let albums = rows
        .into_iter()
        .map(|row| AlbumResponse {
            album_id: row.get("album_id"),
            title: row.get("title"),
            artist: row.get("artist"),
            cover_url: row.get::<Option<String>, _>("cover_url").unwrap_or_else(|| "default-cover.jpg".to_string()),
        })
        .collect();

    Ok(Json(albums))
}

async fn delete_album(
    State(state): State<AppState>,
    Path(album_id): Path<i64>,
) -> Result<StatusCode, StatusCode> {
    sqlx::query("DELETE FROM albums WHERE album_id = ?1")
        .bind(album_id)
        .execute(&*state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct LogPlayRequest {
    album_id: i64,
    played_on: Option<String>, // Added this field
    finished_on: Option<String>,
}

#[derive(Serialize)]
struct PlayHistoryItem {
    album_id: i64,
    title: String,
    artist: String,
    cover_url: String,
    played_on: String,
}

async fn log_play(
    State(state): State<AppState>,
    Json(payload): Json<LogPlayRequest>,
) -> Result<StatusCode, StatusCode> {
    let played_on = payload.played_on.unwrap_or_else(|| Utc::now().to_rfc3339());

    sqlx::query("INSERT INTO play_history (album_id, played_on, finished_on) VALUES (?1, ?2, ?3)")
        .bind(payload.album_id)
        .bind(played_on)
        .bind(payload.finished_on)
        .execute(&*state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::CREATED)
}

async fn get_all_play_history(
    State(state): State<AppState>,
) -> Result<Json<Vec<PlayHistoryItem>>, StatusCode> {
    let rows = sqlx::query(
        "SELECT ph.album_id, a.title, a.artist, a.cover_url, ph.played_on 
         FROM play_history ph 
         JOIN albums a ON ph.album_id = a.album_id 
         ORDER BY ph.played_on DESC",
    )
    .fetch_all(&*state.pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to fetch play history: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let history = rows
        .into_iter()
        .map(|row| PlayHistoryItem {
            album_id: row.get("album_id"),
            title: row.get("title"),
            artist: row.get("artist"),
            cover_url: row.get::<Option<String>, _>("cover_url").unwrap_or_else(|| "default-cover.jpg".to_string()),
            played_on: row.get("played_on"),
        })
        .collect();

    Ok(Json(history))
}
