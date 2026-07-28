use crate::storage::models::Session;

#[tauri::command]
pub async fn list_sessions(
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<Session>, String> {
    // TODO: resolve SessionRepository from Tauri state, call list()
    Ok(vec![])
}

#[tauri::command]
pub async fn get_session(id: String) -> Result<Option<Session>, String> {
    // TODO: resolve SessionRepository from Tauri state, call get()
    Ok(None)
}

#[tauri::command]
pub async fn delete_session(id: String) -> Result<(), String> {
    // TODO: resolve SessionRepository from Tauri state, call delete()
    Ok(())
}
