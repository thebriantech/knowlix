# Contracts

Module public APIs. Do NOT change signatures without updating this file and all callers.

---

## project module

```rust
// Create new project. Returns error if name empty or duplicate.
create_project(name: String, description: Option<String>) -> Result<Project>

// Add folder to existing project. Path must exist on filesystem.
add_folder(project_id: &str, path: &str) -> Result<()>

// Remove folder from project. Does NOT delete indexed data immediately.
remove_folder(project_id: &str, path: &str) -> Result<()>

// List all projects, sorted by updated_at desc.
list_projects() -> Result<Vec<Project>>

// Get single project by id.
get_project(project_id: &str) -> Result<Option<Project>>

// Delete project and all associated data (files, chunks, wiki pages).
delete_project(project_id: &str) -> Result<()>

// Rename project.
update_project(project_id: &str, name: String, description: Option<String>) -> Result<Project>
```

---

## watcher module

```rust
// Start watching all folders for a project. Emits FileEvent on changes.
watch_project(project_id: &str, folders: Vec<String>, tx: Sender<FileEvent>) -> Result<WatcherHandle>

// Stop watcher. Called when project deleted or folder removed.
unwatch(handle: WatcherHandle) -> Result<()>
```

FileEvent:
```rust
enum FileEvent {
    Created(path: String),
    Modified(path: String),
    Deleted(path: String),
}
```

---

## indexer module

```rust
// Index a single file. Extracts text, chunks, generates embeddings.
// Skips if SHA256 unchanged since last index.
// Non-blocking: spawns tokio task, returns immediately.
index_file(file_path: &str, project_id: &str) -> Result<()>

// Remove all index data for a file.
remove_file(file_path: &str) -> Result<()>

// Extract raw text from file (by type). Returns error for unsupported types.
extract_text(file_path: &str) -> Result<String>

// Re-index all files in a project. Skips unchanged files (SHA256 check).
reindex_project(project_id: &str) -> Result<IndexStats>

// Get indexing progress for a project.
get_index_status(project_id: &str) -> Result<IndexStatus>
```

IndexStats:
```rust
struct IndexStats {
    total_files: usize,
    indexed: usize,
    skipped: usize,      // unchanged
    failed: usize,
    duration_ms: u64,
}
```

IndexStatus:
```rust
struct IndexStatus {
    total_files: usize,
    indexed_files: usize,
    in_progress: bool,
}
```

---

## search module

```rust
// Hybrid search: BM25 + vector similarity, merged via RRF.
// project_id = None searches across all projects.
search(
    query: &str,
    project_id: Option<&str>,
    limit: usize,
) -> Result<Vec<SearchResult>>

// BM25 only (faster, no embeddings).
search_keyword(
    query: &str,
    project_id: Option<&str>,
    limit: usize,
) -> Result<Vec<SearchResult>>

// Vector similarity only.
search_semantic(
    query: &str,
    project_id: Option<&str>,
    limit: usize,
) -> Result<Vec<SearchResult>>
```

SearchResult — see DATA_MODEL.md.

---

## storage module

```rust
// Project CRUD — called by project module, not directly from UI.
insert_project(project: &Project) -> Result<()>
get_project(id: &str) -> Result<Option<Project>>
list_projects() -> Result<Vec<Project>>
update_project(project: &Project) -> Result<()>
delete_project(id: &str) -> Result<()>

// FileEntry CRUD
upsert_file_entry(entry: &FileEntry) -> Result<()>
get_file_entry(path: &str) -> Result<Option<FileEntry>>
list_files_for_project(project_id: &str) -> Result<Vec<FileEntry>>
delete_file_entry(path: &str) -> Result<()>

// Chunks
insert_chunks(chunks: Vec<Chunk>) -> Result<()>
get_chunks_for_file(file_id: &str) -> Result<Vec<Chunk>>
delete_chunks_for_file(file_id: &str) -> Result<()>

// Wiki pages
upsert_wiki_page(page: &WikiPage) -> Result<()>
get_wiki_pages_for_project(project_id: &str) -> Result<Vec<WikiPage>>
get_global_wiki_pages() -> Result<Vec<WikiPage>>
delete_wiki_pages_for_project(project_id: &str) -> Result<()>

// Config
get_ai_config() -> Result<AiConfig>
save_ai_config(config: &AiConfig) -> Result<()>
```

---

## viewer module

```rust
// Return file content ready for frontend rendering.
// For code: includes detected language for syntax highlighting.
// For PDF/DOCX: returns extracted HTML.
// For images: returns base64 data URI.
get_view_content(file_path: &str) -> Result<ViewContent>
```

ViewContent:
```rust
enum ViewContent {
    Code { content: String, language: String },
    Markdown { content: String },
    Html { content: String },        // PDF/DOCX converted to HTML
    Image { data_uri: String, mime: String },
    PlainText { content: String },
}
```

---

## wiki module

```rust
// Generate wiki for a project. Requires AI tier >= 1.
// Long-running: runs in background, progress via callback.
generate_project_wiki(
    project_id: &str,
    force_regenerate: bool,
    progress_tx: Sender<WikiProgress>,
) -> Result<()>

// Generate global wiki from all project wikis. Requires AI tier >= 1.
generate_global_wiki(
    force_regenerate: bool,
    progress_tx: Sender<WikiProgress>,
) -> Result<()>

// Get generated wiki pages.
get_project_wiki(project_id: &str) -> Result<Vec<WikiPage>>
get_global_wiki() -> Result<Vec<WikiPage>>
```

WikiProgress:
```rust
struct WikiProgress {
    stage: String,         // "extracting" | "generating" | "saving"
    current: usize,
    total: usize,
}
```

---

## ai_agent module

```rust
// Check current AI tier based on config.
get_ai_tier() -> AiTier   // None | Local | Api

// Answer a natural language question using RAG.
// Retrieves relevant chunks, sends to LLM, returns structured answer.
// Returns error if ai_tier == None.
answer_question(
    query: &str,
    project_id: Option<&str>,
) -> Result<AiAnswer>

// Expand query into variants for improved recall.
// Returns original query if ai_tier == None (graceful degradation).
expand_query(query: &str) -> Result<Vec<String>>

// Test provider connectivity.
health_check() -> Result<AiHealthStatus>
```

AiTier enum:
```rust
enum AiTier {
    None,
    Local,   // Ollama
    Api,     // external API key
}
```

AiHealthStatus:
```rust
struct AiHealthStatus {
    tier: AiTier,
    model: Option<String>,
    reachable: bool,
    error: Option<String>,
}
```
