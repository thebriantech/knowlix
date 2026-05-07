use knowlix_common::Result;
use tokio::sync::mpsc::Sender;

#[derive(Debug)]
pub enum FileEvent {
    Created(String),
    Modified(String),
    Deleted(String),
}

pub struct WatcherHandle {
    _inner: (),
}

pub fn watch_project(
    _project_id: &str,
    _folders: Vec<String>,
    _tx: Sender<FileEvent>,
) -> Result<WatcherHandle> {
    Err(knowlix_common::KnowlixError::Validation(
        "File watcher available in Phase 2".into(),
    ))
}

pub fn unwatch(_handle: WatcherHandle) -> Result<()> {
    Ok(())
}
