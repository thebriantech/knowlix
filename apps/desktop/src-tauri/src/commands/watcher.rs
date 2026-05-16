use std::collections::HashMap;

use knowlix_common::IndexFileStatus;
use knowlix_watcher::FileEvent;
use tauri::{Emitter, State};
use tokio::sync::Mutex;
use tokio::time::{Duration, Instant};

const SCAN_INTERVAL: Duration = Duration::from_secs(30);

pub(crate) struct WatcherEntry {
    _watcher: knowlix_watcher::WatcherHandle,
    event_abort: tokio::task::AbortHandle,
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

    let project = knowlix_storage::get_project(&project_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Project not found: {project_id}"))?;

    let (tx, mut rx) = tokio::sync::mpsc::channel::<FileEvent>(100);

    let watcher = knowlix_watcher::watch_project(&project_id, project.folders, tx)
        .map_err(|e| e.to_string())?;

    tracing::info!("[watcher] started project={project_id}");

    // OS-event task: debounce and index individual file events
    let project_id_ev = project_id.clone();
    let app_handle_ev = app_handle.clone();
    let event_task = tokio::spawn(async move {
        // path -> (deadline, is_delete)
        let mut pending: HashMap<String, (Instant, bool)> = HashMap::new();
        const DEBOUNCE: Duration = Duration::from_millis(300);

        loop {
            let next = pending.values().map(|(t, _)| *t).min();

            tokio::select! {
                event = rx.recv() => {
                    match event {
                        None => break,
                        Some(FileEvent::Created(path) | FileEvent::Modified(path)) => {
                            tracing::info!("[watcher] event modified/created path={path}");
                            pending.insert(path, (Instant::now() + DEBOUNCE, false));
                        }
                        Some(FileEvent::Deleted(path)) => {
                            tracing::info!("[watcher] event deleted path={path}");
                            pending.insert(path, (Instant::now() + DEBOUNCE, true));
                        }
                    }
                }
                _ = async {
                    match next {
                        Some(t) => tokio::time::sleep_until(t).await,
                        None => std::future::pending::<()>().await,
                    }
                } => {
                    let now = Instant::now();
                    let ready: Vec<(String, bool)> = pending
                        .iter()
                        .filter(|(_, (t, _))| now >= *t)
                        .map(|(p, (_, del))| (p.clone(), *del))
                        .collect();
                    for (path, is_delete) in ready {
                        pending.remove(&path);
                        if is_delete {
                            let _ = knowlix_indexer::remove_file(&path).await;
                            app_handle_ev.emit("file_removed", &path).ok();
                        } else {
                            let _ = knowlix_indexer::index_file(&path, &project_id_ev).await;
                            app_handle_ev.emit("file_indexed", &path).ok();
                        }
                    }
                }
            }
        }
    });

    // Periodic scan fallback: catches files the OS watcher may miss (e.g. Windows
    // Controlled Folder Access silently swallowing ReadDirectoryChangesW events).
    let project_id_scan = project_id.clone();
    let app_handle_scan = app_handle.clone();
    let scan_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(SCAN_INTERVAL);
        interval.tick().await; // skip immediate first tick — initial index done by reindex_project
        loop {
            interval.tick().await;
            // Skip if a manual reindex is already running
            let in_progress = knowlix_indexer::get_index_status(&project_id_scan)
                .await
                .map(|s| s.in_progress)
                .unwrap_or(true);
            if in_progress {
                continue;
            }
            if let Ok(stats) = knowlix_indexer::reindex_project(&project_id_scan, None).await {
                for r in &stats.file_results {
                    if r.status == IndexFileStatus::Indexed {
                        app_handle_scan.emit("file_indexed", &r.path).ok();
                    }
                }
            }
        }
    });

    watchers.insert(project_id, WatcherEntry {
        _watcher: watcher,
        event_abort: event_task.abort_handle(),
        scan_abort: scan_task.abort_handle(),
    });

    Ok(())
}

#[tauri::command]
pub async fn stop_file_watcher(
    project_id: String,
    state: State<'_, WatcherState>,
) -> Result<(), String> {
    if let Some(entry) = state.0.lock().await.remove(&project_id) {
        entry.event_abort.abort();
        entry.scan_abort.abort();
    }
    Ok(())
}
