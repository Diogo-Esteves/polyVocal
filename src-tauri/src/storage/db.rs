use anyhow::{anyhow, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;
use tauri::{AppHandle, Manager};
use tracing::info;

/// Initialise the SQLite connection pool and run migrations.
///
/// Called synchronously (via `tauri::async_runtime::block_on`) from
/// `lib.rs`'s `setup()`, before the app finishes starting — every
/// `#[tauri::command]` that takes `State<'_, SqlitePool>` would otherwise
/// be able to run before `app.manage(pool)` below has happened.
pub async fn initialise(app: &AppHandle) -> Result<SqlitePool> {
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| anyhow!("failed to resolve app data directory: {e}"))?;

    std::fs::create_dir_all(&app_dir)?;

    let db_path = app_dir.join("polyvocal.db");
    info!("Database path: {}", db_path.display());

    let options = SqliteConnectOptions::from_str(&format!("sqlite:{}", db_path.display()))?
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    run_migrations(&pool).await?;

    app.manage(pool.clone());
    Ok(pool)
}

pub(crate) async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS sessions (
            id          TEXT PRIMARY KEY,
            created_at  TEXT NOT NULL,
            duration_ms INTEGER NOT NULL DEFAULT 0,
            language    TEXT,
            transcript  TEXT NOT NULL,
            translation TEXT,
            target_lang TEXT,
            synced      INTEGER NOT NULL DEFAULT 0
        );
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}
