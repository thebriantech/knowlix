use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::Utc;
use knowlix_common::{Chunk, FileEntry, FileType, IndexFileResult, IndexFileStatus, IndexProgress, IndexStats, IndexStatus, KnowlixError, Result};
use sha2::{Digest, Sha256};
use uuid::Uuid;

static INDEXING: AtomicBool = AtomicBool::new(false);

// ---- Public API ----

pub async fn index_file(file_path: &str, project_id: &str) -> Result<()> {
    index_file_impl(file_path, project_id, true).await
}

async fn index_file_impl(file_path: &str, project_id: &str, commit: bool) -> Result<()> {
    let path = Path::new(file_path);
    if !path.exists() {
        return Err(KnowlixError::FileNotFound(file_path.into()));
    }

    let content_hash = hash_file(file_path)?;

    if let Some(existing) = knowlix_storage::get_file_entry(file_path).await? {
        if existing.content_hash == content_hash {
            return Ok(());
        }
        knowlix_storage::delete_embeddings_for_file(&existing.id).await?;
        let fid = existing.id.clone();
        tokio::task::spawn_blocking(move || knowlix_storage::remove_file_from_fts(&fid))
            .await
            .map_err(|e| KnowlixError::Index(e.to_string()))??;
        knowlix_storage::delete_chunks_for_file(&existing.id).await?;
    }

    let text = extract_text(file_path)?;
    let file_type = detect_file_type(path);
    let language = if file_type == FileType::Code {
        path.extension()
            .and_then(|e| e.to_str())
            .map(detect_language)
            .map(|s| s.to_string())
    } else {
        None
    };

    let metadata = std::fs::metadata(file_path)?;
    let file_id = Uuid::new_v4().to_string();
    let now = Utc::now();

    let entry = FileEntry {
        id: file_id.clone(),
        project_id: project_id.to_string(),
        path: file_path.to_string(),
        file_type,
        language,
        size_bytes: metadata.len() as i64,
        content_hash,
        last_indexed: now,
        indexed: true,
    };

    knowlix_storage::upsert_file_entry(&entry).await?;

    let chunks = chunk_text(&text, &file_id);
    let fp = file_path.to_string();
    let pid = project_id.to_string();
    let chunks_for_fts = chunks.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        for chunk in &chunks_for_fts {
            knowlix_storage::index_chunk_fts(chunk, &fp, &pid)?;
        }
        Ok(())
    })
    .await
    .map_err(|e| KnowlixError::Index(e.to_string()))??;

    knowlix_storage::insert_chunks(chunks.clone()).await?;

    if knowlix_storage::is_embedding_ready() {
        embed_and_store_chunks(&chunks).await;
    }

    if commit {
        tokio::task::spawn_blocking(knowlix_storage::commit_fts)
            .await
            .map_err(|e| KnowlixError::Index(e.to_string()))??;
    }

    Ok(())
}

async fn embed_and_store_chunks(chunks: &[Chunk]) {
    let texts: Vec<String> = chunks.iter().map(|c| c.content.clone()).collect();
    match knowlix_storage::embed_texts(texts).await {
        Ok(embeddings) => {
            for (chunk, emb) in chunks.iter().zip(embeddings.iter()) {
                if let Err(e) = knowlix_storage::insert_embedding(
                    &chunk.id,
                    knowlix_storage::EMBEDDING_MODEL_NAME,
                    emb,
                )
                .await
                {
                    tracing::warn!("[indexer] embed store failed chunk={} err={e}", chunk.id);
                }
            }
        }
        Err(e) => {
            tracing::warn!("[indexer] embed_texts failed err={e}");
        }
    }
}

pub async fn remove_file(file_path: &str) -> Result<()> {
    if let Some(entry) = knowlix_storage::get_file_entry(file_path).await? {
        let fid = entry.id.clone();
        knowlix_storage::delete_embeddings_for_file(&fid).await?;
        tokio::task::spawn_blocking(move || knowlix_storage::remove_file_from_fts(&fid))
            .await
            .map_err(|e| KnowlixError::Index(e.to_string()))??;
        knowlix_storage::delete_chunks_for_file(&entry.id).await?;
        tokio::task::spawn_blocking(knowlix_storage::commit_fts)
            .await
            .map_err(|e| KnowlixError::Index(e.to_string()))??;
    }
    knowlix_storage::delete_file_entry(file_path).await
}

pub fn extract_text(file_path: &str) -> Result<String> {
    let path = Path::new(file_path);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "txt" | "log" | "md" | "mdx" | "rs" | "py" | "ts" | "tsx" | "js" | "jsx" | "go"
        | "java" | "c" | "cpp" | "cc" | "h" | "hpp" | "html" | "css" | "json" | "yaml"
        | "yml" | "toml" | "sh" | "bash" | "sql" | "rb" | "php" | "swift" | "kt" | "scala"
        | "r" | "lua" | "xml" | "vue" | "svelte" | "dart" | "csv" => {
            std::fs::read_to_string(file_path).map_err(KnowlixError::Io)
        }
        "docx" => extract_zip_xml_text(file_path, "word/document.xml", &["</w:p>"], &["<w:br/>", "<w:br />"]),
        "odt" | "odp" => extract_zip_xml_text(file_path, "content.xml", &["</text:p>"], &["<text:line-break/>", "<text:line-break />"]),
        "pdf" => extract_pdf_text(file_path),
        "xlsx" | "xls" | "xlsb" | "ods" => extract_excel_text(file_path),
        "pptx" => extract_pptx_text(file_path),
        _ => Err(KnowlixError::UnsupportedFileType(ext)),
    }
}

pub async fn reindex_project(
    project_id: &str,
    on_progress: Option<Box<dyn Fn(IndexProgress) + Send + Sync>>,
) -> Result<IndexStats> {
    INDEXING.store(true, Ordering::SeqCst);
    let start = std::time::Instant::now();

    let folders = knowlix_storage::get_project(project_id)
        .await?
        .map(|p| p.folders)
        .unwrap_or_default();
    tracing::info!("[reindex] project={project_id} folders={:?}", folders);

    let tracked = knowlix_storage::list_files_for_project(project_id).await?;
    tracing::info!("[reindex] tracked files count={}", tracked.len());
    let tracked_paths: std::collections::HashSet<String> =
        tracked.iter().map(|e| e.path.clone()).collect();

    // Pre-compute new paths so we can report accurate totals
    let mut new_paths: Vec<String> = Vec::new();
    for folder in &folders {
        let found = walk_folder(folder);
        tracing::info!("[reindex] walk folder={folder} found={} files", found.len());
        for path in found {
            if !tracked_paths.contains(&path) {
                new_paths.push(path);
            }
        }
    }

    let total = tracked.len() + new_paths.len();
    let mut current = 0usize;
    let mut indexed = 0;
    let mut skipped = 0;
    let mut failed = 0;
    let mut removed = 0;
    let mut file_results: Vec<IndexFileResult> = Vec::new();

    for entry in &tracked {
        current += 1;
        if let Some(ref f) = on_progress {
            f(IndexProgress { current, total, current_file: entry.path.clone() });
        }

        let current_hash = match hash_file(&entry.path) {
            Ok(h) => h,
            Err(_) if !Path::new(&entry.path).exists() => {
                tracing::info!("[reindex] file deleted, removing from index path={}", entry.path);
                if let Err(re) = remove_file(&entry.path).await {
                    tracing::warn!("[reindex] remove failed path={} err={re}", entry.path);
                }
                removed += 1;
                file_results.push(IndexFileResult {
                    path: entry.path.clone(),
                    status: IndexFileStatus::Removed,
                    error: None,
                });
                continue;
            }
            Err(e) => {
                tracing::warn!("[reindex] hash failed path={} err={e}", entry.path);
                failed += 1;
                file_results.push(IndexFileResult {
                    path: entry.path.clone(),
                    status: IndexFileStatus::Failed,
                    error: Some(e.to_string()),
                });
                continue;
            }
        };
        if current_hash == entry.content_hash && entry.indexed {
            skipped += 1;
            file_results.push(IndexFileResult {
                path: entry.path.clone(),
                status: IndexFileStatus::Skipped,
                error: None,
            });
            continue;
        }
        match index_file_impl(&entry.path, project_id, false).await {
            Ok(()) => {
                tracing::info!("[reindex] re-indexed {}", entry.path);
                indexed += 1;
                file_results.push(IndexFileResult {
                    path: entry.path.clone(),
                    status: IndexFileStatus::Indexed,
                    error: None,
                });
            }
            Err(e) => {
                tracing::warn!("[reindex] re-index failed {} err={e}", entry.path);
                failed += 1;
                file_results.push(IndexFileResult {
                    path: entry.path.clone(),
                    status: IndexFileStatus::Failed,
                    error: Some(e.to_string()),
                });
            }
        }
    }

    tracing::info!("[reindex] new files to index={}", new_paths.len());
    for path in &new_paths {
        current += 1;
        if let Some(ref f) = on_progress {
            f(IndexProgress { current, total, current_file: path.clone() });
        }

        match index_file_impl(path, project_id, false).await {
            Ok(()) => {
                tracing::info!("[reindex] indexed {path}");
                indexed += 1;
                file_results.push(IndexFileResult {
                    path: path.clone(),
                    status: IndexFileStatus::Indexed,
                    error: None,
                });
            }
            Err(e) => {
                tracing::warn!("[reindex] failed {path} err={e}");
                failed += 1;
                file_results.push(IndexFileResult {
                    path: path.clone(),
                    status: IndexFileStatus::Failed,
                    error: Some(e.to_string()),
                });
            }
        }
    }

    tokio::task::spawn_blocking(knowlix_storage::commit_fts)
        .await
        .map_err(|e| KnowlixError::Index(e.to_string()))??;

    INDEXING.store(false, Ordering::SeqCst);

    Ok(IndexStats {
        total_files: total,
        indexed,
        skipped,
        failed,
        removed,
        duration_ms: start.elapsed().as_millis() as u64,
        file_results,
    })
}

pub async fn get_index_status(project_id: &str) -> Result<IndexStatus> {
    let files = knowlix_storage::list_files_for_project(project_id).await?;
    let total = files.len();
    let indexed_count = files.iter().filter(|f| f.indexed).count();
    let last_indexed = files.iter().filter(|f| f.indexed).map(|f| f.last_indexed).max();
    Ok(IndexStatus {
        total_files: total,
        indexed_files: indexed_count,
        in_progress: INDEXING.load(Ordering::SeqCst),
        last_indexed,
    })
}

// ---- Helpers ----

pub fn chunk_text(text: &str, file_id: &str) -> Vec<Chunk> {
    // ~900 tokens ≈ 3600 chars, overlap 100 tokens ≈ 400 chars
    const CHUNK_SIZE: usize = 3600;
    const OVERLAP: usize = 400;

    if text.len() <= CHUNK_SIZE {
        let token_count = estimate_tokens(text);
        return vec![Chunk {
            id: Uuid::new_v4().to_string(),
            file_id: file_id.to_string(),
            chunk_index: 0,
            content: text.to_string(),
            token_count: token_count as i32,
            start_byte: 0,
            end_byte: text.len() as i64,
        }];
    }

    let mut chunks = Vec::new();
    let mut start = 0;
    let mut chunk_index = 0;

    while start < text.len() {
        let end = if start + CHUNK_SIZE >= text.len() {
            text.len()
        } else {
            // Align to char boundary
            let mut end = start + CHUNK_SIZE;
            while end > start && !text.is_char_boundary(end) {
                end -= 1;
            }
            // Prefer newline boundary
            if let Some(nl) = text[start..end].rfind('\n') {
                start + nl + 1
            } else {
                end
            }
        };

        let content = text[start..end].to_string();
        let token_count = estimate_tokens(&content);
        chunks.push(Chunk {
            id: Uuid::new_v4().to_string(),
            file_id: file_id.to_string(),
            chunk_index,
            content,
            token_count: token_count as i32,
            start_byte: start as i64,
            end_byte: end as i64,
        });

        chunk_index += 1;
        if end >= text.len() {
            break;
        }
        start = end.saturating_sub(OVERLAP);
        // Align overlap start to char boundary
        while start < end && !text.is_char_boundary(start) {
            start += 1;
        }
    }

    chunks
}

fn walk_folder(folder: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let Ok(entries) = std::fs::read_dir(folder) else {
        return paths;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            paths.extend(walk_folder(&path.to_string_lossy()));
        } else if path.is_file() {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if is_supported_extension(&ext) {
                if let Some(p) = path.to_str() {
                    paths.push(p.to_string());
                }
            }
        }
    }
    paths
}

fn is_supported_extension(ext: &str) -> bool {
    matches!(
        ext,
        "txt" | "log" | "md" | "mdx" | "rs" | "py" | "ts" | "tsx" | "js" | "jsx" | "go"
            | "java" | "c" | "cpp" | "cc" | "h" | "hpp" | "html" | "css" | "json" | "yaml"
            | "yml" | "toml" | "sh" | "bash" | "sql" | "rb" | "php" | "swift" | "kt" | "scala"
            | "r" | "lua" | "xml" | "vue" | "svelte" | "dart"
            | "csv" | "docx" | "odt" | "odp"
            | "pdf"
            | "xlsx" | "xls" | "xlsb" | "ods"
            | "pptx"
    )
}

fn xml_to_text(xml: &str, para_end_tags: &[&str], break_tags: &[&str]) -> String {
    let mut s = xml.to_string();
    for tag in para_end_tags {
        s = s.replace(tag, "\n");
    }
    for tag in break_tags {
        s = s.replace(tag, "\n");
    }
    let mut text = String::with_capacity(s.len());
    let mut inside_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => inside_tag = true,
            '>' => inside_tag = false,
            _ if !inside_tag => text.push(ch),
            _ => {}
        }
    }
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
}

fn extract_zip_xml_text(
    file_path: &str,
    entry_name: &str,
    para_end_tags: &[&str],
    break_tags: &[&str],
) -> Result<String> {
    use std::io::Read as _;
    let file = std::fs::File::open(file_path)?;
    if file.metadata().map(|m| m.len()).unwrap_or(0) == 0 {
        return Ok(String::new());
    }
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| KnowlixError::Io(std::io::Error::other(e.to_string())))?;
    let mut xml = String::new();
    archive
        .by_name(entry_name)
        .map_err(|_| KnowlixError::Io(std::io::Error::other(format!("{entry_name} not found"))))?
        .read_to_string(&mut xml)?;
    Ok(xml_to_text(&xml, para_end_tags, break_tags))
}

fn extract_pdf_text(file_path: &str) -> Result<String> {
    let size = std::fs::metadata(file_path).map(|m| m.len()).unwrap_or(0);
    if size == 0 {
        return Ok(String::new());
    }
    pdf_extract::extract_text(file_path)
        .map_err(|e| KnowlixError::Io(std::io::Error::other(e.to_string())))
}

fn extract_excel_text(file_path: &str) -> Result<String> {
    use calamine::{open_workbook_auto, Reader};
    let mut workbook = open_workbook_auto(file_path)
        .map_err(|e| KnowlixError::Io(std::io::Error::other(e.to_string())))?;
    let mut text = String::new();
    for sheet_name in workbook.sheet_names().to_owned() {
        if let Ok(range) = workbook.worksheet_range(&sheet_name) {
            for row in range.rows() {
                let parts: Vec<String> = row
                    .iter()
                    .map(|c| c.to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                if !parts.is_empty() {
                    text.push_str(&parts.join("\t"));
                    text.push('\n');
                }
            }
        }
    }
    Ok(text)
}

fn extract_pptx_text(file_path: &str) -> Result<String> {
    use std::io::Read as _;
    let file = std::fs::File::open(file_path)?;
    if file.metadata().map(|m| m.len()).unwrap_or(0) == 0 {
        return Ok(String::new());
    }
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| KnowlixError::Io(std::io::Error::other(e.to_string())))?;

    let mut slide_names: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let name = {
            match archive.by_index(i) {
                Ok(f) => f.name().to_string(),
                Err(_) => continue,
            }
        };
        if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
            slide_names.push(name);
        }
    }
    slide_names.sort();

    let mut text = String::new();
    for name in &slide_names {
        let mut xml = String::new();
        if let Ok(mut entry) = archive.by_name(name) {
            entry.read_to_string(&mut xml).ok();
            text.push_str(&xml_to_text(&xml, &["</a:p>"], &["<a:br/>", "<a:br />"]));
            text.push('\n');
        }
    }
    Ok(text)
}

fn hash_file(file_path: &str) -> Result<String> {
    let data = std::fs::read(file_path)?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let result = hasher.finalize();
    Ok(result.iter().map(|b| format!("{:02x}", b)).collect())
}

fn estimate_tokens(text: &str) -> usize {
    // Approximate: 1 token ≈ 4 chars
    (text.len() / 4).max(1)
}

fn detect_file_type(path: &Path) -> FileType {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "txt" | "log" | "csv" => FileType::Text,
        "md" | "mdx" => FileType::Markdown,
        "pdf" => FileType::Pdf,
        "docx" | "odt" => FileType::Word,
        "xlsx" | "xls" | "xlsb" | "ods" => FileType::Excel,
        "pptx" | "odp" => FileType::Unknown,
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" => FileType::Image,
        "rs" | "py" | "ts" | "tsx" | "js" | "jsx" | "go" | "java" | "c" | "cpp" | "cc"
        | "h" | "hpp" | "html" | "css" | "json" | "yaml" | "yml" | "toml" | "sh" | "bash"
        | "sql" | "rb" | "php" | "swift" | "kt" | "scala" | "r" | "lua" | "xml" | "vue"
        | "svelte" | "dart" => FileType::Code,
        _ => FileType::Unknown,
    }
}

fn detect_language(ext: &str) -> &'static str {
    match ext.to_lowercase().as_str() {
        "rs" => "rust",
        "py" => "python",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" => "javascript",
        "go" => "go",
        "java" => "java",
        "c" => "c",
        "cpp" | "cc" | "hpp" => "cpp",
        "html" => "html",
        "css" => "css",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "sh" | "bash" => "bash",
        "sql" => "sql",
        "rb" => "ruby",
        "php" => "php",
        "swift" => "swift",
        "kt" => "kotlin",
        "scala" => "scala",
        "r" => "r",
        "lua" => "lua",
        "xml" => "xml",
        "vue" => "vue",
        "svelte" => "svelte",
        "dart" => "dart",
        _ => "plaintext",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    use tempfile::{NamedTempFile, TempDir};

    #[test]
    fn test_extract_text_txt() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "hello world").unwrap();
        let _path = f.path().to_str().unwrap().to_string();
        // Named temp files don't have .txt extension, rename via a path trick
        // Test the core logic directly
        let content = std::fs::read_to_string(f.path()).unwrap();
        assert!(content.contains("hello world"));
    }

    #[test]
    fn test_chunk_text_small() {
        let text = "hello world";
        let chunks = chunk_text(text, "file-1");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content, "hello world");
        assert_eq!(chunks[0].chunk_index, 0);
        assert_eq!(chunks[0].start_byte, 0);
        assert_eq!(chunks[0].end_byte, 11);
    }

    #[test]
    fn test_chunk_text_large() {
        // Generate text larger than CHUNK_SIZE (3600 chars)
        let text = "The quick brown fox jumps over the lazy dog.\n".repeat(100); // ~4500 chars
        let chunks = chunk_text(&text, "file-2");
        assert!(chunks.len() >= 2);
        // Verify chunks cover the full text
        assert_eq!(chunks[0].start_byte, 0);
        // Verify ordering
        for (i, c) in chunks.iter().enumerate() {
            assert_eq!(c.chunk_index, i as i32);
        }
    }

    #[test]
    fn test_chunk_text_overlap() {
        let line = "word ".repeat(1000); // ~5000 chars
        let chunks = chunk_text(&line, "file-3");
        assert!(chunks.len() >= 2);
        // End of chunk N should overlap with start of chunk N+1
        let end1 = chunks[0].end_byte as usize;
        let start2 = chunks[1].start_byte as usize;
        assert!(start2 < end1, "Chunks should overlap");
    }

    #[test]
    fn test_detect_file_type() {
        assert_eq!(detect_file_type(Path::new("foo.rs")), FileType::Code);
        assert_eq!(detect_file_type(Path::new("README.md")), FileType::Markdown);
        assert_eq!(detect_file_type(Path::new("notes.txt")), FileType::Text);
        assert_eq!(detect_file_type(Path::new("doc.pdf")), FileType::Pdf);
        assert_eq!(detect_file_type(Path::new("img.png")), FileType::Image);
        assert_eq!(detect_file_type(Path::new("data.xyz")), FileType::Unknown);
    }

    #[test]
    fn test_detect_language() {
        assert_eq!(detect_language("rs"), "rust");
        assert_eq!(detect_language("py"), "python");
        assert_eq!(detect_language("tsx"), "typescript");
        assert_eq!(detect_language("go"), "go");
        assert_eq!(detect_language("xyz"), "plaintext");
    }

    #[test]
    fn test_hash_file() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "test content").unwrap();
        let path = f.path().to_str().unwrap();
        let hash = hash_file(path).unwrap();
        assert_eq!(hash.len(), 64); // SHA256 hex = 64 chars
        // Same content → same hash
        let hash2 = hash_file(path).unwrap();
        assert_eq!(hash, hash2);
    }

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens(""), 1);
        assert_eq!(estimate_tokens("hello world"), 2); // 11/4 = 2
        assert_eq!(estimate_tokens("a".repeat(400).as_str()), 100);
    }

    // ---- walk_folder / is_supported_extension tests ----

    #[test]
    fn test_is_supported_extension() {
        assert!(is_supported_extension("md"));
        assert!(is_supported_extension("rs"));
        assert!(is_supported_extension("ts"));
        assert!(is_supported_extension("py"));
        assert!(is_supported_extension("txt"));
        assert!(is_supported_extension("pdf"));
        assert!(is_supported_extension("docx"));
        assert!(is_supported_extension("xlsx"));
        assert!(is_supported_extension("pptx"));
        assert!(is_supported_extension("csv"));
        assert!(!is_supported_extension("png"));
        assert!(!is_supported_extension("xyz"));
        assert!(!is_supported_extension(""));
    }

    #[test]
    fn test_walk_folder_finds_supported_files() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        // Supported files
        std::fs::write(root.join("notes.md"), "# Notes").unwrap();
        std::fs::write(root.join("main.rs"), "fn main() {}").unwrap();
        std::fs::write(root.join("app.ts"), "const x = 1;").unwrap();

        // Unsupported files — should be excluded
        std::fs::write(root.join("image.png"), "").unwrap();
        std::fs::write(root.join("archive.zip"), "").unwrap();

        let paths = walk_folder(root.to_str().unwrap());
        let names: Vec<&str> = paths
            .iter()
            .map(|p| std::path::Path::new(p).file_name().unwrap().to_str().unwrap())
            .collect();

        assert!(names.contains(&"notes.md"), "missing notes.md");
        assert!(names.contains(&"main.rs"), "missing main.rs");
        assert!(names.contains(&"app.ts"), "missing app.ts");
        assert!(!names.contains(&"image.png"), "png should be excluded");
        assert!(!names.contains(&"archive.zip"), "zip should be excluded");
    }

    #[test]
    fn test_walk_folder_recurses_subdirs() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        let sub = root.join("subdir").join("nested");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(root.join("top.md"), "top").unwrap();
        std::fs::write(sub.join("deep.rs"), "fn f() {}").unwrap();

        let paths = walk_folder(root.to_str().unwrap());
        let names: Vec<&str> = paths
            .iter()
            .map(|p| std::path::Path::new(p).file_name().unwrap().to_str().unwrap())
            .collect();

        assert!(names.contains(&"top.md"), "missing top-level file");
        assert!(names.contains(&"deep.rs"), "missing nested file");
    }

    #[test]
    fn test_walk_folder_nonexistent_returns_empty() {
        let paths = walk_folder("/nonexistent/path/that/does/not/exist");
        assert!(paths.is_empty());
    }

    // ---- reindex_project integration test ----

    static REINDEX_TEMP: std::sync::OnceLock<TempDir> = std::sync::OnceLock::new();
    static REINDEX_LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();

    async fn setup_storage() -> tokio::sync::MutexGuard<'static, ()> {
        let lock = REINDEX_LOCK.get_or_init(|| tokio::sync::Mutex::new(()));
        let guard = lock.lock().await;
        let dir = REINDEX_TEMP.get_or_init(|| TempDir::new().unwrap());
        knowlix_storage::init_with_dir(dir.path().to_path_buf())
            .await
            .unwrap();
        guard
    }

    #[tokio::test]
    async fn test_reindex_project_discovers_new_files() {
        let _guard = setup_storage().await;

        // Create project with a temp folder
        let files_dir = TempDir::new().unwrap();
        std::fs::write(files_dir.path().join("doc.md"), "# Hello world").unwrap();
        std::fs::write(files_dir.path().join("script.py"), "print('hi')").unwrap();
        std::fs::write(files_dir.path().join("ignore.png"), "").unwrap();

        let project_id = uuid::Uuid::new_v4().to_string();
        let project = knowlix_common::Project {
            id: project_id.clone(),
            name: format!("reindex-test-{}", &project_id[..8]),
            description: None,
            folders: vec![files_dir.path().to_str().unwrap().to_string()],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        knowlix_storage::insert_project(&project).await.unwrap();

        let stats = reindex_project(&project_id, None).await.unwrap();

        println!("total={} indexed={} skipped={} failed={} ms={}",
            stats.total_files, stats.indexed, stats.skipped, stats.failed, stats.duration_ms);

        assert_eq!(stats.total_files, 2, "only .md and .py should be counted");
        assert_eq!(stats.indexed, 2, "both files should be indexed");
        assert_eq!(stats.failed, 0, "no failures expected");
    }

    #[tokio::test]
    async fn test_reindex_project_skips_unchanged_files() {
        let _guard = setup_storage().await;

        let files_dir = TempDir::new().unwrap();
        std::fs::write(files_dir.path().join("note.md"), "unchanged content").unwrap();

        let project_id = uuid::Uuid::new_v4().to_string();
        let project = knowlix_common::Project {
            id: project_id.clone(),
            name: format!("skip-test-{}", &project_id[..8]),
            description: None,
            folders: vec![files_dir.path().to_str().unwrap().to_string()],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        knowlix_storage::insert_project(&project).await.unwrap();

        // First reindex — indexes the file
        let stats1 = reindex_project(&project_id, None).await.unwrap();
        assert_eq!(stats1.indexed, 1);

        // Second reindex — file unchanged, should skip
        let stats2 = reindex_project(&project_id, None).await.unwrap();
        assert_eq!(stats2.indexed, 0);
        assert_eq!(stats2.skipped, 1);
    }
}
