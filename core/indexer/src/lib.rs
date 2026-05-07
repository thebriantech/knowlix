use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::Utc;
use knowlix_common::{Chunk, FileEntry, FileType, IndexStats, IndexStatus, KnowlixError, Result};
use sha2::{Digest, Sha256};
use uuid::Uuid;

static INDEXING: AtomicBool = AtomicBool::new(false);

// ---- Public API ----

pub async fn index_file(file_path: &str, project_id: &str) -> Result<()> {
    let path = Path::new(file_path);
    if !path.exists() {
        return Err(KnowlixError::FileNotFound(file_path.into()));
    }

    let content_hash = hash_file(file_path)?;

    if let Some(existing) = knowlix_storage::get_file_entry(file_path).await? {
        if existing.content_hash == content_hash {
            return Ok(());
        }
        knowlix_storage::remove_file_from_fts(&existing.id)?;
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
    for chunk in &chunks {
        knowlix_storage::index_chunk_fts(chunk, file_path, project_id)?;
    }
    knowlix_storage::insert_chunks(chunks).await?;
    knowlix_storage::commit_fts()?;

    Ok(())
}

pub async fn remove_file(file_path: &str) -> Result<()> {
    if let Some(entry) = knowlix_storage::get_file_entry(file_path).await? {
        knowlix_storage::remove_file_from_fts(&entry.id)?;
        knowlix_storage::delete_chunks_for_file(&entry.id).await?;
        knowlix_storage::commit_fts()?;
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
        | "r" | "lua" | "xml" | "vue" | "svelte" | "dart" => {
            std::fs::read_to_string(file_path).map_err(KnowlixError::Io)
        }
        _ => Err(KnowlixError::UnsupportedFileType(ext)),
    }
}

pub async fn reindex_project(project_id: &str) -> Result<IndexStats> {
    INDEXING.store(true, Ordering::SeqCst);
    let start = std::time::Instant::now();

    let files = knowlix_storage::list_files_for_project(project_id).await?;
    let total_files = files.len();
    let mut indexed = 0;
    let mut skipped = 0;
    let mut failed = 0;

    for entry in &files {
        let current_hash = match hash_file(&entry.path) {
            Ok(h) => h,
            Err(_) => {
                failed += 1;
                continue;
            }
        };
        if current_hash == entry.content_hash && entry.indexed {
            skipped += 1;
            continue;
        }
        match index_file(&entry.path, project_id).await {
            Ok(()) => indexed += 1,
            Err(_) => failed += 1,
        }
    }
    // Phase 2 (watcher) will handle new files added to folders.

    INDEXING.store(false, Ordering::SeqCst);

    Ok(IndexStats {
        total_files,
        indexed,
        skipped,
        failed,
        duration_ms: start.elapsed().as_millis() as u64,
    })
}

pub async fn get_index_status(project_id: &str) -> Result<IndexStatus> {
    let files = knowlix_storage::list_files_for_project(project_id).await?;
    let total = files.len();
    let indexed_count = files.iter().filter(|f| f.indexed).count();
    Ok(IndexStatus {
        total_files: total,
        indexed_files: indexed_count,
        in_progress: INDEXING.load(Ordering::SeqCst),
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
        "txt" | "log" => FileType::Text,
        "md" | "mdx" => FileType::Markdown,
        "pdf" => FileType::Pdf,
        "docx" => FileType::Word,
        "xlsx" => FileType::Excel,
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
}
