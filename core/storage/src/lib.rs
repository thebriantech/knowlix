use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use chrono::Utc;
use knowlix_common::{
    AiConfig, AiProvider, Chunk, FileEntry, FileType, KnowlixError, Project, Result, WikiPage,
};
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use tantivy::{
    collector::TopDocs,
    doc,
    query::{BooleanQuery, Occur, QueryParser, TermQuery},
    schema::{Field, IndexRecordOption, Schema, Value, STORED, STRING, TEXT},
    Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument, Term,
};

struct TantivyFields {
    chunk_id: Field,
    file_id: Field,
    project_id: Field,
    file_path: Field,
    content: Field,
}

struct StorageState {
    db: SqlitePool,
    index: Index,
    writer: Mutex<IndexWriter>,
    reader: IndexReader,
    fields: TantivyFields,
}

// std::sync::OnceLock is safe across multiple tokio runtimes (no runtime dependency).
static STATE: OnceLock<StorageState> = OnceLock::new();

fn get_state() -> Result<&'static StorageState> {
    STATE
        .get()
        .ok_or_else(|| KnowlixError::Storage("Storage not initialized. Call init() first.".into()))
}

fn get_data_dir() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("KNOWLIX_DATA_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| KnowlixError::Storage("Cannot determine home directory".into()))?;
    Ok(PathBuf::from(home).join(".knowlix"))
}

pub async fn init() -> Result<()> {
    let data_dir = get_data_dir()?;
    init_with_dir(data_dir).await
}

pub async fn init_with_dir(data_dir: PathBuf) -> Result<()> {
    if STATE.get().is_some() {
        return Ok(());
    }
    let state = build_state(data_dir).await?;
    // OnceLock::set fails silently if already set by another concurrent caller — that's fine.
    let _ = STATE.set(state);
    Ok(())
}

async fn build_state(data_dir: PathBuf) -> Result<StorageState> {
    std::fs::create_dir_all(&data_dir)?;

    let db_path = data_dir.join("projects.db");
    let options = SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(true);
    let db = SqlitePool::connect_with(options)
        .await
        .map_err(|e| KnowlixError::Storage(e.to_string()))?;

    run_migrations(&db).await?;

    let tantivy_dir = data_dir.join("index").join("tantivy");
    std::fs::create_dir_all(&tantivy_dir)?;

    let mut schema_builder = Schema::builder();
    let f_chunk_id = schema_builder.add_text_field("chunk_id", STRING | STORED);
    let f_file_id = schema_builder.add_text_field("file_id", STRING | STORED);
    let f_project_id = schema_builder.add_text_field("project_id", STRING | STORED);
    let f_file_path = schema_builder.add_text_field("file_path", STORED);
    let f_content = schema_builder.add_text_field("content", TEXT | STORED);
    let schema = schema_builder.build();

    let mmap_dir = tantivy::directory::MmapDirectory::open(&tantivy_dir)
        .map_err(|e| KnowlixError::Storage(format!("Tantivy dir error: {e}")))?;
    let index = Index::open_or_create(mmap_dir, schema)
        .map_err(|e| KnowlixError::Storage(format!("Tantivy index error: {e}")))?;
    let writer = index
        .writer(50_000_000)
        .map_err(|e| KnowlixError::Storage(format!("Tantivy writer error: {e}")))?;
    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::OnCommitWithDelay)
        .try_into()
        .map_err(|e| KnowlixError::Storage(format!("Tantivy reader error: {e}")))?;

    Ok(StorageState {
        db,
        index,
        writer: Mutex::new(writer),
        reader,
        fields: TantivyFields {
            chunk_id: f_chunk_id,
            file_id: f_file_id,
            project_id: f_project_id,
            file_path: f_file_path,
            content: f_content,
        },
    })
}

async fn run_migrations(db: &SqlitePool) -> Result<()> {
    sqlx::query("PRAGMA journal_mode=WAL")
        .execute(db)
        .await
        .map_err(|e| KnowlixError::Storage(e.to_string()))?;
    sqlx::query("PRAGMA foreign_keys=ON")
        .execute(db)
        .await
        .map_err(|e| KnowlixError::Storage(e.to_string()))?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS projects (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            description TEXT,
            folders TEXT NOT NULL DEFAULT '[]',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
    )
    .execute(db)
    .await
    .map_err(|e| KnowlixError::Storage(e.to_string()))?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS file_entries (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            path TEXT NOT NULL UNIQUE,
            file_type TEXT NOT NULL,
            language TEXT,
            size_bytes INTEGER NOT NULL,
            content_hash TEXT NOT NULL,
            last_indexed TEXT NOT NULL,
            indexed INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
        )",
    )
    .execute(db)
    .await
    .map_err(|e| KnowlixError::Storage(e.to_string()))?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS chunks (
            id TEXT PRIMARY KEY,
            file_id TEXT NOT NULL,
            chunk_index INTEGER NOT NULL,
            content TEXT NOT NULL,
            token_count INTEGER NOT NULL,
            start_byte INTEGER NOT NULL,
            end_byte INTEGER NOT NULL,
            FOREIGN KEY (file_id) REFERENCES file_entries(id) ON DELETE CASCADE
        )",
    )
    .execute(db)
    .await
    .map_err(|e| KnowlixError::Storage(e.to_string()))?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS wiki_pages (
            id TEXT PRIMARY KEY,
            project_id TEXT,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            tags TEXT NOT NULL DEFAULT '[]',
            source_hashes TEXT NOT NULL DEFAULT '[]',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
    )
    .execute(db)
    .await
    .map_err(|e| KnowlixError::Storage(e.to_string()))?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS ai_config (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            provider TEXT NOT NULL DEFAULT 'none',
            ollama_model TEXT,
            ollama_url TEXT NOT NULL DEFAULT 'http://localhost:11434',
            api_key TEXT,
            api_base_url TEXT NOT NULL DEFAULT 'https://api.openai.com/v1',
            api_model TEXT
        )",
    )
    .execute(db)
    .await
    .map_err(|e| KnowlixError::Storage(e.to_string()))?;

    Ok(())
}

// ---- Project CRUD ----

pub async fn insert_project(project: &Project) -> Result<()> {
    let s = get_state()?;
    let folders = serde_json::to_string(&project.folders)
        .map_err(|e| KnowlixError::Storage(e.to_string()))?;
    sqlx::query(
        "INSERT INTO projects (id, name, description, folders, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&project.id)
    .bind(&project.name)
    .bind(&project.description)
    .bind(&folders)
    .bind(project.created_at.to_rfc3339())
    .bind(project.updated_at.to_rfc3339())
    .execute(&s.db)
    .await
    .map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            KnowlixError::Duplicate(format!("Project '{}' already exists", project.name))
        } else {
            KnowlixError::Storage(e.to_string())
        }
    })?;
    Ok(())
}

pub async fn get_project(id: &str) -> Result<Option<Project>> {
    let s = get_state()?;
    let row = sqlx::query_as::<_, ProjectRow>(
        "SELECT id, name, description, folders, created_at, updated_at FROM projects WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&s.db)
    .await
    .map_err(|e| KnowlixError::Storage(e.to_string()))?;
    row.map(project_from_row).transpose()
}

pub async fn get_project_by_name(name: &str) -> Result<Option<Project>> {
    let s = get_state()?;
    let row = sqlx::query_as::<_, ProjectRow>(
        "SELECT id, name, description, folders, created_at, updated_at FROM projects WHERE name = ?",
    )
    .bind(name)
    .fetch_optional(&s.db)
    .await
    .map_err(|e| KnowlixError::Storage(e.to_string()))?;
    row.map(project_from_row).transpose()
}

pub async fn list_projects() -> Result<Vec<Project>> {
    let s = get_state()?;
    let rows = sqlx::query_as::<_, ProjectRow>(
        "SELECT id, name, description, folders, created_at, updated_at FROM projects ORDER BY updated_at DESC",
    )
    .fetch_all(&s.db)
    .await
    .map_err(|e| KnowlixError::Storage(e.to_string()))?;
    rows.into_iter().map(project_from_row).collect()
}

pub async fn update_project(project: &Project) -> Result<()> {
    let s = get_state()?;
    let folders = serde_json::to_string(&project.folders)
        .map_err(|e| KnowlixError::Storage(e.to_string()))?;
    sqlx::query(
        "UPDATE projects SET name = ?, description = ?, folders = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&project.name)
    .bind(&project.description)
    .bind(&folders)
    .bind(project.updated_at.to_rfc3339())
    .bind(&project.id)
    .execute(&s.db)
    .await
    .map_err(|e| KnowlixError::Storage(e.to_string()))?;
    Ok(())
}

pub async fn delete_project(id: &str) -> Result<()> {
    let s = get_state()?;
    sqlx::query("DELETE FROM projects WHERE id = ?")
        .bind(id)
        .execute(&s.db)
        .await
        .map_err(|e| KnowlixError::Storage(e.to_string()))?;
    Ok(())
}

// ---- FileEntry CRUD ----

pub async fn upsert_file_entry(entry: &FileEntry) -> Result<()> {
    let s = get_state()?;
    let file_type_str = file_type_to_str(&entry.file_type);
    sqlx::query(
        "INSERT INTO file_entries
            (id, project_id, path, file_type, language, size_bytes, content_hash, last_indexed, indexed)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(path) DO UPDATE SET
            project_id = excluded.project_id,
            file_type  = excluded.file_type,
            language   = excluded.language,
            size_bytes = excluded.size_bytes,
            content_hash = excluded.content_hash,
            last_indexed = excluded.last_indexed,
            indexed    = excluded.indexed",
    )
    .bind(&entry.id)
    .bind(&entry.project_id)
    .bind(&entry.path)
    .bind(file_type_str)
    .bind(&entry.language)
    .bind(entry.size_bytes)
    .bind(&entry.content_hash)
    .bind(entry.last_indexed.to_rfc3339())
    .bind(entry.indexed as i64)
    .execute(&s.db)
    .await
    .map_err(|e| KnowlixError::Storage(e.to_string()))?;
    Ok(())
}

pub async fn get_file_entry(path: &str) -> Result<Option<FileEntry>> {
    let s = get_state()?;
    let row = sqlx::query_as::<_, FileEntryRow>(
        "SELECT id, project_id, path, file_type, language, size_bytes, content_hash, last_indexed, indexed
         FROM file_entries WHERE path = ?",
    )
    .bind(path)
    .fetch_optional(&s.db)
    .await
    .map_err(|e| KnowlixError::Storage(e.to_string()))?;
    row.map(file_entry_from_row).transpose()
}

pub async fn list_files_for_project(project_id: &str) -> Result<Vec<FileEntry>> {
    let s = get_state()?;
    let rows = sqlx::query_as::<_, FileEntryRow>(
        "SELECT id, project_id, path, file_type, language, size_bytes, content_hash, last_indexed, indexed
         FROM file_entries WHERE project_id = ?",
    )
    .bind(project_id)
    .fetch_all(&s.db)
    .await
    .map_err(|e| KnowlixError::Storage(e.to_string()))?;
    rows.into_iter().map(file_entry_from_row).collect()
}

pub async fn delete_file_entry(path: &str) -> Result<()> {
    let s = get_state()?;
    sqlx::query("DELETE FROM file_entries WHERE path = ?")
        .bind(path)
        .execute(&s.db)
        .await
        .map_err(|e| KnowlixError::Storage(e.to_string()))?;
    Ok(())
}

// ---- Chunks CRUD ----

pub async fn insert_chunks(chunks: Vec<Chunk>) -> Result<()> {
    let s = get_state()?;
    for chunk in &chunks {
        sqlx::query(
            "INSERT OR REPLACE INTO chunks
                (id, file_id, chunk_index, content, token_count, start_byte, end_byte)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&chunk.id)
        .bind(&chunk.file_id)
        .bind(chunk.chunk_index)
        .bind(&chunk.content)
        .bind(chunk.token_count)
        .bind(chunk.start_byte)
        .bind(chunk.end_byte)
        .execute(&s.db)
        .await
        .map_err(|e| KnowlixError::Storage(e.to_string()))?;
    }
    Ok(())
}

pub async fn get_chunks_for_file(file_id: &str) -> Result<Vec<Chunk>> {
    let s = get_state()?;
    let rows = sqlx::query_as::<_, ChunkRow>(
        "SELECT id, file_id, chunk_index, content, token_count, start_byte, end_byte
         FROM chunks WHERE file_id = ? ORDER BY chunk_index",
    )
    .bind(file_id)
    .fetch_all(&s.db)
    .await
    .map_err(|e| KnowlixError::Storage(e.to_string()))?;
    Ok(rows.into_iter().map(chunk_from_row).collect())
}

pub async fn delete_chunks_for_file(file_id: &str) -> Result<()> {
    let s = get_state()?;
    sqlx::query("DELETE FROM chunks WHERE file_id = ?")
        .bind(file_id)
        .execute(&s.db)
        .await
        .map_err(|e| KnowlixError::Storage(e.to_string()))?;
    Ok(())
}

// ---- FTS (Tantivy) ----

pub fn index_chunk_fts(chunk: &Chunk, file_path: &str, project_id: &str) -> Result<()> {
    let s = get_state()?;
    let f = &s.fields;
    let document = doc!(
        f.chunk_id => chunk.id.as_str(),
        f.file_id => chunk.file_id.as_str(),
        f.project_id => project_id,
        f.file_path => file_path,
        f.content => chunk.content.as_str(),
    );
    s.writer
        .lock()
        .map_err(|_| KnowlixError::Index("Writer lock poisoned".into()))?
        .add_document(document)
        .map_err(|e| KnowlixError::Index(e.to_string()))?;
    Ok(())
}

pub fn remove_file_from_fts(file_id: &str) -> Result<()> {
    let s = get_state()?;
    let term = Term::from_field_text(s.fields.file_id, file_id);
    s.writer
        .lock()
        .map_err(|_| KnowlixError::Index("Writer lock poisoned".into()))?
        .delete_term(term);
    Ok(())
}

pub fn commit_fts() -> Result<()> {
    let s = get_state()?;
    s.writer
        .lock()
        .map_err(|_| KnowlixError::Index("Writer lock poisoned".into()))?
        .commit()
        .map_err(|e| KnowlixError::Index(e.to_string()))?;
    s.reader
        .reload()
        .map_err(|e| KnowlixError::Index(e.to_string()))?;
    Ok(())
}

pub struct FtsHit {
    pub chunk_id: String,
    pub file_id: String,
    pub file_path: String,
    pub score: f32,
    pub snippet: String,
}

pub fn search_keyword_fts(
    query_str: &str,
    project_id: Option<&str>,
    limit: usize,
) -> Result<Vec<FtsHit>> {
    let s = get_state()?;
    let f = &s.fields;
    let searcher = s.reader.searcher();

    let query_parser = QueryParser::for_index(&s.index, vec![f.content]);
    let content_query = query_parser
        .parse_query(query_str)
        .map_err(|e| KnowlixError::Index(format!("Query parse error: {e}")))?;

    let fetch_limit = limit.max(1);
    let top_docs = if let Some(pid) = project_id {
        let project_term = Term::from_field_text(f.project_id, pid);
        let project_query: Box<dyn tantivy::query::Query> =
            Box::new(TermQuery::new(project_term, IndexRecordOption::Basic));
        let combined = BooleanQuery::new(vec![
            (Occur::Must, content_query),
            (Occur::Must, project_query),
        ]);
        searcher
            .search(&combined, &TopDocs::with_limit(fetch_limit))
            .map_err(|e| KnowlixError::Index(e.to_string()))?
    } else {
        searcher
            .search(&content_query, &TopDocs::with_limit(fetch_limit))
            .map_err(|e| KnowlixError::Index(e.to_string()))?
    };

    let mut hits = Vec::with_capacity(top_docs.len());
    for (score, doc_addr) in top_docs {
        let doc: TantivyDocument = searcher
            .doc(doc_addr)
            .map_err(|e| KnowlixError::Index(e.to_string()))?;

        let get_str = |field: Field| -> String {
            doc.get_first(field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };

        let content = get_str(f.content);
        let snippet = make_snippet(&content, 300);

        hits.push(FtsHit {
            chunk_id: get_str(f.chunk_id),
            file_id: get_str(f.file_id),
            file_path: get_str(f.file_path),
            score,
            snippet,
        });
    }

    Ok(hits)
}

fn make_snippet(content: &str, max_len: usize) -> String {
    if content.len() <= max_len {
        return content.to_string();
    }
    let truncated = &content[..max_len];
    match truncated.rfind(|c: char| c.is_whitespace()) {
        Some(pos) => format!("{}…", &truncated[..pos]),
        None => format!("{}…", truncated),
    }
}

// ---- Wiki pages ----

pub async fn upsert_wiki_page(page: &WikiPage) -> Result<()> {
    let s = get_state()?;
    let tags = serde_json::to_string(&page.tags).map_err(|e| KnowlixError::Storage(e.to_string()))?;
    let hashes = serde_json::to_string(&page.source_hashes)
        .map_err(|e| KnowlixError::Storage(e.to_string()))?;
    sqlx::query(
        "INSERT INTO wiki_pages (id, project_id, title, content, tags, source_hashes, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
            title = excluded.title,
            content = excluded.content,
            tags = excluded.tags,
            source_hashes = excluded.source_hashes,
            updated_at = excluded.updated_at",
    )
    .bind(&page.id)
    .bind(&page.project_id)
    .bind(&page.title)
    .bind(&page.content)
    .bind(&tags)
    .bind(&hashes)
    .bind(page.created_at.to_rfc3339())
    .bind(page.updated_at.to_rfc3339())
    .execute(&s.db)
    .await
    .map_err(|e| KnowlixError::Storage(e.to_string()))?;
    Ok(())
}

pub async fn get_wiki_pages_for_project(project_id: &str) -> Result<Vec<WikiPage>> {
    let s = get_state()?;
    let rows = sqlx::query_as::<_, WikiPageRow>(
        "SELECT id, project_id, title, content, tags, source_hashes, created_at, updated_at
         FROM wiki_pages WHERE project_id = ? ORDER BY title",
    )
    .bind(project_id)
    .fetch_all(&s.db)
    .await
    .map_err(|e| KnowlixError::Storage(e.to_string()))?;
    rows.into_iter().map(wiki_page_from_row).collect()
}

pub async fn get_global_wiki_pages() -> Result<Vec<WikiPage>> {
    let s = get_state()?;
    let rows = sqlx::query_as::<_, WikiPageRow>(
        "SELECT id, project_id, title, content, tags, source_hashes, created_at, updated_at
         FROM wiki_pages WHERE project_id IS NULL ORDER BY title",
    )
    .fetch_all(&s.db)
    .await
    .map_err(|e| KnowlixError::Storage(e.to_string()))?;
    rows.into_iter().map(wiki_page_from_row).collect()
}

pub async fn delete_wiki_pages_for_project(project_id: &str) -> Result<()> {
    let s = get_state()?;
    sqlx::query("DELETE FROM wiki_pages WHERE project_id = ?")
        .bind(project_id)
        .execute(&s.db)
        .await
        .map_err(|e| KnowlixError::Storage(e.to_string()))?;
    Ok(())
}

// ---- AI Config ----

pub async fn get_ai_config() -> Result<AiConfig> {
    let s = get_state()?;
    let row = sqlx::query_as::<_, AiConfigRow>(
        "SELECT provider, ollama_model, ollama_url, api_key, api_base_url, api_model
         FROM ai_config WHERE id = 1",
    )
    .fetch_optional(&s.db)
    .await
    .map_err(|e| KnowlixError::Storage(e.to_string()))?;

    match row {
        Some(r) => Ok(AiConfig {
            provider: str_to_ai_provider(&r.provider),
            ollama_model: r.ollama_model,
            ollama_url: r.ollama_url,
            api_key: r.api_key,
            api_base_url: r.api_base_url,
            api_model: r.api_model,
        }),
        None => Ok(AiConfig::default()),
    }
}

pub async fn save_ai_config(config: &AiConfig) -> Result<()> {
    let s = get_state()?;
    let provider = ai_provider_to_str(&config.provider);
    sqlx::query(
        "INSERT INTO ai_config (id, provider, ollama_model, ollama_url, api_key, api_base_url, api_model)
         VALUES (1, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
            provider = excluded.provider,
            ollama_model = excluded.ollama_model,
            ollama_url = excluded.ollama_url,
            api_key = excluded.api_key,
            api_base_url = excluded.api_base_url,
            api_model = excluded.api_model",
    )
    .bind(provider)
    .bind(&config.ollama_model)
    .bind(&config.ollama_url)
    .bind(&config.api_key)
    .bind(&config.api_base_url)
    .bind(&config.api_model)
    .execute(&s.db)
    .await
    .map_err(|e| KnowlixError::Storage(e.to_string()))?;
    Ok(())
}

// ---- Row types ----

#[derive(sqlx::FromRow)]
struct ProjectRow {
    id: String,
    name: String,
    description: Option<String>,
    folders: String,
    created_at: String,
    updated_at: String,
}

fn project_from_row(row: ProjectRow) -> Result<Project> {
    let folders: Vec<String> = serde_json::from_str(&row.folders)
        .map_err(|e| KnowlixError::Storage(e.to_string()))?;
    let created_at = chrono::DateTime::parse_from_rfc3339(&row.created_at)
        .map_err(|e| KnowlixError::Storage(e.to_string()))?
        .with_timezone(&Utc);
    let updated_at = chrono::DateTime::parse_from_rfc3339(&row.updated_at)
        .map_err(|e| KnowlixError::Storage(e.to_string()))?
        .with_timezone(&Utc);
    Ok(Project {
        id: row.id,
        name: row.name,
        description: row.description,
        folders,
        created_at,
        updated_at,
    })
}

#[derive(sqlx::FromRow)]
struct FileEntryRow {
    id: String,
    project_id: String,
    path: String,
    file_type: String,
    language: Option<String>,
    size_bytes: i64,
    content_hash: String,
    last_indexed: String,
    indexed: i64,
}

fn file_entry_from_row(row: FileEntryRow) -> Result<FileEntry> {
    let file_type = str_to_file_type(&row.file_type);
    let last_indexed = chrono::DateTime::parse_from_rfc3339(&row.last_indexed)
        .map_err(|e| KnowlixError::Storage(e.to_string()))?
        .with_timezone(&Utc);
    Ok(FileEntry {
        id: row.id,
        project_id: row.project_id,
        path: row.path,
        file_type,
        language: row.language,
        size_bytes: row.size_bytes,
        content_hash: row.content_hash,
        last_indexed,
        indexed: row.indexed != 0,
    })
}

#[derive(sqlx::FromRow)]
struct ChunkRow {
    id: String,
    file_id: String,
    chunk_index: i32,
    content: String,
    token_count: i32,
    start_byte: i64,
    end_byte: i64,
}

fn chunk_from_row(row: ChunkRow) -> Chunk {
    Chunk {
        id: row.id,
        file_id: row.file_id,
        chunk_index: row.chunk_index,
        content: row.content,
        token_count: row.token_count,
        start_byte: row.start_byte,
        end_byte: row.end_byte,
    }
}

#[derive(sqlx::FromRow)]
struct WikiPageRow {
    id: String,
    project_id: Option<String>,
    title: String,
    content: String,
    tags: String,
    source_hashes: String,
    created_at: String,
    updated_at: String,
}

fn wiki_page_from_row(row: WikiPageRow) -> Result<WikiPage> {
    let tags: Vec<String> =
        serde_json::from_str(&row.tags).map_err(|e| KnowlixError::Storage(e.to_string()))?;
    let source_hashes: Vec<String> = serde_json::from_str(&row.source_hashes)
        .map_err(|e| KnowlixError::Storage(e.to_string()))?;
    let created_at = chrono::DateTime::parse_from_rfc3339(&row.created_at)
        .map_err(|e| KnowlixError::Storage(e.to_string()))?
        .with_timezone(&Utc);
    let updated_at = chrono::DateTime::parse_from_rfc3339(&row.updated_at)
        .map_err(|e| KnowlixError::Storage(e.to_string()))?
        .with_timezone(&Utc);
    Ok(WikiPage {
        id: row.id,
        project_id: row.project_id,
        title: row.title,
        content: row.content,
        tags,
        source_hashes,
        created_at,
        updated_at,
    })
}

#[derive(sqlx::FromRow)]
struct AiConfigRow {
    provider: String,
    ollama_model: Option<String>,
    ollama_url: String,
    api_key: Option<String>,
    api_base_url: String,
    api_model: Option<String>,
}

// ---- Enum conversions ----

fn file_type_to_str(ft: &FileType) -> &'static str {
    match ft {
        FileType::Text => "text",
        FileType::Markdown => "markdown",
        FileType::Code => "code",
        FileType::Pdf => "pdf",
        FileType::Word => "word",
        FileType::Excel => "excel",
        FileType::Image => "image",
        FileType::Unknown => "unknown",
    }
}

fn str_to_file_type(s: &str) -> FileType {
    match s {
        "text" => FileType::Text,
        "markdown" => FileType::Markdown,
        "code" => FileType::Code,
        "pdf" => FileType::Pdf,
        "word" => FileType::Word,
        "excel" => FileType::Excel,
        "image" => FileType::Image,
        _ => FileType::Unknown,
    }
}

fn ai_provider_to_str(p: &AiProvider) -> &'static str {
    match p {
        AiProvider::None => "none",
        AiProvider::Ollama => "ollama",
        AiProvider::Api => "api",
    }
}

fn str_to_ai_provider(s: &str) -> AiProvider {
    match s {
        "ollama" => AiProvider::Ollama,
        "api" => AiProvider::Api,
        _ => AiProvider::None,
    }
}

// ---- Tests ----

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    static TEMP_DIR: std::sync::OnceLock<TempDir> = std::sync::OnceLock::new();
    static LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();

    async fn setup() -> tokio::sync::MutexGuard<'static, ()> {
        let lock = LOCK.get_or_init(|| tokio::sync::Mutex::new(()));
        let guard = lock.lock().await;
        let dir = TEMP_DIR.get_or_init(|| TempDir::new().expect("tempdir"));
        init_with_dir(dir.path().to_path_buf())
            .await
            .expect("storage init");
        guard
    }

    #[tokio::test]
    async fn test_project_crud() {
        let _guard = setup().await;
        let id = uuid::Uuid::new_v4().to_string();
        let project = Project {
            id: id.clone(),
            name: format!("Test Project {}", &id[..8]),
            description: Some("desc".into()),
            folders: vec!["/tmp/test".into()],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        insert_project(&project).await.expect("insert");

        let fetched = get_project(&id).await.expect("get").expect("should exist");
        assert_eq!(fetched.name, project.name);
        assert_eq!(fetched.folders, project.folders);

        let all = list_projects().await.expect("list");
        assert!(all.iter().any(|p| p.id == id));

        delete_project(&id).await.expect("delete");
        let gone = get_project(&id).await.expect("get after delete");
        assert!(gone.is_none());
    }

    #[tokio::test]
    async fn test_duplicate_project_name() {
        let _guard = setup().await;
        let name = format!("Dup-{}", uuid::Uuid::new_v4());
        let make = |name: &str| Project {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            description: None,
            folders: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        insert_project(&make(&name)).await.expect("first insert");
        let err = insert_project(&make(&name)).await.unwrap_err();
        assert!(matches!(err, KnowlixError::Duplicate(_)));
    }

    #[tokio::test]
    async fn test_file_entry_upsert() {
        let _guard = setup().await;

        let proj_id = uuid::Uuid::new_v4().to_string();
        let project = Project {
            id: proj_id.clone(),
            name: format!("FE-Test-{}", &proj_id[..8]),
            description: None,
            folders: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        insert_project(&project).await.expect("insert project");

        let file_id = uuid::Uuid::new_v4().to_string();
        let entry = FileEntry {
            id: file_id.clone(),
            project_id: proj_id.clone(),
            path: format!("/tmp/test-{}.txt", &file_id[..8]),
            file_type: FileType::Text,
            language: None,
            size_bytes: 100,
            content_hash: "abc123".into(),
            last_indexed: Utc::now(),
            indexed: true,
        };

        upsert_file_entry(&entry).await.expect("upsert");
        let fetched = get_file_entry(&entry.path).await.expect("get").expect("exists");
        assert_eq!(fetched.content_hash, "abc123");
        assert!(fetched.indexed);

        delete_file_entry(&entry.path).await.expect("delete");
        let gone = get_file_entry(&entry.path).await.expect("get after delete");
        assert!(gone.is_none());
    }

    #[tokio::test]
    async fn test_chunk_crud() {
        let _guard = setup().await;

        let proj_id = uuid::Uuid::new_v4().to_string();
        let project = Project {
            id: proj_id.clone(),
            name: format!("Chunk-Test-{}", &proj_id[..8]),
            description: None,
            folders: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        insert_project(&project).await.unwrap();

        let file_id = uuid::Uuid::new_v4().to_string();
        let entry = FileEntry {
            id: file_id.clone(),
            project_id: proj_id.clone(),
            path: format!("/tmp/chunk-{}.txt", &file_id[..8]),
            file_type: FileType::Text,
            language: None,
            size_bytes: 200,
            content_hash: "def456".into(),
            last_indexed: Utc::now(),
            indexed: true,
        };
        upsert_file_entry(&entry).await.unwrap();

        let chunks = vec![
            Chunk {
                id: uuid::Uuid::new_v4().to_string(),
                file_id: file_id.clone(),
                chunk_index: 0,
                content: "hello world".into(),
                token_count: 2,
                start_byte: 0,
                end_byte: 11,
            },
            Chunk {
                id: uuid::Uuid::new_v4().to_string(),
                file_id: file_id.clone(),
                chunk_index: 1,
                content: "foo bar".into(),
                token_count: 2,
                start_byte: 11,
                end_byte: 18,
            },
        ];

        insert_chunks(chunks.clone()).await.unwrap();
        let fetched = get_chunks_for_file(&file_id).await.unwrap();
        assert_eq!(fetched.len(), 2);
        assert_eq!(fetched[0].content, "hello world");

        delete_chunks_for_file(&file_id).await.unwrap();
        let empty = get_chunks_for_file(&file_id).await.unwrap();
        assert!(empty.is_empty());
    }

    #[test]
    fn test_make_snippet_short() {
        let s = make_snippet("hello world", 300);
        assert_eq!(s, "hello world");
    }

    #[test]
    fn test_make_snippet_truncates() {
        let long = "a".repeat(400);
        let snippet = make_snippet(&long, 300);
        assert!(snippet.len() <= 304); // 300 + "…"
    }
}
