use std::collections::HashMap;

use knowlix_common::IndexFileStatus;
use tauri::{Emitter, State};
use tokio::sync::Mutex;
use tokio::time::Duration;

const SCAN_INTERVAL: Duration = Duration::from_secs(30);

pub(crate) struct WatcherEntry {
    scan_abort: tokio::task::AbortHandle,
}

pub struct WatcherState(pub Mutex<HashMap<String, WatcherEntry>>);

#[tauri::command]
pub async fn start_file_watcher(
    project_id: String,
    app_handle: tauri::AppHandle,
    state: State<'_, WatcherState>,
) -> Result<(), String> {
    let mut watchers = state.0.lock().await;
    if watchers.contains_key(&project_id) {
        return Ok(());
    }

    tracing::info!("[watcher] started project={project_id}");

    let project_id_scan = project_id.clone();
    let scan_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(SCAN_INTERVAL);
        interval.tick().await; // skip immediate first tick — initial index done by reindex_project
        loop {
            interval.tick().await;
            let in_progress = knowlix_indexer::get_index_status(&project_id_scan)
                .await
                .map(|s| s.in_progress)
                .unwrap_or(true);
            if in_progress {
                continue;
            }
            if let Ok(stats) = knowlix_indexer::reindex_project(&project_id_scan, None).await {
                for r in &stats.file_results {
                    match r.status {
                        IndexFileStatus::Indexed => {
                            app_handle.emit("file_indexed", &r.path).ok();
                        }
                        IndexFileStatus::Removed => {
                            app_handle.emit("file_removed", &r.path).ok();
                        }
                        _ => {}
                    }
                }
                app_handle.emit("reindex_complete", &stats).ok();
            }
        }
    });

    watchers.insert(project_id, WatcherEntry { scan_abort: scan_task.abort_handle() });

    Ok(())
}

#[tauri::command]
pub async fn stop_file_watcher(
    project_id: String,
    state: State<'_, WatcherState>,
) -> Result<(), String> {
    if let Some(entry) = state.0.lock().await.remove(&project_id) {
        entry.scan_abort.abort();
    }
    Ok(())
}
