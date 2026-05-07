use knowlix_common::{KnowlixError, Result, WikiPage, WikiProgress};
use tokio::sync::mpsc::Sender;

pub async fn generate_project_wiki(
    _project_id: &str,
    _force_regenerate: bool,
    _progress_tx: Sender<WikiProgress>,
) -> Result<()> {
    Err(KnowlixError::Ai("Wiki generation available in Phase 5".into()))
}

pub async fn generate_global_wiki(
    _force_regenerate: bool,
    _progress_tx: Sender<WikiProgress>,
) -> Result<()> {
    Err(KnowlixError::Ai("Wiki generation available in Phase 5".into()))
}

pub async fn get_project_wiki(_project_id: &str) -> Result<Vec<WikiPage>> {
    Ok(vec![])
}

pub async fn get_global_wiki() -> Result<Vec<WikiPage>> {
    Ok(vec![])
}
