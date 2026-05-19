# Roadmap

Phases are sequential. Do not implement Phase N+1 features while Phase N is incomplete.

---

## Phase 1 — MVP (Core Search) ✅ COMPLETE

Goal: usable local search for a single project.

- [x] Project management (create, list, delete, add/remove folders)
- [x] Manual file indexing (text + markdown + code)
- [x] BM25 keyword search via tantivy
- [x] Basic file viewer (code with syntax highlight, markdown rendered)
- [x] Result list with snippets and file path

Milestone: user can add a project folder and search across all text/code files.

---

## Phase 2 — File Watcher + Incremental Index ✅ COMPLETE

Goal: index stays fresh automatically.

- [x] File watcher (notify create) — detect create/modify/delete
- [x] Incremental indexing (SHA256 change detection, skip unchanged)
- [x] PDF indexing (pdf_extract crate — text extraction for search)
- [x] PDF viewer (render via pdfjs-dist)
- [x] DOCX / ODT / ODP indexing (zip XML extraction)
- [x] DOCX viewer (render via docx-preview)
- [x] Excel / PPTX indexing (calamine + zip XML extraction)
- [x] Image viewer (no indexing, view only)
- [x] Index detail popup — per-file status (indexed / skipped / failed) with failure reasons, filter tabs
- [x] Real-time progress during indexing (Tauri events — progress bar, current file)
- [x] Last indexed time display per project

Milestone: user adds files to folder and they appear in search within seconds.

---

## Phase 3 — Semantic Search (Tier 0) ✅ COMPLETE

Goal: semantic search without any API key.

- [x] fastembed-rs integration (local embedding model, download on first use)
- [x] Vector storage + cosine similarity search (BLOB storage in SQLite + Rust cosine sim; sqlite-vec migration optional future work)
- [x] Hybrid search: BM25 + vector + RRF merge
- [x] ~900 token chunking with boundary detection and overlap (implemented in `core/indexer` — 3600 chars ≈ 900 tokens, 400-char overlap, newline-boundary alignment)
- [x] Search mode toggle: keyword / semantic / hybrid
- [x] Embedding model download UX (progress, size warning ~25 MB)

Milestone: user gets semantic results "find all docs about authentication" without API key.

---

## Phase 4 — Multi-project + Global Search ✅ COMPLETE

Goal: search across all projects simultaneously.

- [x] Cross-project search backend (project_id = None) — tantivy query already filters by project or searches all; UI has "This project / All projects" scope toggle in SearchPanel
- [x] Search result grouping by project (results grouped by project name when "All projects" selected)
- [x] Project filter dropdown in search UI (select dropdown replaces toggle — lists all projects)
- [x] Excel (.xlsx) viewer (SheetJS) — renders sheets as tables, multi-sheet tabs
- [x] File type filter in search UI (dropdown: All / Code / Markdown / Text / PDF / Word / Excel / Image)

Milestone: user can search "deployment config" across all their projects at once.

---

## Phase 5 — Local LLM (Tier 1)

Goal: wiki and Q&A via local Ollama, fully offline.

Storage infrastructure ready: `ai_config` table, `wiki_pages` table, `AiConfig` / `WikiPage` models, wiki and ai_agent command scaffolding — all in place. Core logic not yet implemented.

- [ ] Ollama provider integration (detect, health check, model list)
- [ ] Query expansion (rewrite query into variants before search)
- [ ] Natural language Q&A with RAG (retrieve chunks → Ollama → answer)
- [ ] Per-project wiki generation (user-triggered)
- [ ] Global wiki generation (from project wikis)
- [ ] Wiki viewer UI
- [ ] AI settings UI (provider selection, model picker, health status)

Milestone: user with Ollama installed gets natural language answers and auto-generated wiki.

---

## Phase 6 — API Key (Tier 2)

Goal: higher quality Q&A and wiki via external LLM APIs.

Storage infrastructure ready: `AiConfig` model already has `api_key`, `api_base_url`, `api_model` fields persisted in SQLite.

- [ ] ApiProvider (OpenAI-compatible, configurable base URL)
- [ ] API key storage (encrypted at rest)
- [ ] Model selection (gpt-4o, claude-sonnet, custom)
- [ ] Quality comparison UX (optional: show Tier 1 vs Tier 2 answer)
- [ ] Token usage tracking and cost estimation display

Milestone: user with OpenAI/Anthropic key gets best-quality Q&A.

---

## Out of Scope (do not implement)

- Cloud sync or remote storage
- Collaboration features
- Mobile app
- Browser extension
- Real-time collaboration
- Any auto-wiki generation triggered by indexing (wiki is always user-triggered)
