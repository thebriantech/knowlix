use knowlix_common::{AiAnswer, AiConfig, AiHealthStatus, AiTier};

#[tauri::command]
pub async fn answer_question(
    query: String,
    project_id: Option<String>,
) -> Result<AiAnswer, String> {
    knowlix_ai_agent::answer_question(&query, project_id.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_ai_tier() -> AiTier {
    knowlix_ai_agent::get_ai_tier()
}

#[tauri::command]
pub async fn health_check() -> Result<AiHealthStatus, String> {
    knowlix_ai_agent::health_check()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_ai_config() -> Result<AiConfig, String> {
    knowlix_storage::get_ai_config()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_ai_config(config: AiConfig) -> Result<(), String> {
    knowlix_storage::save_ai_config(&config)
        .await
        .map_err(|e| e.to_string())?;
    knowlix_ai_agent::update_config(config);
    Ok(())
}
