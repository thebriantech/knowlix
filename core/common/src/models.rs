use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub folders: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FileType {
    Text,
    Markdown,
    Code,
    Pdf,
    Word,
    Excel,
    Image,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub id: String,
    pub project_id: String,
    pub path: String,
    pub file_type: FileType,
    pub language: Option<String>,
    pub size_bytes: i64,
    pub content_hash: String,
    pub last_indexed: DateTime<Utc>,
    pub indexed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id: String,
    pub file_id: String,
    pub chunk_index: i32,
    pub content: String,
    pub token_count: i32,
    pub start_byte: i64,
    pub end_byte: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiPage {
    pub id: String,
    pub project_id: Option<String>,
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
    pub source_hashes: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AiProvider {
    None,
    Ollama,
    Api,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    pub provider: AiProvider,
    pub ollama_model: Option<String>,
    pub ollama_url: String,
    pub api_key: Option<String>,
    pub api_base_url: String,
    pub api_model: Option<String>,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: AiProvider::None,
            ollama_model: Some("phi3.5".to_string()),
            ollama_url: "http://localhost:11434".to_string(),
            api_key: None,
            api_base_url: "https://api.openai.com/v1".to_string(),
            api_model: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AiTier {
    None,
    Local,
    Api,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SearchSource {
    Bm25,
    Vector,
    Hybrid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub file_id: String,
    pub file_path: String,
    pub chunk_id: String,
    pub snippet: String,
    pub score: f32,
    pub rank_bm25: Option<i32>,
    pub rank_vec: Option<i32>,
    pub source: SearchSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiAnswer {
    pub answer: String,
    pub sources: Vec<SearchResult>,
    pub model: String,
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum IndexFileStatus {
    Indexed,
    Skipped,
    Failed,
    Removed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexFileResult {
    pub path: String,
    pub status: IndexFileStatus,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStats {
    pub total_files: usize,
    pub indexed: usize,
    pub skipped: usize,
    pub failed: usize,
    pub removed: usize,
    pub duration_ms: u64,
    pub file_results: Vec<IndexFileResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexProgress {
    pub current: usize,
    pub total: usize,
    pub current_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStatus {
    pub total_files: usize,
    pub indexed_files: usize,
    pub in_progress: bool,
    pub last_indexed: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiProgress {
    pub stage: String,
    pub current: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiHealthStatus {
    pub tier: AiTier,
    pub model: Option<String>,
    pub reachable: bool,
    pub error: Option<String>,
}
