use knowlix_common::{IndexProgress, IndexStats, IndexStatus};
use tauri::Emitter;

#[tauri::command]
pub async fn index_file(file_path: String, project_id: String) -> Result<(), String> {
    knowlix_indexer::index_file(&file_path, &project_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reindex_project(
    project_id: String,
    app_handle: tauri::AppHandle,
) -> Result<IndexStats, String> {
    let on_progress: Box<dyn Fn(IndexProgress) + Send + Sync> = Box::new(move |progress| {
        app_handle.emit("indexing_progress", &progress).ok();
    });
    knowlix_indexer::reindex_project(&project_id, Some(on_progress))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_index_status(project_id: String) -> Result<IndexStatus, String> {
    knowlix_indexer::get_index_status(&project_id)
        .await
        .map_err(|e| e.to_string())
}
