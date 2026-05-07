# Data Model

## Entities

### Project
```
id:          String (UUID)
name:        String
description: Option<String>
folders:     Vec<String>       -- absolute paths
created_at:  DateTime<Utc>
updated_at:  DateTime<Utc>
```

### FileEntry
```
id:           String (UUID)
project_id:   String
path:         String            -- absolute path
file_type:    FileType
language:     Option<String>    -- for code files (rust, python, ts, ...)
size_bytes:   i64
content_hash: String            -- SHA256, used for change detection
last_indexed: DateTime<Utc>
indexed:      bool
```

FileType enum:
```
Text        -- .txt, .log
Markdown    -- .md, .mdx
Code        -- .rs, .py, .ts, .js, .go, .java, .c, .cpp, .html, .css, ...
Pdf         -- .pdf
Word        -- .docx
Excel       -- .xlsx
Image       -- .png, .jpg, .jpeg, .gif, .svg, .webp
Unknown
```

### Chunk
```
id:         String (UUID)
file_id:    String
chunk_index: i32               -- order within file
content:    String             -- raw text of this chunk
token_count: i32               -- approx token count
start_byte: i64                -- byte offset in original file
end_byte:   i64
```

Chunking strategy:
- Target size: ~900 tokens
- Overlap: 100 tokens between adjacent chunks
- Boundary detection: respect paragraph breaks, code blocks, headings
- Never split mid-sentence

### EmbeddingRecord
```
chunk_id:   String             -- FK → Chunk.id
model:      String             -- embedding model name used
vector:     Vec<f32>           -- stored in sqlite-vec
created_at: DateTime<Utc>
```

### WikiPage
```
id:           String (UUID)
project_id:   Option<String>   -- None = global wiki page
title:        String
content:      String           -- markdown
tags:         Vec<String>
source_hashes: Vec<String>     -- SHA256 of source chunks used
created_at:   DateTime<Utc>
updated_at:   DateTime<Utc>
```

Global wiki page has project_id = None and may reference multiple projects.

### AiConfig
```
provider:      AiProvider
ollama_model:  Option<String>  -- e.g. "phi3.5", "qwen2.5:7b"
ollama_url:    String          -- default "http://localhost:11434"
api_key:       Option<String>  -- encrypted at rest
api_base_url:  String          -- OpenAI-compatible endpoint
api_model:     Option<String>  -- e.g. "gpt-4o", "claude-3-5-sonnet"
```

AiProvider enum:
```
None
Ollama
Api
```

---

## Search Types

### SearchResult
```
file_id:    String
file_path:  String
chunk_id:   String
snippet:    String             -- ~300 chars, matches highlighted
score:      f32                -- normalized 0.0–1.0
rank_bm25:  Option<i32>
rank_vec:   Option<i32>
source:     SearchSource       -- BM25 | Vector | Hybrid
```

SearchSource enum:
```
Bm25
Vector
Hybrid                         -- RRF merged
```

### AiAnswer
```
answer:     String             -- LLM-generated markdown
sources:    Vec<SearchResult>  -- chunks used as context
model:      String             -- which model generated this
query:      String             -- original user query
```

---

## Storage Layout

```
~/.knowlix/
  config.toml          -- app config, AiConfig
  projects.db          -- SQLite: Project, FileEntry, Chunk, WikiPage, AiConfig
  index/
    tantivy/           -- tantivy FTS index
    vectors.db         -- sqlite-vec database
  cache/
    embeddings/        -- fastembed model files
    thumbnails/        -- image thumbnails (optional)
```
