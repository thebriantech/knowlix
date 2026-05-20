import { useState, useCallback, useEffect, useMemo } from 'react';
import { listen } from '@tauri-apps/api/event';
import { api } from '../api';
import type { AiAnswer, AiTier, EmbeddingModelStatus, Project, SearchMode, SearchResult } from '../types';

interface Props {
  projects: Project[];
  selectedProject: Project | null;
  onResultSelect: (result: SearchResult) => void;
  selectedResultId: string | null;
}

function basename(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

function dirname(path: string): string {
  const parts = path.split(/[\\/]/);
  parts.pop();
  return parts.join('/') || '/';
}

function getFileTypeCategory(filePath: string): string {
  const ext = filePath.split('.').pop()?.toLowerCase() ?? '';
  if (['md', 'mdx'].includes(ext)) return 'markdown';
  if (['txt', 'log'].includes(ext)) return 'text';
  if (ext === 'pdf') return 'pdf';
  if (ext === 'docx') return 'word';
  if (ext === 'xlsx') return 'excel';
  if (['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp'].includes(ext)) return 'image';
  return 'code';
}

function renderAnswerMarkdown(md: string): string {
  let html = md
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/```[\w]*\n?([\s\S]*?)```/g, '<pre><code>$1</code></pre>')
    .replace(/`([^`]+)`/g, '<code>$1</code>')
    .replace(/^#### (.+)$/gm, '<h4>$1</h4>')
    .replace(/^### (.+)$/gm, '<h3>$1</h3>')
    .replace(/^## (.+)$/gm, '<h2>$1</h2>')
    .replace(/^# (.+)$/gm, '<h1>$1</h1>')
    .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
    .replace(/\*(.+?)\*/g, '<em>$1</em>')
    .replace(/^---$/gm, '<hr>')
    .replace(/^> (.+)$/gm, '<blockquote>$1</blockquote>')
    .replace(/^[-*] (.+)$/gm, '<li>$1</li>')
    .replace(/\n\n(?!<)/g, '</p><p>');
  html = html.replace(/(<li>.*<\/li>(\n|$))+/g, (m) => `<ul>${m}</ul>`);
  return `<p>${html}</p>`;
}

export function SearchPanel({ projects, selectedProject, onResultSelect, selectedResultId }: Props) {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<SearchResult[]>([]);
  const [searching, setSearching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [searched, setSearched] = useState(false);
  const [searchProjectId, setSearchProjectId] = useState<string | undefined>(
    selectedProject?.id ?? undefined
  );
  const [mode, setMode] = useState<SearchMode>('keyword');
  const [fileTypeFilter, setFileTypeFilter] = useState('');
  const [modelStatus, setModelStatus] = useState<EmbeddingModelStatus>({
    ready: false,
    downloading: false,
  });

  // Ask mode state
  const [aiAnswer, setAiAnswer] = useState<AiAnswer | null>(null);
  const [asking, setAsking] = useState(false);
  const [aiTier, setAiTier] = useState<AiTier>('none');

  useEffect(() => {
    setSearchProjectId(selectedProject?.id ?? undefined);
  }, [selectedProject?.id]);

  useEffect(() => {
    api.getEmbeddingModelStatus().then(setModelStatus).catch(() => {});
  }, []);

  useEffect(() => {
    api.getAiTier().then(setAiTier).catch(() => {});
  }, []);

  useEffect(() => {
    let unlisten1: (() => void) | null = null;
    let unlisten2: (() => void) | null = null;
    let unlisten3: (() => void) | null = null;

    Promise.all([
      listen('embedding_model_downloading', () =>
        setModelStatus({ ready: false, downloading: true })
      ).then(fn => { unlisten1 = fn; }),
      listen('embedding_model_ready', () =>
        setModelStatus({ ready: true, downloading: false })
      ).then(fn => { unlisten2 = fn; }),
      listen<string>('embedding_model_error', ev =>
        setModelStatus({ ready: false, downloading: false, error: ev.payload })
      ).then(fn => { unlisten3 = fn; }),
    ]);

    return () => {
      unlisten1?.();
      unlisten2?.();
      unlisten3?.();
    };
  }, []);

  const doSearch = useCallback(async () => {
    const q = query.trim();
    if (!q) return;

    if (mode === 'ask') {
      setAsking(true);
      setAiAnswer(null);
      setError(null);
      try {
        const answer = await api.answerQuestion(q, searchProjectId);
        setAiAnswer(answer);
      } catch (e) {
        setError(String(e));
      } finally {
        setAsking(false);
      }
      return;
    }

    setSearching(true);
    setError(null);
    setSearched(true);
    try {
      let res: SearchResult[];
      if (mode === 'keyword') {
        res = await api.searchKeyword(q, searchProjectId, 20);
      } else if (mode === 'semantic') {
        res = await api.searchSemantic(q, searchProjectId, 20);
      } else {
        res = await api.search(q, searchProjectId, 20);
      }
      setResults(res);
    } catch (e) {
      setError(String(e));
      setResults([]);
    } finally {
      setSearching(false);
    }
  }, [query, searchProjectId, mode]);

  const handleDownloadModel = useCallback(async () => {
    try {
      await api.ensureEmbeddingModel();
    } catch (e) {
      setModelStatus(s => ({ ...s, error: String(e) }));
    }
  }, []);

  const needsModel = (mode === 'semantic' || mode === 'hybrid') && !modelStatus.ready;

  const projectMap = useMemo(() => {
    const m = new Map<string, string>();
    projects.forEach(p => m.set(p.id, p.name));
    return m;
  }, [projects]);

  const filteredResults = useMemo(() => {
    if (!fileTypeFilter) return results;
    return results.filter(r => getFileTypeCategory(r.file_path) === fileTypeFilter);
  }, [results, fileTypeFilter]);

  const displayGroups = useMemo(() => {
    if (searchProjectId !== undefined || filteredResults.length === 0) return null;
    const groupMap = new Map<string, SearchResult[]>();
    for (const r of filteredResults) {
      const g = groupMap.get(r.project_id) ?? [];
      g.push(r);
      groupMap.set(r.project_id, g);
    }
    return Array.from(groupMap.entries()).map(([pid, res]) => ({
      projectId: pid,
      projectName: projectMap.get(pid) ?? pid,
      results: res,
    }));
  }, [filteredResults, searchProjectId, projectMap]);

  function renderResult(r: SearchResult) {
    return (
      <div
        key={r.chunk_id}
        className={`result-item${selectedResultId === r.chunk_id ? ' selected' : ''}`}
        onClick={() => onResultSelect(r)}
      >
        <div className="result-filename">{basename(r.file_path)}</div>
        <div className="result-path">{dirname(r.file_path)}</div>
        <div className="result-snippet">{r.snippet}</div>
        <div className="result-meta">
          <span className="result-source">{r.source.toUpperCase()}</span>
          <span className="result-score">{r.score.toFixed(3)}</span>
        </div>
      </div>
    );
  }

  const totalFiltered = filteredResults.length;
  const isAskMode = mode === 'ask';

  const searchModes: Array<{ key: SearchMode; label: string }> = [
    { key: 'keyword', label: 'Keyword' },
    { key: 'semantic', label: 'Semantic' },
    { key: 'hybrid', label: 'Hybrid' },
    { key: 'ask', label: 'Ask' },
  ];

  return (
    <div className="search-panel">
      <div className="search-header">
        {!isAskMode && (
          <div className="search-filters-row">
            <select
              className="search-filter-select"
              value={searchProjectId ?? ''}
              onChange={e => setSearchProjectId(e.target.value || undefined)}
            >
              <option value="">All projects</option>
              {projects.map(p => (
                <option key={p.id} value={p.id}>{p.name}</option>
              ))}
            </select>

            <select
              className="search-filter-select"
              value={fileTypeFilter}
              onChange={e => setFileTypeFilter(e.target.value)}
            >
              <option value="">All types</option>
              <option value="code">Code</option>
              <option value="markdown">Markdown</option>
              <option value="text">Text</option>
              <option value="pdf">PDF</option>
              <option value="word">Word</option>
              <option value="excel">Excel</option>
              <option value="image">Image</option>
            </select>
          </div>
        )}

        {isAskMode && (
          <div className="search-filters-row">
            <select
              className="search-filter-select"
              value={searchProjectId ?? ''}
              onChange={e => setSearchProjectId(e.target.value || undefined)}
            >
              <option value="">All projects</option>
              {projects.map(p => (
                <option key={p.id} value={p.id}>{p.name}</option>
              ))}
            </select>
          </div>
        )}

        <div className="search-mode-toggle">
          {searchModes.map(m => (
            <button
              key={m.key}
              className={`mode-btn${mode === m.key ? ' active' : ''}`}
              onClick={() => {
                setMode(m.key);
                setAiAnswer(null);
                setError(null);
              }}
            >
              {m.label}
            </button>
          ))}
        </div>

        {needsModel && (
          <div className="model-download-banner">
            {modelStatus.downloading ? (
              <span className="model-status-downloading">Downloading model…</span>
            ) : modelStatus.error ? (
              <>
                <span className="model-status-error">Download failed: {modelStatus.error}</span>
                <button className="btn btn-sm" onClick={handleDownloadModel}>Retry</button>
              </>
            ) : (
              <>
                <span>Semantic search requires the embedding model (~25 MB)</span>
                <button className="btn btn-sm btn-primary" onClick={handleDownloadModel}>
                  Download
                </button>
              </>
            )}
          </div>
        )}

        <div className="search-input-row">
          <input
            className="search-input"
            value={query}
            onChange={e => setQuery(e.target.value)}
            onKeyDown={e => e.key === 'Enter' && doSearch()}
            placeholder={
              isAskMode
                ? 'Ask a question about your files…'
                : 'Search files…'
            }
            autoFocus
          />
          <button
            className="btn btn-primary"
            onClick={doSearch}
            disabled={
              isAskMode
                ? asking || !query.trim() || aiTier === 'none'
                : searching || !query.trim() || (needsModel && !modelStatus.ready)
            }
          >
            {isAskMode ? (asking ? '…' : 'Ask') : (searching ? '…' : 'Search')}
          </button>
        </div>
      </div>

      {error && <div className="error-msg">{error}</div>}

      {/* Ask mode result */}
      {isAskMode && (
        <div className="search-results" style={{ padding: 0 }}>
          {aiTier === 'none' && !asking && !aiAnswer && (
            <div className="ask-notice">
              Enable Ollama in AI Settings (gear icon at bottom of sidebar) to use Ask mode.
            </div>
          )}
          {asking && (
            <div className="loading" style={{ padding: 24 }}>
              Thinking…
            </div>
          )}
          {aiAnswer && (
            <div style={{ padding: '0 0 16px' }}>
              <div
                className="viewer-markdown"
                style={{ padding: '16px', maxWidth: 'none', height: 'auto', overflow: 'visible' }}
                dangerouslySetInnerHTML={{ __html: renderAnswerMarkdown(aiAnswer.answer) }}
              />
              {aiAnswer.sources.length > 0 && (
                <>
                  <div style={{
                    padding: '8px 12px',
                    fontSize: 11,
                    fontWeight: 600,
                    textTransform: 'uppercase',
                    color: 'var(--text-muted)',
                    borderTop: '1px solid var(--panel-border)',
                    marginTop: 8,
                  }}>
                    Sources ({aiAnswer.sources.length})
                  </div>
                  {aiAnswer.sources.map(r => renderResult(r))}
                </>
              )}
            </div>
          )}
        </div>
      )}

      {/* Normal search results */}
      {!isAskMode && (
        <>
          {searched && !searching && (
            <div className="search-results-header">
              {totalFiltered > 0
                ? `${totalFiltered} result${totalFiltered !== 1 ? 's' : ''}`
                : 'No results'}
            </div>
          )}

          <div className="search-results">
            {filteredResults.length > 0
              ? displayGroups
                ? displayGroups.map(group => (
                    <div key={group.projectId} className="result-group">
                      <div className="result-group-header">{group.projectName}</div>
                      {group.results.map(r => renderResult(r))}
                    </div>
                  ))
                : filteredResults.map(r => renderResult(r))
              : !searching && (
                  <div className="empty-state">
                    <div className="empty-state-icon">🔍</div>
                    <div className="empty-state-title">
                      {searched ? 'No results found' : 'Search your files'}
                    </div>
                    <div className="empty-state-desc">
                      {searched
                        ? 'Try different keywords or reindex the project.'
                        : 'Type a query and press Enter.'}
                    </div>
                  </div>
                )}
          </div>
        </>
      )}
    </div>
  );
}
