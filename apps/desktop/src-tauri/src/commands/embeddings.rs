use knowlix_common::EmbeddingModelStatus;
use tauri::{AppHandle, Emitter, Manager};

fn embedding_cache_dir(app: &AppHandle) -> std::path::PathBuf {
    app.path()
        .app_data_dir()
        .expect("app data dir")
        .join("cache")
        .join("embeddings")
}

#[tauri::command]
pub fn get_embedding_model_status() -> EmbeddingModelStatus {
    knowlix_storage::get_embedding_model_status()
}

/// Starts the embedding model download in the background.
/// Emits `embedding_model_downloading`, `embedding_model_ready`, or `embedding_model_error`.
#[tauri::command]
pub async fn ensure_embedding_model(app: AppHandle) -> Result<(), String> {
    if knowlix_storage::is_embedding_ready() {
        return Ok(());
    }
    if knowlix_storage::is_embedding_downloading() {
        return Ok(());
    }
    let cache_dir = embedding_cache_dir(&app);
    knowlix_storage::set_embedding_cache_dir(cache_dir);
    let app2 = app.clone();
    tokio::spawn(async move {
        let _ = app2.emit("embedding_model_downloading", ());
        let result = tokio::task::spawn_blocking(knowlix_storage::ensure_embedding_model_blocking)
            .await;
        match result {
            Ok(Ok(())) => {
                tracing::info!("[embeddings] model ready");
                let _ = app2.emit("embedding_model_ready", ());
            }
            Ok(Err(e)) => {
                tracing::warn!("[embeddings] model init failed: {e}");
                let _ = app2.emit("embedding_model_error", e.to_string());
            }
            Err(e) => {
                tracing::warn!("[embeddings] spawn_blocking failed: {e}");
                let _ = app2.emit("embedding_model_error", e.to_string());
            }
        }
    });
    Ok(())
}
