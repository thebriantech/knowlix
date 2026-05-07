use knowlix_common::{IndexStats, IndexStatus};

#[tauri::command]
pub async fn index_file(file_path: String, project_id: String) -> Result<(), String> {
    knowlix_indexer::index_file(&file_path, &project_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reindex_project(project_id: String) -> Result<IndexStats, String> {
    knowlix_indexer::reindex_project(&project_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_index_status(project_id: String) -> Result<IndexStatus, String> {
    knowlix_indexer::get_index_status(&project_id)
        .await
        .map_err(|e| e.to_string())
}
