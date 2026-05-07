# Knowlix Architecture

## Overview

Knowlix is a local-first desktop application for project-centric knowledge management, search, and Q&A.

```
Local project folders → Index → Search → View → (Optional) AI-enhanced Q&A
```

## Core Principles

- **Local-first**: All core features work fully offline, zero cloud dependency
- **Project-scoped**: Knowledge is organized per project, with optional cross-project views
- **Non-blocking**: All indexing and heavy processing run in background threads
- **AI is optional**: Full search and viewing without any API key
- **Privacy tiers**: User controls what leaves their machine

---

## AI Tiers

```
Tier 0 — No AI (default)
  - BM25 keyword search
  - Local semantic search (fastembed-rs, runs on device)
  - Document viewer
  - No API key required

Tier 1 — Local LLM (Ollama)
  - All Tier 0 features
  - Per-project wiki generation
  - Global wiki across projects
  - Natural language Q&A (RAG)
  - Query expansion
  - 100% offline, data never leaves machine
  - Requires: Ollama installed and running

Tier 2 — API Key
  - All Tier 1 features
  - Higher quality answers and wiki
  - Supports: OpenAI, Anthropic, any OpenAI-compatible endpoint
  - Chunks sent to external API
```

---

## System Components

```
┌─────────────────────────────────────────────────────────┐
│                        UI Layer                         │
│              (Tauri v2 + React + TypeScript)             │
└────────────┬──────────────────────────────┬─────────────┘
             │                              │
    ┌────────▼────────┐          ┌──────────▼──────────┐
    │  Project Manager│          │    Viewer Module     │
    │                 │          │  (code/pdf/image/doc)│
    └────────┬────────┘          └─────────────────────┘
             │
    ┌────────▼────────┐
    │  File Watcher   │
    └────────┬────────┘
             │
    ┌────────▼────────┐
    │    Indexer      │◄─── text extraction + chunking
    │                 │◄─── local embeddings (fastembed-rs)
    └────────┬────────┘
             │
    ┌────────▼────────┐
    │  Storage Layer  │
    │  SQLite (meta)  │
    │  tantivy (FTS)  │
    │  sqlite-vec     │
    └────────┬────────┘
             │
    ┌────────▼────────┐
    │  Search Engine  │
    │  BM25 + Vector  │
    │  RRF merge      │
    └────────┬────────┘
             │
    ┌────────▼────────┐     ┌─────────────────────────┐
    │   Wiki Module   │     │      AI Agent Module     │
    │  project wiki   │◄────│  OllamaProvider          │
    │  global wiki    │     │  ApiProvider             │
    └─────────────────┘     │  (OpenAI-compatible)     │
                            └─────────────────────────┘
```

---

## Data Flow

### Indexing
```
User adds folder
→ Watcher registers path
→ Watcher detects file (new/modified)
→ Indexer extracts text (by file type)
→ Indexer chunks text (~900 tokens, boundary-aware)
→ Indexer generates local embeddings (fastembed-rs)
→ Store: metadata → SQLite, FTS → tantivy, vectors → sqlite-vec
```

### Search (Tier 0)
```
User submits query
→ Search Engine runs BM25 (tantivy)
→ Search Engine runs vector similarity (sqlite-vec)
→ RRF merges both ranked lists
→ Return SearchResult list with snippets
```

### Search + Q&A (Tier 1/2)
```
User submits natural language query
→ AI Agent expands query into variants
→ Search Engine runs hybrid search for each variant
→ Top-k chunks retrieved
→ AI Agent queries LLM with chunks as context
→ Return structured answer + source citations
```

### Wiki Generation (Tier 1/2, user-triggered)
```
User requests wiki for project
→ AI Agent reads project chunks in batches
→ LLM generates wiki pages (entities, concepts, cross-refs)
→ Pages stored in SQLite (cached by SHA256)
→ On global wiki request: LLM reads all project wikis → generates cross-project wiki
```

---

## Tech Stack

| Layer | Technology |
|---|---|
| Desktop shell | Tauri v2 |
| Frontend | React + TypeScript |
| Code viewer | CodeMirror 6 |
| PDF viewer | pdf.js |
| DOCX viewer | mammoth.js |
| Backend language | Rust |
| Full-text search | tantivy |
| Local embeddings | fastembed-rs |
| Vector storage | sqlite-vec |
| Metadata storage | SQLite (sqlx) |
| Local LLM | Ollama (detected, not bundled) |
| File watching | notify (Rust crate) |

---

## Non-functional Requirements

- Search latency < 100ms (Tier 0)
- Indexing runs in background threads, never blocks UI
- App works fully offline (Tier 0)
- No external network calls without explicit user consent
- Embedding model downloaded once on first semantic search use, cached locally
- Wiki pages cached by SHA256 of source content — no redundant LLM calls
- Installer size < 30MB (embedding model downloaded separately)
