import { useState } from 'react';
import type { IndexStats, IndexFileStatus } from '../types';

interface Props {
  stats: IndexStats;
  projectName: string;
  onClose: () => void;
}

type Filter = 'all' | IndexFileStatus;

function basename(path: string): string {
  return path.split(/[/\\]/).pop() ?? path;
}

const STATUS_ORDER: Record<IndexFileStatus, number> = { failed: 0, removed: 1, indexed: 2, skipped: 3 };

export function IndexDetailModal({ stats, projectName, onClose }: Props) {
  const [filter, setFilter] = useState<Filter>('all');

  const counts = {
    indexed: stats.file_results.filter(r => r.status === 'indexed').length,
    skipped: stats.file_results.filter(r => r.status === 'skipped').length,
    failed: stats.file_results.filter(r => r.status === 'failed').length,
    removed: stats.file_results.filter(r => r.status === 'removed').length,
  };

  const displayed = (filter === 'all' ? stats.file_results : stats.file_results.filter(r => r.status === filter))
    .slice()
    .sort((a, b) => STATUS_ORDER[a.status] - STATUS_ORDER[b.status]);

  const FILTERS: { key: Filter; label: string; count: number }[] = [
    { key: 'all', label: 'All', count: stats.file_results.length },
    { key: 'failed', label: 'Failed', count: counts.failed },
    { key: 'removed', label: 'Removed', count: counts.removed },
    { key: 'indexed', label: 'Indexed', count: counts.indexed },
    { key: 'skipped', label: 'Skipped', count: counts.skipped },
  ];

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal" onClick={e => e.stopPropagation()}>
        <div className="modal-header">
          <div>
            <div className="modal-title">Index Results</div>
            <div className="modal-subtitle">{projectName}</div>
          </div>
          <button className="modal-close" onClick={onClose}>×</button>
        </div>

        <div className="modal-stats">
          <span className="modal-stat indexed">✓ {counts.indexed} indexed</span>
          <span className="modal-stat skipped">⟳ {counts.skipped} skipped</span>
          <span className={`modal-stat removed${counts.removed === 0 ? ' zero' : ''}`}>
            − {counts.removed} removed
          </span>
          <span className={`modal-stat failed${counts.failed === 0 ? ' zero' : ''}`}>
            ✗ {counts.failed} failed
          </span>
          <span className="modal-stat time">{stats.duration_ms}ms</span>
        </div>

        <div className="modal-filter">
          {FILTERS.map(({ key, label, count }) => (
            <button
              key={key}
              className={`modal-filter-btn${filter === key ? ' active' : ''}${key !== 'all' ? ` ${key}` : ''}`}
              onClick={() => setFilter(key)}
            >
              {label} <span className="modal-filter-count">{count}</span>
            </button>
          ))}
        </div>

        <div className="modal-file-list">
          {displayed.length === 0 && (
            <div className="modal-empty">No files in this category.</div>
          )}
          {displayed.map((r, i) => (
            <div key={i} className={`modal-file-item ${r.status}`}>
              <span className={`file-status-badge ${r.status}`}>
                {r.status === 'indexed' ? '✓' : r.status === 'skipped' ? '⟳' : r.status === 'removed' ? '−' : '✗'}
              </span>
              <div className="modal-file-info">
                <div className="modal-file-name">{basename(r.path)}</div>
                <div className="modal-file-path" title={r.path}>{r.path}</div>
                {r.error && <div className="modal-file-error">→ {r.error}</div>}
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
