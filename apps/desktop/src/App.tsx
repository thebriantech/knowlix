import { useState, useEffect, useCallback, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import './App.css';
import { api } from './api';
import type { Project, SearchResult } from './types';
import { ProjectSidebar } from './components/ProjectSidebar';
import { SearchPanel } from './components/SearchPanel';
import { FileViewer } from './components/FileViewer';

interface Toast {
  id: number;
  kind: 'index' | 'remove';
  file: string;
}

let toastSeq = 0;

function basename(p: string) {
  return p.split(/[\\/]/).pop() ?? p;
}

export default function App() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [selectedProject, setSelectedProject] = useState<Project | null>(null);
  const [selectedResult, setSelectedResult] = useState<SearchResult | null>(null);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const timersRef = useRef<Map<number, ReturnType<typeof setTimeout>>>(new Map());

  const pushToast = useCallback((kind: Toast['kind'], file: string) => {
    const id = ++toastSeq;
    setToasts(prev => [...prev, { id, kind, file }]);
    const t = setTimeout(() => {
      setToasts(prev => prev.filter(x => x.id !== id));
      timersRef.current.delete(id);
    }, 3000);
    timersRef.current.set(id, t);
  }, []);

  useEffect(() => {
    const timers = timersRef.current;
    return () => { timers.forEach(clearTimeout); timers.clear(); };
  }, []);

  useEffect(() => {
    let u1: (() => void) | null = null;
    let u2: (() => void) | null = null;
    listen<string>('file_indexed', e => pushToast('index', e.payload))
      .then(fn => { u1 = fn; });
    listen<string>('file_removed', e => pushToast('remove', e.payload))
      .then(fn => { u2 = fn; });
    return () => { u1?.(); u2?.(); };
  }, [pushToast]);

  const loadProjects = useCallback(async () => {
    try {
      const list = await api.listProjects();
      setProjects(list);
      setSelectedProject(prev => {
        if (!prev) return null;
        return list.find(p => p.id === prev.id) ?? null;
      });
    } catch (e) {
      console.error('Failed to load projects:', e);
    }
  }, []);

  useEffect(() => {
    loadProjects();
  }, [loadProjects]);

  function handleSelectProject(p: Project) {
    setSelectedProject(p);
    setSelectedResult(null);
  }

  return (
    <div className="app">
      <nav className="sidebar">
        <ProjectSidebar
          projects={projects}
          selectedProject={selectedProject}
          onSelect={handleSelectProject}
          onProjectsChange={loadProjects}
        />
      </nav>

      <SearchPanel
        selectedProject={selectedProject}
        onResultSelect={setSelectedResult}
        selectedResultId={selectedResult?.chunk_id ?? null}
      />

      <FileViewer result={selectedResult} />

      <div className="toast-container">
        {toasts.map(t => (
          <div key={t.id} className={`toast toast-${t.kind}`}>
            <span className="toast-icon">{t.kind === 'index' ? '✓' : '✕'}</span>
            <div className="toast-body">
              <div className="toast-title">{t.kind === 'index' ? 'Auto-indexed' : 'File removed'}</div>
              <div className="toast-file">{basename(t.file)}</div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
