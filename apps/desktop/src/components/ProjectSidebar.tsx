import { useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { api } from '../api';
import type { IndexProgress, IndexStats, IndexStatus, Project } from '../types';
import { IndexDetailModal } from './IndexDetailModal';

interface Props {
  projects: Project[];
  selectedProject: Project | null;
  onSelect: (p: Project) => void;
  onProjectsChange: () => void;
  onShowWiki: (global: boolean) => void;
}

function formatLastIndexed(iso: string | null): string {
  if (!iso) return 'Never';
  const d = new Date(iso);
  const diff = Date.now() - d.getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return 'Just now';
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}h ago`;
  return d.toLocaleDateString();
}

export function ProjectSidebar({ projects, selectedProject, onSelect, onProjectsChange, onShowWiki }: Props) {
  const [newName, setNewName] = useState('');
  const [newDesc, setNewDesc] = useState('');
  const [creating, setCreating] = useState(false);

  const [addingFolder, setAddingFolder] = useState(false);

  const [indexing, setIndexing] = useState(false);
  const [indexResult, setIndexResult] = useState<{ stats: IndexStats; error?: string } | null>(null);
  const [modalOpen, setModalOpen] = useState(false);
  const [progress, setProgress] = useState<IndexProgress | null>(null);
  const [indexStatus, setIndexStatus] = useState<IndexStatus | null>(null);

  const prevProjectId = useRef<string | null>(null);

  // Start/stop file watcher when selected project changes
  useEffect(() => {
    const prevId = prevProjectId.current;
    const currentId = selectedProject?.id ?? null;

    if (prevId && prevId !== currentId) {
      api.stopFileWatcher(prevId).catch(console.error);
    }
    if (currentId) {
      api.startFileWatcher(currentId).catch(console.error);
    }
    prevProjectId.current = currentId;
    // No cleanup: watcher lives until project changes or process exits.
    // Cleanup caused a race with React StrictMode (stop arrived after re-start → dead watcher).
  }, [selectedProject?.id]);

  // Fetch index status when project selected or after reindex
  useEffect(() => {
    if (!selectedProject) { setIndexStatus(null); return; }
    api.getIndexStatus(selectedProject.id)
      .then(setIndexStatus)
      .catch(console.error);
  }, [selectedProject?.id, indexResult]);

  // Listen to indexing_progress events
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    listen<IndexProgress>('indexing_progress', e => setProgress(e.payload))
      .then(fn => { unlisten = fn; });
    return () => { unlisten?.(); };
  }, []);

  // Listen to file_indexed / file_removed / reindex_complete events (refresh status)
  useEffect(() => {
    if (!selectedProject) return;
    let unlisten1: (() => void) | null = null;
    let unlisten2: (() => void) | null = null;
    let unlisten3: (() => void) | null = null;
    const projectId = selectedProject.id;
    const refresh = () => api.getIndexStatus(projectId).then(setIndexStatus).catch(console.error);
    listen<string>('file_indexed', refresh).then(fn => { unlisten1 = fn; });
    listen<string>('file_removed', refresh).then(fn => { unlisten2 = fn; });
    listen<IndexStats>('reindex_complete', e => {
      setIndexStatus(s => s ? { ...s } : s); // trigger time refresh
      setIndexResult({ stats: e.payload });
      refresh();
    }).then(fn => { unlisten3 = fn; });
    return () => { unlisten1?.(); unlisten2?.(); unlisten3?.(); };
  }, [selectedProject?.id]);

  // Tick every 30s to keep relative time display fresh
  useEffect(() => {
    if (!selectedProject) return;
    const projectId = selectedProject.id;
    const id = setInterval(() => {
      api.getIndexStatus(projectId).then(setIndexStatus).catch(console.error);
    }, 30_000);
    return () => clearInterval(id);
  }, [selectedProject?.id]);

  async function createProject() {
    const name = newName.trim();
    if (!name) return;
    setCreating(true);
    try {
      await api.createProject(name, newDesc.trim() || undefined);
      setNewName('');
      setNewDesc('');
      onProjectsChange();
    } catch (e) {
      console.error(e);
    } finally {
      setCreating(false);
    }
  }

  async function deleteProject(p: Project) {
    if (!confirm(`Delete project "${p.name}"?`)) return;
    try {
      await api.stopFileWatcher(p.id).catch(() => {});
      await api.deleteProject(p.id);
      onProjectsChange();
    } catch (e) {
      console.error(e);
    }
  }

  async function addFolder() {
    if (!selectedProject) return;
    setAddingFolder(true);
    try {
      const selected = await open({ directory: true, multiple: false });
      if (selected) {
        await api.addFolder(selectedProject.id, selected);
        onProjectsChange();
        setIndexing(true);
        setIndexResult(null);
        setModalOpen(false);
        setProgress(null);
        try {
          const stats = await api.reindexProject(selectedProject.id);
          setIndexResult({ stats });
        } catch (e) {
          setIndexResult({ stats: { total_files: 0, indexed: 0, skipped: 0, failed: 0, removed: 0, duration_ms: 0, file_results: [] }, error: String(e) });
        } finally {
          setIndexing(false);
          setProgress(null);
        }
      }
    } catch (e) {
      console.error(e);
    } finally {
      setAddingFolder(false);
    }
  }

  async function removeFolder(folder: string) {
    if (!selectedProject) return;
    try {
      await api.removeFolder(selectedProject.id, folder);
      onProjectsChange();
    } catch (e) {
      console.error(e);
    }
  }

  async function reindex() {
    if (!selectedProject) return;
    setIndexing(true);
    setIndexResult(null);
    setModalOpen(false);
    setProgress(null);
    try {
      const stats = await api.reindexProject(selectedProject.id);
      setIndexResult({ stats });
    } catch (e) {
      setIndexResult({ stats: { total_files: 0, indexed: 0, skipped: 0, failed: 0, removed: 0, duration_ms: 0, file_results: [] }, error: String(e) });
    } finally {
      setIndexing(false);
      setProgress(null);
    }
  }

  return (
    <>
      <div className="sidebar-header">Knowlix</div>

      <div className="sidebar-section-label">Projects</div>

      <div className="sidebar-scroll">
        {projects.length === 0 && (
          <div style={{ padding: '8px 14px', fontSize: 12, color: 'var(--sidebar-muted)' }}>
            No projects yet.
          </div>
        )}
        {projects.map(p => (
          <div
            key={p.id}
            className={`project-item${selectedProject?.id === p.id ? ' selected' : ''}`}
            onClick={() => onSelect(p)}
          >
            <div className="project-item-name">{p.name}</div>
            {p.description && <div className="project-item-desc">{p.description}</div>}
          </div>
        ))}
      </div>

      {selectedProject && (
        <div className="project-detail">
          <div className="project-detail-title">{selectedProject.name}</div>

          {selectedProject.folders.map(f => (
            <div key={f} className="folder-item">
              <span style={{ fontSize: 11, flexShrink: 0 }}>📁</span>
              <span className="folder-path" title={f}>{f}</span>
              <button className="folder-remove-btn" onClick={() => removeFolder(f)} title="Remove folder">×</button>
            </div>
          ))}

          {selectedProject.folders.length === 0 && (
            <div style={{ fontSize: 11, color: 'var(--sidebar-muted)', padding: '3px 4px' }}>No folders added.</div>
          )}

          <button
            className="btn btn-ghost btn-sm"
            style={{ width: '100%', marginTop: 4 }}
            onClick={addFolder}
            disabled={addingFolder}
          >
            {addingFolder ? 'Selecting…' : '+ Add Folder'}
          </button>

          <div className="project-actions">
            <button
              className="btn btn-ghost btn-sm"
              onClick={reindex}
              disabled={indexing || selectedProject.folders.length === 0}
              style={{ flex: 1 }}
            >
              {indexing ? 'Indexing…' : '⟳ Reindex'}
            </button>
            <button className="btn btn-danger btn-sm" onClick={() => deleteProject(selectedProject)}>
              Delete
            </button>
          </div>

          <div className="project-actions" style={{ marginTop: 4 }}>
            <button
              className="btn btn-ghost btn-sm"
              onClick={() => onShowWiki(false)}
              style={{ flex: 1 }}
              title="View or generate wiki for this project"
            >
              📖 Wiki
            </button>
            <button
              className="btn btn-ghost btn-sm"
              onClick={() => onShowWiki(true)}
              style={{ flex: 1 }}
              title="View global wiki across all projects"
            >
              🌐 Global
            </button>
          </div>

          {indexing && progress && progress.total > 0 && (
            <div className="index-progress">
              <div className="index-progress-bar-wrap">
                <div
                  className="index-progress-bar"
                  style={{ width: `${Math.round((progress.current / progress.total) * 100)}%` }}
                />
              </div>
              <div className="index-progress-label">
                {progress.current}/{progress.total} — {progress.current_file.split(/[\\/]/).pop()}
              </div>
            </div>
          )}

          {indexStatus && !indexing && (
            <div style={{ fontSize: 10, color: 'var(--sidebar-muted)', padding: '2px 4px', marginTop: 2 }}>
              Last indexed: {formatLastIndexed(indexStatus.last_indexed)}
              {indexStatus.total_files > 0 && ` · ${indexStatus.indexed_files}/${indexStatus.total_files} files`}
            </div>
          )}

          {indexResult && (
            <div className={`index-result ${indexResult.error ? 'error' : 'success'}`}>
              {indexResult.error ? (
                `Error: ${indexResult.error}`
              ) : (
                <div className="index-result-row">
                  <span>
                    ✓ {indexResult.stats.indexed} indexed
                    {indexResult.stats.skipped > 0 && `, ${indexResult.stats.skipped} skipped`}
                    {indexResult.stats.failed > 0 && (
                      <span className="index-failed-count"> · ✗ {indexResult.stats.failed} failed</span>
                    )}
                  </span>
                  <button className="index-detail-btn" onClick={() => setModalOpen(true)}>
                    Details →
                  </button>
                </div>
              )}
            </div>
          )}

          {modalOpen && indexResult && !indexResult.error && (
            <IndexDetailModal
              stats={indexResult.stats}
              projectName={selectedProject.name}
              onClose={() => setModalOpen(false)}
            />
          )}
        </div>
      )}

      <div className="create-project-section">
        <div style={{ fontSize: 11, color: 'var(--sidebar-muted)', marginBottom: 5, fontWeight: 600 }}>
          NEW PROJECT
        </div>
        <input
          value={newName}
          onChange={e => setNewName(e.target.value)}
          onKeyDown={e => e.key === 'Enter' && createProject()}
          placeholder="Project name"
        />
        <input
          value={newDesc}
          onChange={e => setNewDesc(e.target.value)}
          onKeyDown={e => e.key === 'Enter' && createProject()}
          placeholder="Description (optional)"
        />
        <button
          className="btn btn-primary btn-sm"
          style={{ width: '100%' }}
          onClick={createProject}
          disabled={creating || !newName.trim()}
        >
          {creating ? 'Creating…' : '+ Create Project'}
        </button>
      </div>
    </>
  );
}
