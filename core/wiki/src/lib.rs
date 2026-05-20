use chrono::Utc;
use sha2::{Digest, Sha256};
use tokio::sync::mpsc::Sender;
use uuid::Uuid;

use knowlix_common::{KnowlixError, Result, WikiPage, WikiProgress};

const BATCH_CHARS: usize = 12_000;

pub async fn generate_project_wiki(
    project_id: &str,
    force_regenerate: bool,
    progress_tx: Sender<WikiProgress>,
) -> Result<()> {
    if knowlix_ai_agent::get_ai_tier() == knowlix_common::AiTier::None {
        return Err(KnowlixError::AiNotConfigured);
    }

    let _ = progress_tx
        .send(WikiProgress {
            stage: "extracting".into(),
            current: 0,
            total: 0,
        })
        .await;

    // Gather all chunks for indexed files
    let files = knowlix_storage::list_files_for_project(project_id).await?;
    let indexed_files: Vec<_> = files.into_iter().filter(|f| f.indexed).collect();

    let mut all_chunks: Vec<knowlix_common::Chunk> = Vec::new();
    for file in &indexed_files {
        match knowlix_storage::get_chunks_for_file(&file.id).await {
            Ok(chunks) => all_chunks.extend(chunks),
            Err(e) => tracing::warn!("Failed to get chunks for file {}: {e}", file.id),
        }
    }

    if all_chunks.is_empty() {
        return Err(KnowlixError::Ai(
            "No indexed content found for this project. Index the project first.".into(),
        ));
    }

    // Compute SHA256 of all combined content for cache invalidation
    let combined_content: String = all_chunks.iter().map(|c| c.content.as_str()).collect();
    let mut hasher = Sha256::new();
    hasher.update(combined_content.as_bytes());
    let content_hash = format!("{:x}", hasher.finalize());

    // Check if existing wiki is up-to-date
    if !force_regenerate {
        let existing = knowlix_storage::get_wiki_pages_for_project(project_id).await?;
        if !existing.is_empty() {
            let already_current = existing
                .iter()
                .any(|p| p.source_hashes.contains(&content_hash));
            if already_current {
                tracing::info!("Wiki for project {project_id} is up-to-date, skipping generation");
                return Ok(());
            }
        }
    }

    // Delete existing wiki pages for this project
    knowlix_storage::delete_wiki_pages_for_project(project_id).await?;

    // Split chunks into batches of BATCH_CHARS
    let mut batches: Vec<String> = Vec::new();
    let mut current_batch = String::new();

    for chunk in &all_chunks {
        if !current_batch.is_empty()
            && current_batch.len() + chunk.content.len() > BATCH_CHARS
        {
            batches.push(current_batch.clone());
            current_batch.clear();
        }
        if !current_batch.is_empty() {
            current_batch.push_str("\n\n---\n\n");
        }
        current_batch.push_str(&chunk.content);
    }
    if !current_batch.is_empty() {
        batches.push(current_batch);
    }

    let n_batches = batches.len();

    for (i, batch_context) in batches.into_iter().enumerate() {
        let _ = progress_tx
            .send(WikiProgress {
                stage: "generating".into(),
                current: i + 1,
                total: n_batches,
            })
            .await;

        let prompt = format!(
            "You are a technical documentation writer. Based on these source file excerpts, write a wiki page in markdown.\n\nSources:\n{batch_context}\n\nWrite with # Title, Overview, Key Concepts sections. Start with # Title."
        );

        let content = match knowlix_ai_agent::generate_text(&prompt).await {
            Ok(text) => text,
            Err(e) => {
                tracing::error!("Wiki generation failed for batch {i}: {e}");
                return Err(e);
            }
        };

        // Parse first line for title
        let title = content
            .lines()
            .next()
            .map(|l| l.trim_start_matches('#').trim().to_string())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| format!("Wiki Page {}", i + 1));

        let now = Utc::now();
        let page = WikiPage {
            id: Uuid::new_v4().to_string(),
            project_id: Some(project_id.to_string()),
            title,
            content,
            tags: vec![],
            source_hashes: vec![content_hash.clone()],
            created_at: now,
            updated_at: now,
        };

        knowlix_storage::upsert_wiki_page(&page).await?;
    }

    let _ = progress_tx
        .send(WikiProgress {
            stage: "saving".into(),
            current: n_batches,
            total: n_batches,
        })
        .await;

    Ok(())
}

pub async fn generate_global_wiki(
    force_regenerate: bool,
    progress_tx: Sender<WikiProgress>,
) -> Result<()> {
    if knowlix_ai_agent::get_ai_tier() == knowlix_common::AiTier::None {
        return Err(KnowlixError::AiNotConfigured);
    }

    let _ = progress_tx
        .send(WikiProgress {
            stage: "extracting".into(),
            current: 0,
            total: 0,
        })
        .await;

    // Gather all projects and their wiki pages
    let projects = knowlix_storage::list_projects().await?;
    let mut project_wikis: Vec<(String, Vec<WikiPage>)> = Vec::new();

    for project in &projects {
        let pages = knowlix_storage::get_wiki_pages_for_project(&project.id).await?;
        if !pages.is_empty() {
            project_wikis.push((project.name.clone(), pages));
        }
    }

    if project_wikis.is_empty() {
        return Err(KnowlixError::Ai(
            "No project wikis found. Generate project wikis first.".into(),
        ));
    }

    // Compute combined hash for cache check
    let combined_for_hash: String = project_wikis
        .iter()
        .flat_map(|(_, pages)| pages.iter().map(|p| p.content.as_str()))
        .collect();
    let mut hasher = Sha256::new();
    hasher.update(combined_for_hash.as_bytes());
    let content_hash = format!("{:x}", hasher.finalize());

    // Check if existing global wiki is up-to-date
    if !force_regenerate {
        let existing = knowlix_storage::get_global_wiki_pages().await?;
        if !existing.is_empty() {
            let already_current = existing
                .iter()
                .any(|p| p.source_hashes.contains(&content_hash));
            if already_current {
                tracing::info!("Global wiki is up-to-date, skipping generation");
                return Ok(());
            }
        }
    }

    let _ = progress_tx
        .send(WikiProgress {
            stage: "generating".into(),
            current: 0,
            total: 1,
        })
        .await;

    // Build context — up to 4000 chars per project
    const MAX_CHARS_PER_PROJECT: usize = 4_000;
    let context: String = project_wikis
        .iter()
        .map(|(name, pages)| {
            let pages_text: String = pages
                .iter()
                .map(|p| format!("## {}\n{}", p.title, p.content))
                .collect::<Vec<_>>()
                .join("\n\n");

            let truncated = if pages_text.len() > MAX_CHARS_PER_PROJECT {
                let mut end = MAX_CHARS_PER_PROJECT;
                while end > 0 && !pages_text.is_char_boundary(end) {
                    end -= 1;
                }
                format!("{}…", &pages_text[..end])
            } else {
                pages_text
            };

            format!("# Project: {name}\n{truncated}")
        })
        .collect::<Vec<_>>()
        .join("\n\n---\n\n");

    let prompt = format!(
        "Synthesize these project wikis into a global overview.\n\n{context}\n\nWrite # Global Knowledge Base, then: Executive Summary, Projects Overview, Cross-Project Patterns. Start with # Global Knowledge Base."
    );

    let content = knowlix_ai_agent::generate_text(&prompt).await?;

    let title = content
        .lines()
        .next()
        .map(|l| l.trim_start_matches('#').trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "Global Knowledge Base".to_string());

    let now = Utc::now();
    // Fixed id so ON CONFLICT updates it
    let page = WikiPage {
        id: "global-wiki-1".to_string(),
        project_id: None,
        title,
        content,
        tags: vec![],
        source_hashes: vec![content_hash],
        created_at: now,
        updated_at: now,
    };

    knowlix_storage::upsert_wiki_page(&page).await?;

    let _ = progress_tx
        .send(WikiProgress {
            stage: "saving".into(),
            current: 1,
            total: 1,
        })
        .await;

    Ok(())
}

pub async fn get_project_wiki(project_id: &str) -> Result<Vec<WikiPage>> {
    knowlix_storage::get_wiki_pages_for_project(project_id).await
}

pub async fn get_global_wiki() -> Result<Vec<WikiPage>> {
    knowlix_storage::get_global_wiki_pages().await
}
