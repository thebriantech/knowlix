use std::path::Path;
use std::time::Duration;

use knowlix_common::Result;
use notify::{
    Config, Event, EventKind, PollWatcher, RecursiveMode, Watcher,
    event::{ModifyKind, RenameMode},
};
use tokio::sync::mpsc::Sender;

#[derive(Debug)]
pub enum FileEvent {
    Created(String),
    Modified(String),
    Deleted(String),
}

pub struct WatcherHandle {
    _watcher: PollWatcher,
}

pub fn watch_project(
    _project_id: &str,
    folders: Vec<String>,
    tx: Sender<FileEvent>,
) -> Result<WatcherHandle> {
    // Use PollWatcher — RecommendedWatcher (ReadDirectoryChangesW) silently fails
    // on Windows when Controlled Folder Access is enabled for watched directories.
    let config = Config::default()
        .with_poll_interval(Duration::from_secs(2))
        .with_compare_contents(false);

    let mut watcher = PollWatcher::new(
        move |res: notify::Result<Event>| {
            let Ok(event) = res else { return };
            tracing::info!("[notify] poll event kind={:?} paths={:?}", event.kind, event.paths);
            let file_event = match event.kind {
                EventKind::Create(_) => event
                    .paths
                    .first()
                    .map(|p| FileEvent::Created(p.to_string_lossy().into_owned())),

                EventKind::Modify(ModifyKind::Name(rename_mode)) => match rename_mode {
                    RenameMode::Both => event
                        .paths
                        .get(1)
                        .map(|p| FileEvent::Modified(p.to_string_lossy().into_owned())),
                    RenameMode::To => event
                        .paths
                        .first()
                        .map(|p| FileEvent::Modified(p.to_string_lossy().into_owned())),
                    RenameMode::From => event
                        .paths
                        .first()
                        .map(|p| FileEvent::Deleted(p.to_string_lossy().into_owned())),
                    _ => None,
                },

                EventKind::Modify(_) => event
                    .paths
                    .first()
                    .map(|p| FileEvent::Modified(p.to_string_lossy().into_owned())),
                EventKind::Remove(_) => event
                    .paths
                    .first()
                    .map(|p| FileEvent::Deleted(p.to_string_lossy().into_owned())),
                _ => None,
            };
            if let Some(ev) = file_event {
                if let Err(e) = tx.try_send(ev) {
                    tracing::warn!("Watcher channel full, dropping event: {e}");
                }
            }
        },
        config,
    )
    .map_err(|e| knowlix_common::KnowlixError::Validation(e.to_string()))?;

    for folder in &folders {
        if Path::new(folder).is_dir() {
            watcher
                .watch(Path::new(folder), RecursiveMode::Recursive)
                .map_err(|e| knowlix_common::KnowlixError::Validation(e.to_string()))?;
        }
    }

    Ok(WatcherHandle { _watcher: watcher })
}

pub fn unwatch(_handle: WatcherHandle) -> Result<()> {
    Ok(())
}
