# Rules

Hard constraints. These apply to ALL code in this repo. Do not violate.

---

## Architecture

- DO NOT introduce external services (Redis, Kafka, RabbitMQ, Elasticsearch, etc.)
- DO NOT introduce a separate backend process — everything runs inside Tauri
- DO NOT couple modules directly — all cross-module calls go through defined contracts in CONTRACTS.md
- DO NOT change function signatures in CONTRACTS.md without updating all callers and the doc
- DO NOT access SQLite or tantivy directly from the UI layer — always go through storage module

## Threading

- DO NOT block the main thread
- ALL indexing must run in async tokio tasks
- ALL file watching runs in a background thread
- ALL LLM/API calls must be async and cancellable
- UI events must remain responsive during heavy background work

## AI

- DO NOT make any network call without explicit user configuration (AiProvider != None)
- DO NOT send full file contents to external API — only chunks, max 4096 tokens per request
- DO NOT hardcode any API provider URLs — use AiConfig.api_base_url
- Wiki generation must be user-triggered, never automatic
- All AI features must degrade gracefully to Tier 0 behavior when AiProvider == None

## Storage

- ALL persistent state lives under ~/.knowlix/
- DO NOT write to project folders (read-only access to user data)
- Wiki pages cached by SHA256 of source chunks — never re-generate unchanged content
- Embedding model files stored in ~/.knowlix/cache/embeddings/ — never bundled in app

## Code Quality

- Every public function in a module must match its signature in CONTRACTS.md exactly
- Errors must use Result<T, KnowlixError> — no panics in library code
- No unwrap() in production paths — use ? operator or explicit error handling
- Each module lives in its own crate under core/ — no circular dependencies

## Dependencies

- DO NOT add dependencies without checking for existing alternatives in Cargo.toml
- Prefer pure-Rust crates over FFI bindings where performance allows
- Exceptions requiring FFI: pdfium-render (PDF), fastembed-rs (embeddings)

## File Access

- Indexer has READ-ONLY access to project folders
- Watcher has READ-ONLY access to project folders
- Viewer has READ-ONLY access to project folders
- No module writes to user project folders under any circumstance
