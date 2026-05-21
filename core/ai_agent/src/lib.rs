use std::sync::{Mutex, OnceLock};

use knowlix_common::{
    AiAnswer, AiConfig, AiHealthStatus, AiProvider, AiTier, KnowlixError, Result, SearchResult,
    TokenUsage,
};
use serde::Deserialize;

// ---- Config cache ----

static AI_CONFIG: OnceLock<Mutex<AiConfig>> = OnceLock::new();

fn config_lock() -> &'static Mutex<AiConfig> {
    AI_CONFIG.get_or_init(|| Mutex::new(AiConfig::default()))
}

pub fn update_config(config: AiConfig) {
    match config_lock().lock() {
        Ok(mut guard) => *guard = config,
        Err(e) => tracing::error!("AI config lock poisoned: {e}"),
    }
}

fn get_config() -> AiConfig {
    config_lock()
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default()
}

// ---- Public API ----

pub fn get_ai_tier() -> AiTier {
    match get_config().provider {
        AiProvider::None => AiTier::None,
        AiProvider::Ollama => AiTier::Local,
        AiProvider::Api => AiTier::Api,
    }
}

pub async fn health_check() -> Result<AiHealthStatus> {
    let config = get_config();
    match config.provider {
        AiProvider::None => Ok(AiHealthStatus {
            tier: AiTier::None,
            model: None,
            reachable: false,
            error: Some("No AI provider configured".into()),
        }),
        AiProvider::Ollama => {
            let url = config.ollama_url.clone();
            let model = config.ollama_model.clone();
            match ollama_list_models(&url).await {
                Ok(models) => {
                    let resolved = model.clone().or_else(|| models.into_iter().next());
                    Ok(AiHealthStatus {
                        tier: AiTier::Local,
                        model: resolved,
                        reachable: true,
                        error: None,
                    })
                }
                Err(e) => Ok(AiHealthStatus {
                    tier: AiTier::Local,
                    model,
                    reachable: false,
                    error: Some(e.to_string()),
                }),
            }
        }
        AiProvider::Api => {
            let url = normalize_base_url(&config.api_base_url);
            let key = match config.api_key.as_deref() {
                Some(k) if !k.is_empty() => k.to_string(),
                _ => {
                    return Ok(AiHealthStatus {
                        tier: AiTier::Api,
                        model: config.api_model,
                        reachable: false,
                        error: Some("API key not configured".into()),
                    })
                }
            };
            match api_list_models(&url, &key).await {
                Ok(models) => {
                    let resolved = config.api_model.clone().or_else(|| models.into_iter().next());
                    Ok(AiHealthStatus {
                        tier: AiTier::Api,
                        model: resolved,
                        reachable: true,
                        error: None,
                    })
                }
                Err(e) => Ok(AiHealthStatus {
                    tier: AiTier::Api,
                    model: config.api_model,
                    reachable: false,
                    error: Some(e.to_string()),
                }),
            }
        }
    }
}

pub async fn expand_query(query: &str) -> Result<Vec<String>> {
    let config = get_config();
    if config.provider == AiProvider::None {
        return Ok(vec![query.to_string()]);
    }

    let prompt = format!(
        "Generate 3 alternative search queries for: \"{query}\"\nReturn ONLY the queries, one per line, no numbering, no explanation."
    );

    let text = match config.provider {
        AiProvider::Ollama => {
            let model = match &config.ollama_model {
                Some(m) if !m.is_empty() => m.clone(),
                _ => return Ok(vec![query.to_string()]),
            };
            match ollama_generate(&config.ollama_url, &model, &prompt).await {
                Ok((t, _)) => t,
                Err(e) => {
                    tracing::warn!("expand_query (ollama) failed: {e}");
                    return Ok(vec![query.to_string()]);
                }
            }
        }
        AiProvider::Api => {
            let url = normalize_base_url(&config.api_base_url);
            let key = match config.api_key.as_deref() {
                Some(k) if !k.is_empty() => k.to_string(),
                _ => return Ok(vec![query.to_string()]),
            };
            let model = match config.api_model.as_deref() {
                Some(m) if !m.is_empty() => m.to_string(),
                _ => return Ok(vec![query.to_string()]),
            };
            match api_generate(&url, &key, &model, &prompt).await {
                Ok((t, _)) => t,
                Err(e) => {
                    tracing::warn!("expand_query (api) failed: {e}");
                    return Ok(vec![query.to_string()]);
                }
            }
        }
        AiProvider::None => return Ok(vec![query.to_string()]),
    };

    let mut variants: Vec<String> = text
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .take(3)
        .collect();
    if variants.is_empty() {
        variants.push(query.to_string());
    }
    Ok(variants)
}

pub async fn answer_question(query: &str, project_id: Option<&str>) -> Result<AiAnswer> {
    let config = get_config();

    let (model, base_url, api_key) = match &config.provider {
        AiProvider::None => return Err(KnowlixError::AiNotConfigured),
        AiProvider::Ollama => {
            let model = config
                .ollama_model
                .clone()
                .filter(|m| !m.is_empty())
                .ok_or_else(|| KnowlixError::Ai("No Ollama model configured".into()))?;
            (model, config.ollama_url.clone(), None::<String>)
        }
        AiProvider::Api => {
            let key = config
                .api_key
                .clone()
                .filter(|k| !k.is_empty())
                .ok_or_else(|| KnowlixError::Ai("API key not configured".into()))?;
            let model = config
                .api_model
                .clone()
                .filter(|m| !m.is_empty())
                .ok_or_else(|| KnowlixError::Ai("API model not configured".into()))?;
            let url = normalize_base_url(&config.api_base_url);
            (model, url, Some(key))
        }
    };

    // Expand query (graceful fallback)
    let variants = expand_query(query).await.unwrap_or_else(|_| vec![query.to_string()]);

    // Gather unique top-k chunks via hybrid search across all variants
    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut all_results: Vec<SearchResult> = Vec::new();

    for variant in &variants {
        match knowlix_search::search(variant, project_id, 10).await {
            Ok(results) => {
                for r in results {
                    if seen_ids.insert(r.chunk_id.clone()) {
                        all_results.push(r);
                    }
                }
            }
            Err(e) => tracing::warn!("Search variant '{}' failed: {e}", variant),
        }
    }

    all_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    all_results.truncate(8);

    // Build context string
    let context: String = if all_results.is_empty() {
        "(no relevant context found)".to_string()
    } else {
        all_results
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let filename = r.file_path.split('/').last().unwrap_or(&r.file_path);
                format!("[{}] {}\n{}", i + 1, filename, r.snippet)
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    };

    let prompt = format!(
        "Answer using ONLY the provided context. If context insufficient, say so.\n\nQuestion: {query}\n\nContext:\n{context}\n\nAnswer in markdown, cite sources with [N]."
    );

    let (answer, token_usage) = match config.provider {
        AiProvider::Ollama => ollama_generate(&base_url, &model, &prompt).await?,
        AiProvider::Api => {
            let key = api_key.unwrap();
            api_generate(&base_url, &key, &model, &prompt).await?
        }
        AiProvider::None => unreachable!(),
    };

    Ok(AiAnswer {
        answer,
        sources: all_results,
        model,
        query: query.to_string(),
        token_usage,
    })
}

/// Generate text for wiki — used by wiki module.
pub async fn generate_text(prompt: &str) -> Result<String> {
    let config = get_config();
    match config.provider {
        AiProvider::None => Err(KnowlixError::AiNotConfigured),
        AiProvider::Ollama => {
            let model = config
                .ollama_model
                .clone()
                .filter(|m| !m.is_empty())
                .ok_or_else(|| KnowlixError::Ai("No Ollama model configured".into()))?;
            let (text, _) = ollama_generate(&config.ollama_url, &model, prompt).await?;
            Ok(text)
        }
        AiProvider::Api => {
            let url = normalize_base_url(&config.api_base_url);
            let key = config
                .api_key
                .clone()
                .filter(|k| !k.is_empty())
                .ok_or_else(|| KnowlixError::Ai("API key not configured".into()))?;
            let model = config
                .api_model
                .clone()
                .filter(|m| !m.is_empty())
                .ok_or_else(|| KnowlixError::Ai("API model not configured".into()))?;
            let (text, _) = api_generate(&url, &key, &model, prompt).await?;
            Ok(text)
        }
    }
}

// ---- Internal helpers ----

fn normalize_base_url(url: &str) -> String {
    url.trim_end_matches('/').to_string()
}

// ---- Ollama helpers ----

#[derive(Deserialize)]
struct OllamaGenerateResponse {
    response: String,
    eval_count: Option<u32>,
    prompt_eval_count: Option<u32>,
}

#[derive(Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModelEntry>,
}

#[derive(Deserialize)]
struct OllamaModelEntry {
    name: String,
}

async fn ollama_generate(
    url: &str,
    model: &str,
    prompt: &str,
) -> Result<(String, Option<TokenUsage>)> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| KnowlixError::Ai(format!("HTTP client error: {e}")))?;

    let body = serde_json::json!({
        "model": model,
        "prompt": prompt,
        "stream": false,
    });

    let resp = client
        .post(format!("{url}/api/generate"))
        .json(&body)
        .send()
        .await
        .map_err(|e| KnowlixError::AiProviderUnreachable(e.to_string()))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(KnowlixError::Ai(format!("Ollama generate error {status}: {text}")));
    }

    let parsed: OllamaGenerateResponse = resp
        .json()
        .await
        .map_err(|e| KnowlixError::Ai(format!("Ollama response parse error: {e}")))?;

    let token_usage = match (parsed.prompt_eval_count, parsed.eval_count) {
        (Some(p), Some(c)) => Some(TokenUsage {
            prompt_tokens: p,
            completion_tokens: c,
            total_tokens: p + c,
        }),
        _ => None,
    };

    Ok((parsed.response, token_usage))
}

async fn ollama_list_models(url: &str) -> Result<Vec<String>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| KnowlixError::Ai(format!("HTTP client error: {e}")))?;

    let resp = client
        .get(format!("{url}/api/tags"))
        .send()
        .await
        .map_err(|e| KnowlixError::AiProviderUnreachable(e.to_string()))?;

    if !resp.status().is_success() {
        let status = resp.status();
        return Err(KnowlixError::Ai(format!("Ollama /api/tags returned {status}")));
    }

    let parsed: OllamaTagsResponse = resp
        .json()
        .await
        .map_err(|e| KnowlixError::Ai(format!("Ollama tags parse error: {e}")))?;

    Ok(parsed.models.into_iter().map(|m| m.name).collect())
}

// ---- OpenAI-compatible API helpers ----

#[derive(Deserialize)]
struct ApiChatResponse {
    choices: Vec<ApiChatChoice>,
    usage: Option<ApiUsage>,
}

#[derive(Deserialize)]
struct ApiChatChoice {
    message: ApiChatMessage,
}

#[derive(Deserialize)]
struct ApiChatMessage {
    content: String,
}

#[derive(Deserialize)]
struct ApiUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(Deserialize)]
struct ApiModelsResponse {
    data: Vec<ApiModelEntry>,
}

#[derive(Deserialize)]
struct ApiModelEntry {
    id: String,
}

async fn api_generate(
    base_url: &str,
    api_key: &str,
    model: &str,
    prompt: &str,
) -> Result<(String, Option<TokenUsage>)> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| KnowlixError::Ai(format!("HTTP client error: {e}")))?;

    let body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "max_tokens": 2000,
    });

    let resp = client
        .post(format!("{base_url}/chat/completions"))
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| KnowlixError::AiProviderUnreachable(e.to_string()))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(KnowlixError::Ai(format!("API error {status}: {text}")));
    }

    let parsed: ApiChatResponse = resp
        .json()
        .await
        .map_err(|e| KnowlixError::Ai(format!("API response parse error: {e}")))?;

    let content = parsed
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .ok_or_else(|| KnowlixError::Ai("Empty response from API".into()))?;

    let token_usage = parsed.usage.map(|u| TokenUsage {
        prompt_tokens: u.prompt_tokens,
        completion_tokens: u.completion_tokens,
        total_tokens: u.total_tokens,
    });

    Ok((content, token_usage))
}

async fn api_list_models(base_url: &str, api_key: &str) -> Result<Vec<String>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| KnowlixError::Ai(format!("HTTP client error: {e}")))?;

    let resp = client
        .get(format!("{base_url}/models"))
        .header("Authorization", format!("Bearer {api_key}"))
        .send()
        .await
        .map_err(|e| KnowlixError::AiProviderUnreachable(e.to_string()))?;

    if !resp.status().is_success() {
        let status = resp.status();
        return Err(KnowlixError::Ai(format!("API /models returned {status}")));
    }

    let parsed: ApiModelsResponse = resp
        .json()
        .await
        .map_err(|e| KnowlixError::Ai(format!("API models parse error: {e}")))?;

    Ok(parsed.data.into_iter().map(|m| m.id).collect())
}
