import { useCallback, useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { api } from '../api';
import type { AiTier, Project, WikiPage, WikiProgress } from '../types';

interface Props {
  project: Project | null;
  isGlobal: boolean;
}

function renderMarkdown(md: string): string {
  // Minimal inline markdown-to-html: headings, bold, code, paragraphs
  // Reuse the .viewer-markdown CSS class for styling
  let html = md
    // Escape HTML first
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    // Fenced code blocks
    .replace(/```[\w]*\n?([\s\S]*?)```/g, '<pre><code>$1</code></pre>')
    // Inline code
    .replace(/`([^`]+)`/g, '<code>$1</code>')
    // Headings
    .replace(/^#### (.+)$/gm, '<h4>$1</h4>')
    .replace(/^### (.+)$/gm, '<h3>$1</h3>')
    .replace(/^## (.+)$/gm, '<h2>$1</h2>')
    .replace(/^# (.+)$/gm, '<h1>$1</h1>')
    // Bold / italic
    .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
    .replace(/\*(.+?)\*/g, '<em>$1</em>')
    // Horizontal rule
    .replace(/^---$/gm, '<hr>')
    // Blockquote
    .replace(/^> (.+)$/gm, '<blockquote>$1</blockquote>')
    // Unordered list items
    .replace(/^[-*] (.+)$/gm, '<li>$1</li>')
    // Paragraphs (double newline)
    .replace(/\n\n(?!<)/g, '</p><p>')
    // Wrap in paragraph if needed
    .replace(/^(?!<[hHpPlLbBhH])(.+)$/gm, (match) => {
      if (match.startsWith('<')) return match;
      return match;
    });

  // Wrap orphaned li's
  html = html.replace(/(<li>.*<\/li>(\n|$))+/g, (m) => `<ul>${m}</ul>`);

  return `<p>${html}</p>`;
}

export function WikiViewer({ project, isGlobal }: Props) {
  const [pages, setPages] = useState<WikiPage[]>([]);
  const [selectedPage, setSelectedPage] = useState<WikiPage | null>(null);
  const [loading, setLoading] = useState(false);
  const [generating, setGenerating] = useState(false);
  const [progress, setProgress] = useState<WikiProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [aiTier, setAiTier] = useState<AiTier>('none');

  const unlistenRef = useRef<(() => void) | null>(null);

  const loadWikiPages = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      let wikiPages: WikiPage[];
      if (isGlobal) {
        wikiPages = await api.getGlobalWiki();
      } else if (project) {
        wikiPages = await api.getProjectWiki(project.id);
      } else {
        wikiPages = [];
      }
      setPages(wikiPages);
      setSelectedPage(wikiPages[0] ?? null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [project, isGlobal]);

  useEffect(() => {
    loadWikiPages();
  }, [loadWikiPages]);

  useEffect(() => {
    api.getAiTier().then(setAiTier).catch(() => {});
  }, []);

  async function handleGenerate(force: boolean) {
    setGenerating(true);
    setError(null);
    setProgress({ stage: 'starting', current: 0, total: 0 });

    // Listen to wiki:progress events
    if (unlistenRef.current) {
      unlistenRef.current();
      unlistenRef.current = null;
    }

    const unlisten = await listen<WikiProgress>('wiki:progress', e => {
      setProgress(e.payload);
    });
    unlistenRef.current = unlisten;

    try {
      if (isGlobal) {
        await api.generateGlobalWiki(force);
      } else if (project) {
        await api.generateProjectWiki(project.id, force);
      }
      await loadWikiPages();
    } catch (e) {
      setError(String(e));
    } finally {
      setGenerating(false);
      setProgress(null);
      if (unlistenRef.current) {
        unlistenRef.current();
        unlistenRef.current = null;
      }
    }
  }

  // Cleanup listener on unmount
  useEffect(() => {
    return () => {
      if (unlistenRef.current) {
        unlistenRef.current();
      }
    };
  }, []);

  const title = isGlobal
    ? 'Global Wiki'
    : project
    ? `${project.name} Wiki`
    : 'Wiki';

  const progressPct =
    progress && progress.total > 0
      ? Math.round((progress.current / progress.total) * 100)
      : null;

  return (
    <div className="wiki-viewer">
      {/* Header */}
      <div className="wiki-viewer-header">
        <span className="wiki-viewer-title">{title}</span>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          {pages.length > 0 && !generating && (
            <button
              className="btn btn-ghost btn-sm"
              onClick={() => handleGenerate(true)}
              disabled={aiTier === 'none'}
              title="Regenerate wiki from scratch"
            >
              Regenerate
            </button>
          )}
          {aiTier !== 'none' ? (
            <button
              className="btn btn-primary btn-sm"
              onClick={() => handleGenerate(false)}
              disabled={generating}
            >
              {generating ? 'Generating…' : pages.length === 0 ? 'Generate Wiki' : 'Refresh'}
            </button>
          ) : null}
        </div>
      </div>

      {/* AI not configured warning */}
      {aiTier === 'none' && (
        <div className="wiki-ai-notice">
          Configure AI provider in Settings to generate wikis
        </div>
      )}

      {/* Progress bar */}
      {generating && progress && (
        <div className="wiki-progress-wrap">
          <div className="wiki-progress-label">
            {progress.stage.charAt(0).toUpperCase() + progress.stage.slice(1)}
            {progress.total > 0 && ` (${progress.current}/${progress.total})`}
          </div>
          <div className="wiki-progress-bar-track">
            <div
              className="wiki-progress-bar-fill"
              style={{ width: progressPct !== null ? `${progressPct}%` : '100%' }}
            />
          </div>
        </div>
      )}

      {error && <div className="error-msg" style={{ margin: '8px 12px' }}>{error}</div>}

      {loading && (
        <div className="loading">Loading wiki…</div>
      )}

      {!loading && !generating && pages.length === 0 && aiTier !== 'none' && (
        <div className="empty-state">
          <div className="empty-state-icon">📖</div>
          <div className="empty-state-title">No wiki pages yet</div>
          <div className="empty-state-desc">
            Click "Generate Wiki" to create wiki pages from your indexed files.
          </div>
        </div>
      )}

      {!loading && pages.length > 0 && (
        <div className="wiki-body">
          {/* Page list */}
          <div className="wiki-list">
            {pages.map(page => (
              <div
                key={page.id}
                className={`wiki-list-item${selectedPage?.id === page.id ? ' selected' : ''}`}
                onClick={() => setSelectedPage(page)}
              >
                {page.title}
              </div>
            ))}
          </div>

          {/* Page content */}
          <div className="wiki-content">
            {selectedPage ? (
              <div
                className="viewer-markdown"
                dangerouslySetInnerHTML={{ __html: renderMarkdown(selectedPage.content) }}
              />
            ) : (
              <div className="empty-state">
                <div className="empty-state-title">Select a page</div>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
