use knowlix_common::{AiAnswer, AiHealthStatus, AiTier, KnowlixError, Result};

pub fn get_ai_tier() -> AiTier {
    AiTier::None
}

pub async fn answer_question(
    _query: &str,
    _project_id: Option<&str>,
) -> Result<AiAnswer> {
    Err(KnowlixError::AiNotConfigured)
}

pub async fn expand_query(query: &str) -> Result<Vec<String>> {
    Ok(vec![query.to_string()])
}

pub async fn health_check() -> Result<AiHealthStatus> {
    Ok(AiHealthStatus {
        tier: AiTier::None,
        model: None,
        reachable: false,
        error: Some("No AI provider configured".into()),
    })
}
