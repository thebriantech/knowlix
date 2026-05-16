use std::path::Path;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use knowlix_common::{KnowlixError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ViewContent {
    Code { content: String, language: String },
    Markdown { content: String },
    Html { content: String },
    Image { data_uri: String, mime: String },
    PlainText { content: String },
    Pdf { data: String },
    Docx { data: String },
}

pub async fn get_view_content(file_path: &str) -> Result<ViewContent> {
    let path = Path::new(file_path);
    if !path.exists() {
        return Err(KnowlixError::FileNotFound(file_path.into()));
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "md" | "mdx" => {
            let content = std::fs::read_to_string(file_path)?;
            Ok(ViewContent::Markdown { content })
        }
        "txt" | "log" => {
            let content = std::fs::read_to_string(file_path)?;
            Ok(ViewContent::PlainText { content })
        }
        "png" | "jpg" | "jpeg" | "gif" | "webp" => {
            let mime = image_mime(&ext);
            let data = std::fs::read(file_path)?;
            let encoded = STANDARD.encode(&data);
            Ok(ViewContent::Image {
                data_uri: format!("data:{};base64,{}", mime, encoded),
                mime: mime.to_string(),
            })
        }
        "svg" => {
            let content = std::fs::read_to_string(file_path)?;
            let encoded = STANDARD.encode(content.as_bytes());
            Ok(ViewContent::Image {
                data_uri: format!("data:image/svg+xml;base64,{}", encoded),
                mime: "image/svg+xml".to_string(),
            })
        }
        "pdf" => {
            let data = std::fs::read(file_path)?;
            Ok(ViewContent::Pdf { data: STANDARD.encode(&data) })
        }
        "docx" => {
            let data = std::fs::read(file_path)?;
            Ok(ViewContent::Docx { data: STANDARD.encode(&data) })
        }
        _ => {
            // Treat as code
            let content = std::fs::read_to_string(file_path).map_err(|_| {
                KnowlixError::UnsupportedFileType(format!("Cannot read binary file: .{}", ext))
            })?;
            let language = detect_language(&ext);
            Ok(ViewContent::Code {
                content,
                language: language.to_string(),
            })
        }
    }
}

fn image_mime(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "image/png",
    }
}

fn detect_language(ext: &str) -> &'static str {
    match ext {
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
    use tempfile::NamedTempFile;

    fn write_temp(ext: &str, content: &[u8]) -> NamedTempFile {
        let f = NamedTempFile::with_suffix(&format!(".{}", ext)).unwrap();
        std::fs::write(f.path(), content).unwrap();
        f
    }

    #[tokio::test]
    async fn test_view_markdown() {
        let f = write_temp("md", b"# Hello\nWorld");
        let result = get_view_content(f.path().to_str().unwrap()).await.unwrap();
        assert!(matches!(result, ViewContent::Markdown { content } if content.contains("Hello")));
    }

    #[tokio::test]
    async fn test_view_plain_text() {
        let f = write_temp("txt", b"plain text content");
        let result = get_view_content(f.path().to_str().unwrap()).await.unwrap();
        assert!(matches!(result, ViewContent::PlainText { content } if content.contains("plain")));
    }

    #[tokio::test]
    async fn test_view_rust_code() {
        let f = write_temp("rs", b"fn main() {}");
        let result = get_view_content(f.path().to_str().unwrap()).await.unwrap();
        match result {
            ViewContent::Code { content, language } => {
                assert_eq!(language, "rust");
                assert!(content.contains("fn main"));
            }
            _ => panic!("Expected Code variant"),
        }
    }

    #[tokio::test]
    async fn test_view_typescript() {
        let f = write_temp("ts", b"const x: number = 42;");
        let result = get_view_content(f.path().to_str().unwrap()).await.unwrap();
        assert!(matches!(result, ViewContent::Code { language, .. } if language == "typescript"));
    }

    #[tokio::test]
    async fn test_view_json() {
        let f = write_temp("json", b"{\"key\": \"value\"}");
        let result = get_view_content(f.path().to_str().unwrap()).await.unwrap();
        assert!(matches!(result, ViewContent::Code { language, .. } if language == "json"));
    }

    #[tokio::test]
    async fn test_view_missing_file() {
        let err = get_view_content("/nonexistent/path/file.txt").await.unwrap_err();
        assert!(matches!(err, KnowlixError::FileNotFound(_)));
    }

    #[tokio::test]
    async fn test_view_pdf_returns_data() {
        let f = write_temp("pdf", b"%PDF-1.4 test content");
        let result = get_view_content(f.path().to_str().unwrap()).await.unwrap();
        assert!(matches!(result, ViewContent::Pdf { .. }));
    }

    #[tokio::test]
    async fn test_view_docx_returns_data() {
        let f = write_temp("docx", b"PK fake docx bytes");
        let result = get_view_content(f.path().to_str().unwrap()).await.unwrap();
        assert!(matches!(result, ViewContent::Docx { .. }));
    }

    #[test]
    fn test_detect_language() {
        assert_eq!(detect_language("rs"), "rust");
        assert_eq!(detect_language("py"), "python");
        assert_eq!(detect_language("unknown"), "plaintext");
    }

    #[test]
    fn test_image_mime() {
        assert_eq!(image_mime("png"), "image/png");
        assert_eq!(image_mime("jpg"), "image/jpeg");
        assert_eq!(image_mime("jpeg"), "image/jpeg");
        assert_eq!(image_mime("gif"), "image/gif");
        assert_eq!(image_mime("webp"), "image/webp");
    }
}
