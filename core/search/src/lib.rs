use knowlix_common::{KnowlixError, Result, SearchResult, SearchSource};

pub async fn search(
    query: &str,
    project_id: Option<&str>,
    limit: usize,
) -> Result<Vec<SearchResult>> {
    // Phase 1: keyword only. Phase 3 adds hybrid.
    search_keyword(query, project_id, limit).await
}

pub async fn search_keyword(
    query: &str,
    project_id: Option<&str>,
    limit: usize,
) -> Result<Vec<SearchResult>> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }

    let hits = knowlix_storage::search_keyword_fts(query, project_id, limit)
        .map_err(|e| KnowlixError::Index(e.to_string()))?;

    let results = hits
        .into_iter()
        .enumerate()
        .map(|(rank, hit)| SearchResult {
            file_id: hit.file_id,
            file_path: hit.file_path,
            chunk_id: hit.chunk_id,
            snippet: hit.snippet,
            score: hit.score,
            rank_bm25: Some(rank as i32 + 1),
            rank_vec: None,
            source: SearchSource::Bm25,
        })
        .collect();

    Ok(results)
}

pub async fn search_semantic(
    query: &str,
    project_id: Option<&str>,
    limit: usize,
) -> Result<Vec<SearchResult>> {
    // Phase 3 — fall back to keyword for now
    search_keyword(query, project_id, limit).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    use tempfile::{NamedTempFile, TempDir};

    static TEMP: std::sync::OnceLock<TempDir> = std::sync::OnceLock::new();
    static LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();

    async fn setup() -> tokio::sync::MutexGuard<'static, ()> {
        let lock = LOCK.get_or_init(|| tokio::sync::Mutex::new(()));
        let guard = lock.lock().await;
        let dir = TEMP.get_or_init(|| TempDir::new().unwrap());
        knowlix_storage::init_with_dir(dir.path().to_path_buf())
            .await
            .unwrap();
        guard
    }

    async fn index_temp_file(content: &str, project_id: &str) -> NamedTempFile {
        // NamedTempFile keeps a handle; write content and index it
        let mut f = NamedTempFile::with_suffix(".txt").unwrap();
        write!(f, "{}", content).unwrap();
        f.flush().unwrap();
        let path = f.path().to_str().unwrap().to_string();
        knowlix_indexer::index_file(&path, project_id)
            .await
            .unwrap();
        f
    }

    #[tokio::test]
    async fn test_search_empty_query() {
        let _guard = setup().await;
        let results = search_keyword("", None, 10).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_search_finds_indexed_content() {
        let _guard = setup().await;

        // Create a project first
        let proj_id = uuid::Uuid::new_v4().to_string();
        let project = knowlix_common::Project {
            id: proj_id.clone(),
            name: format!("SearchTest-{}", &proj_id[..8]),
            description: None,
            folders: vec![],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        knowlix_storage::insert_project(&project).await.unwrap();

        let _f = index_temp_file("authentication token expiry mechanism", &proj_id).await;

        let results = search_keyword("authentication", Some(&proj_id), 10)
            .await
            .unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].source, SearchSource::Bm25);
        assert!(results[0].rank_bm25.is_some());
    }

    #[tokio::test]
    async fn test_search_no_results_for_unindexed() {
        let _guard = setup().await;
        let results = search_keyword("xyzzy_nonexistent_token_9999", None, 10)
            .await
            .unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_result_fields() {
        let r = SearchResult {
            file_id: "fid".into(),
            file_path: "/tmp/f.txt".into(),
            chunk_id: "cid".into(),
            snippet: "hello".into(),
            score: 0.9,
            rank_bm25: Some(1),
            rank_vec: None,
            source: SearchSource::Bm25,
        };
        assert_eq!(r.rank_bm25, Some(1));
        assert!(r.rank_vec.is_none());
    }
}
