use std::sync::{Mutex, OnceLock};

use knowlix_common::{AiAnswer, AiConfig, AiHealthStatus, AiProvider, AiTier, KnowlixError, Result, SearchResult};
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
                    let resolved = model
                        .clone()
                        .or_else(|| models.into_iter().next());
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
        AiProvider::Api => Ok(AiHealthStatus {
            tier: AiTier::Api,
            model: config.api_model,
            reachable: false,
            error: Some("API provider not yet implemented (Phase 6)".into()),
        }),
    }
}

pub async fn expand_query(query: &str) -> Result<Vec<String>> {
    let config = get_config();
    if config.provider == AiProvider::None {
        return Ok(vec![query.to_string()]);
    }
    if config.provider != AiProvider::Ollama {
        return Ok(vec![query.to_string()]);
    }

    let model = match &config.ollama_model {
        Some(m) if !m.is_empty() => m.clone(),
        _ => return Ok(vec![query.to_string()]),
    };

    let prompt = format!(
        "Generate 3 alternative search queries for: \"{query}\"\nReturn ONLY the queries, one per line, no numbering, no explanation."
    );

    match ollama_generate(&config.ollama_url, &model, &prompt).await {
        Ok(text) => {
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
        Err(e) => {
            tracing::warn!("expand_query failed, using original: {e}");
            Ok(vec![query.to_string()])
        }
    }
}

pub async fn answer_question(query: &str, project_id: Option<&str>) -> Result<AiAnswer> {
    let config = get_config();
    if config.provider == AiProvider::None {
        return Err(KnowlixError::AiNotConfigured);
    }
    if config.provider != AiProvider::Ollama {
        return Err(KnowlixError::Ai("API provider not yet implemented (Phase 6)".into()));
    }
    let model = config
        .ollama_model
        .clone()
        .filter(|m| !m.is_empty())
        .ok_or_else(|| KnowlixError::Ai("No Ollama model configured".into()))?;

    // Expand query to variants (graceful: fall back to original)
    let variants = expand_query(query).await.unwrap_or_else(|_| vec![query.to_string()]);

    // Search hybrid for each variant, collect unique chunks by chunk_id, top 8 by score
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

    // Sort by score desc, keep top 8
    all_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    all_results.truncate(8);

    if all_results.is_empty() {
        // No context found — still ask the model
        let prompt = format!(
            "Answer using ONLY the provided context. If context insufficient, say so.\n\nQuestion: {query}\n\nContext:\n(no relevant context found)\n\nAnswer in markdown, cite sources with [N]."
        );
        let answer = ollama_generate(&config.ollama_url, &model, &prompt).await?;
        return Ok(AiAnswer {
            answer,
            sources: vec![],
            model,
            query: query.to_string(),
        });
    }

    // Build context string
    let context: String = all_results
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let filename = r.file_path.split('/').last()
                .unwrap_or(&r.file_path);
            format!("[{}] {}\n{}", i + 1, filename, r.snippet)
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let prompt = format!(
        "Answer using ONLY the provided context. If context insufficient, say so.\n\nQuestion: {query}\n\nContext:\n{context}\n\nAnswer in markdown, cite sources with [N]."
    );

    let answer = ollama_generate(&config.ollama_url, &model, &prompt).await?;

    Ok(AiAnswer {
        answer,
        sources: all_results,
        model,
        query: query.to_string(),
    })
}

/// Public: generate text via Ollama. Used by wiki module.
pub async fn generate_text(prompt: &str) -> Result<String> {
    let config = get_config();
    if config.provider == AiProvider::None {
        return Err(KnowlixError::AiNotConfigured);
    }
    if config.provider != AiProvider::Ollama {
        return Err(KnowlixError::Ai("API provider not yet implemented (Phase 6)".into()));
    }
    let model = config
        .ollama_model
        .clone()
        .filter(|m| !m.is_empty())
        .ok_or_else(|| KnowlixError::Ai("No Ollama model configured".into()))?;

    ollama_generate(&config.ollama_url, &model, prompt).await
}

// ---- Internal helpers ----

#[derive(Deserialize)]
struct OllamaGenerateResponse {
    response: String,
}

#[derive(Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModelEntry>,
}

#[derive(Deserialize)]
struct OllamaModelEntry {
    name: String,
}

async fn ollama_generate(url: &str, model: &str, prompt: &str) -> Result<String> {
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

    Ok(parsed.response)
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
