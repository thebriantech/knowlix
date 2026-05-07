use knowlix_common::SearchResult;

#[tauri::command]
pub async fn search(
    query: String,
    project_id: Option<String>,
    limit: usize,
) -> Result<Vec<SearchResult>, String> {
    knowlix_search::search(&query, project_id.as_deref(), limit)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn search_keyword(
    query: String,
    project_id: Option<String>,
    limit: usize,
) -> Result<Vec<SearchResult>, String> {
    knowlix_search::search_keyword(&query, project_id.as_deref(), limit)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn search_semantic(
    query: String,
    project_id: Option<String>,
    limit: usize,
) -> Result<Vec<SearchResult>, String> {
    knowlix_search::search_semantic(&query, project_id.as_deref(), limit)
        .await
        .map_err(|e| e.to_string())
}
