import { useState, useCallback, useEffect, useMemo } from 'react';
import { listen } from '@tauri-apps/api/event';
import { api } from '../api';
import type { Project, SearchResult, EmbeddingModelStatus, SearchMode } from '../types';

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

  useEffect(() => {
    setSearchProjectId(selectedProject?.id ?? undefined);
  }, [selectedProject?.id]);

  useEffect(() => {
    api.getEmbeddingModelStatus().then(setModelStatus).catch(() => {});
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

  const needsModel = mode !== 'keyword' && !modelStatus.ready;

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

  return (
    <div className="search-panel">
      <div className="search-header">
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
          {totalFiltered > 0 ? `${totalFiltered} result${totalFiltered !== 1 ? 's' : ''}` : 'No results'}
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
    </div>
  );
}
