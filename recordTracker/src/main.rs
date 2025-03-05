use axum::{
    Router,
    extract::{Json, Path, State},
    http::StatusCode,
    routing::{delete, get, post},
};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Row, Sqlite, sqlite::SqlitePoolOptions};
use std::sync::Arc;
use tower_http::services::ServeDir;
use serde_json::Value;

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

    let app = Router::new()
        .route("/albums", post(add_album).get(get_albums))
        .route("/albums/:album_id", delete(delete_album))
        .route("/albums/barcode/:barcode", get(get_album_by_barcode)) // New route for fetching albums by barcode
        .route("/play_history", get(get_play_history).post(add_play_entry))
        .route("/now_playing", get(get_now_playing))
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
			barcode TEXT UNIQUE, -- Add barcode column
			created_on DATETIME DEFAULT (datetime('now', 'localtime'))
		);",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS play_history (
			play_id INTEGER PRIMARY KEY AUTOINCREMENT,
			album_id INTEGER NOT NULL,
			played_on DATETIME DEFAULT (datetime('now', 'localtime')),
			FOREIGN KEY (album_id) REFERENCES albums (album_id) ON DELETE CASCADE
		);",
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
    barcode: Option<String>, // Add barcode field
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
    let cover_url = payload
        .cover_url
        .unwrap_or_else(|| "default-cover.jpg".to_string());

    sqlx::query("INSERT INTO albums (title, artist, cover_url, barcode) VALUES (?1, ?2, ?3, ?4)")
        .bind(&payload.title)
        .bind(&payload.artist)
        .bind(&cover_url)
        .bind(&payload.barcode) // Bind barcode value
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

    let albums = rows
        .into_iter()
        .map(|row| AlbumResponse {
            album_id: row.get("album_id"),
            title: row.get("title"),
            artist: row.get("artist"),
            cover_url: row
                .get::<Option<String>, _>("cover_url")
                .unwrap_or_else(|| "default-cover.jpg".to_string()),
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

    Ok(StatusCode::NO_CONTENT) // Return 204 No Content on success
}

// New function to fetch an album by its barcode
async fn get_album_by_barcode(
    State(state): State<AppState>,
    Path(barcode): Path<String>,
) -> Result<Json<AlbumResponse>, StatusCode> {
    let row =
        sqlx::query("SELECT album_id, title, artist, cover_url FROM albums WHERE barcode = ?1")
            .bind(barcode)
            .fetch_optional(&*state.pool)
            .await
            .map_err(|e| {
                eprintln!("Failed to fetch album by barcode: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

    if let Some(row) = row {
        let album = AlbumResponse {
            album_id: row.get("album_id"),
            title: row.get("title"),
            artist: row.get("artist"),
            cover_url: row
                .get::<Option<String>, _>("cover_url")
                .unwrap_or_else(|| "default-cover.jpg".to_string()),
        };
        Ok(Json(album))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

#[derive(Serialize)]
struct PlayHistoryResponse {
    play_id: i64,
    album_id: i64,
    played_on: NaiveDateTime,
}

#[derive(Debug, Deserialize)]
struct AddPlayRequest {
    album_id: i64,
}

async fn get_play_history(
    State(state): State<AppState>,
) -> Result<Json<Vec<PlayHistoryResponse>>, StatusCode> {
    let rows = sqlx::query_as!(
        PlayHistoryResponse,
        r#"
		SELECT 
			play_id AS "play_id!", 
			album_id AS "album_id!", 
			played_on AS "played_on!" 
		FROM play_history 
		ORDER BY played_on DESC
		"#
    )
    .fetch_all(&*state.pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to fetch play history: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(rows))
}


async fn add_play_entry(
    State(state): State<AppState>,
    Json(payload): Json<Value>, // Accept dynamic JSON payload
) -> Result<StatusCode, StatusCode> {
    if let Some(album_id) = payload.get("album_id").and_then(|v| v.as_i64()) {
        println!("Received album_id: {}", album_id);

        sqlx::query!(
            "INSERT INTO play_history (album_id) VALUES (?1)",
            album_id
        )
        .execute(&*state.pool)
        .await
        .map_err(|e| {
            eprintln!("Failed to insert play entry: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        Ok(StatusCode::CREATED)
    } else {
        eprintln!("Invalid payload: {:?}", payload);
        Err(StatusCode::UNPROCESSABLE_ENTITY)
    }
}


#[derive(Serialize)]
struct NowPlayingResponse {
    play_id: i64,
    album_id: i64,
    played_on: NaiveDateTime,
}

async fn get_now_playing(
    State(state): State<AppState>,
) -> Result<Json<Option<NowPlayingResponse>>, StatusCode> {
    let result = sqlx::query(
        "SELECT play_id, album_id, played_on 
         FROM play_history 
         ORDER BY play_id DESC 
         LIMIT 1",
    )
    .fetch_optional(&*state.pool) // Dereference Arc to get Pool<Sqlite>
    .await
    .map_err(|e| {
        eprintln!("Database query failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if let Some(row) = result {
        let response = NowPlayingResponse {
            play_id: row.get("play_id"),
            album_id: row.get("album_id"),
            played_on: row.get("played_on"),
        };
        Ok(Json(Some(response)))
    } else {
        Ok(Json(None)) // No currently playing album found
    }
}
