use knowlix_common::WikiPage;
use tauri::Emitter;

#[tauri::command]
pub async fn generate_project_wiki(
    project_id: String,
    force_regenerate: bool,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(32);

    // Forward progress events to frontend via Tauri events
    tauri::async_runtime::spawn(async move {
        while let Some(progress) = rx.recv().await {
            let _ = app.emit("wiki:progress", progress);
        }
    });

    knowlix_wiki::generate_project_wiki(&project_id, force_regenerate, tx)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn generate_global_wiki(
    force_regenerate: bool,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(32);

    tauri::async_runtime::spawn(async move {
        while let Some(progress) = rx.recv().await {
            let _ = app.emit("wiki:progress", progress);
        }
    });

    knowlix_wiki::generate_global_wiki(force_regenerate, tx)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_project_wiki(project_id: String) -> Result<Vec<WikiPage>, String> {
    knowlix_wiki::get_project_wiki(&project_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_global_wiki() -> Result<Vec<WikiPage>, String> {
    knowlix_wiki::get_global_wiki()
        .await
        .map_err(|e| e.to_string())
}
