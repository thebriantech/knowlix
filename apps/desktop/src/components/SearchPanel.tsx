import { useState, useCallback, useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { api } from '../api';
import type { Project, SearchResult, EmbeddingModelStatus, SearchMode } from '../types';

interface Props {
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

export function SearchPanel({ selectedProject, onResultSelect, selectedResultId }: Props) {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<SearchResult[]>([]);
  const [searching, setSearching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [searched, setSearched] = useState(false);
  const [scopeAll, setScopeAll] = useState(false);
  const [mode, setMode] = useState<SearchMode>('keyword');
  const [modelStatus, setModelStatus] = useState<EmbeddingModelStatus>({
    ready: false,
    downloading: false,
  });
  useEffect(() => {
    if (!selectedProject) setScopeAll(true);
  }, [selectedProject]);

  // Load initial model status
  useEffect(() => {
    api.getEmbeddingModelStatus().then(setModelStatus).catch(() => {});
  }, []);

  // Subscribe to model download events
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

  const effectiveProjectId = scopeAll ? undefined : selectedProject?.id;

  const doSearch = useCallback(async () => {
    const q = query.trim();
    if (!q) return;
    setSearching(true);
    setError(null);
    setSearched(true);
    try {
      let res: SearchResult[];
      if (mode === 'keyword') {
        res = await api.searchKeyword(q, effectiveProjectId, 20);
      } else if (mode === 'semantic') {
        res = await api.searchSemantic(q, effectiveProjectId, 20);
      } else {
        res = await api.search(q, effectiveProjectId, 20);
      }
      setResults(res);
    } catch (e) {
      setError(String(e));
      setResults([]);
    } finally {
      setSearching(false);
    }
  }, [query, effectiveProjectId, mode]);

  const handleDownloadModel = useCallback(async () => {
    try {
      await api.ensureEmbeddingModel();
    } catch (e) {
      setModelStatus(s => ({ ...s, error: String(e) }));
    }
  }, []);

  const needsModel = mode !== 'keyword' && !modelStatus.ready;

  return (
    <div className="search-panel">
      <div className="search-header">
        <div className="search-scope-toggle">
          <button
            className={`scope-btn${!scopeAll ? ' active' : ''}`}
            onClick={() => setScopeAll(false)}
            disabled={!selectedProject}
            title={selectedProject ? selectedProject.name : 'No project selected'}
          >
            This project
          </button>
          <button
            className={`scope-btn${scopeAll ? ' active' : ''}`}
            onClick={() => setScopeAll(true)}
          >
            All projects
          </button>
        </div>

        <div className="search-mode-toggle">
          {(['keyword', 'semantic', 'hybrid'] as SearchMode[]).map(m => (
            <button
              key={m}
              className={`mode-btn${mode === m ? ' active' : ''}`}
              onClick={() => setMode(m)}
            >
              {m.charAt(0).toUpperCase() + m.slice(1)}
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
            placeholder="Search files…"
            autoFocus
          />
          <button
            className="btn btn-primary"
            onClick={doSearch}
            disabled={searching || !query.trim() || (needsModel && !modelStatus.ready)}
          >
            {searching ? '…' : '🔍'}
          </button>
        </div>
      </div>

      {error && <div className="error-msg">{error}</div>}

      {searched && !searching && (
        <div className="search-results-header">
          {results.length > 0 ? `${results.length} results` : 'No results'}
        </div>
      )}

      <div className="search-results">
        {results.length > 0
          ? results.map(r => (
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
            ))
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
    </div>
  );
}
