# Roadmap

Phases are sequential. Do not implement Phase N+1 features while Phase N is incomplete.

---

## Phase 1 — MVP (Core Search)

Goal: usable local search for a single project.

- [ ] Project management (create, list, delete, add/remove folders)
- [ ] Manual file indexing (text + markdown + code)
- [ ] BM25 keyword search via tantivy
- [ ] Basic file viewer (code with syntax highlight, markdown rendered)
- [ ] Result list with snippets and file path

Milestone: user can add a project folder and search across all text/code files.

---



## Phase 2 — File Watcher + Incremental Index

Goal: index stays fresh automatically.

- [ ] File watcher (notify crate) — detect create/modify/delete
- [ ] Incremental indexing (SHA256 change detection, skip unchanged)
- [ ] PDF indexing + viewer (pdf.js + text extraction)
- [ ] DOCX indexing + viewer (mammoth.js)
- [ ] Image viewer (no indexing, view only)
- [ ] IndexStatus UI (progress bar, last indexed time)

Milestone: user adds files to folder and they appear in search within seconds.

---

## Phase 3 — Semantic Search (Tier 0)

Goal: semantic search without any API key.

- [ ] fastembed-rs integration (local embedding model, download on first use)
- [ ] sqlite-vec integration (vector storage + similarity search)
- [ ] Hybrid search: BM25 + vector + RRF merge
- [ ] ~900 token chunking with boundary detection and overlap
- [ ] Search mode toggle: keyword / semantic / hybrid
- [ ] Embedding model download UX (progress, size warning)

Milestone: user gets semantic results "find all docs about authentication" without API key.

---

## Phase 4 — Multi-project + Global Search

Goal: search across all projects simultaneously.

- [ ] Cross-project search (project_id = None)
- [ ] Search result grouping by project
- [ ] Project filter in search UI
- [ ] Excel (.xlsx) indexing + viewer (SheetJS)
- [ ] File type filter in search UI

Milestone: user can search "deployment config" across all their projects at once.

---

## Phase 5 — Local LLM (Tier 1)

Goal: wiki and Q&A via local Ollama, fully offline.

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
