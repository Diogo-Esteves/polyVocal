use crate::storage::models::Session;
use crate::storage::repository::SessionRepository;
use sqlx::SqlitePool;
use tauri::State;

#[tauri::command]
pub async fn list_sessions(
    pool: State<'_, SqlitePool>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<Session>, String> {
    let repository = SessionRepository::new(pool.inner().clone());
    repository
        .list(limit.unwrap_or(50), offset.unwrap_or(0))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_session(
    pool: State<'_, SqlitePool>,
    id: String,
) -> Result<Option<Session>, String> {
    let repository = SessionRepository::new(pool.inner().clone());
    repository.get(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_session(pool: State<'_, SqlitePool>, id: String) -> Result<(), String> {
    let repository = SessionRepository::new(pool.inner().clone());
    repository.delete(&id).await.map_err(|e| e.to_string())
}
