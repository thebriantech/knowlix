import { useEffect, useRef, useState } from 'react';
import { marked } from 'marked';
import { EditorView, lineNumbers } from '@codemirror/view';
import { EditorState } from '@codemirror/state';
import { syntaxHighlighting, HighlightStyle } from '@codemirror/language';
import { tags } from '@lezer/highlight';
import { javascript } from '@codemirror/lang-javascript';
import { python } from '@codemirror/lang-python';
import { rust } from '@codemirror/lang-rust';
import { cpp } from '@codemirror/lang-cpp';
import { java } from '@codemirror/lang-java';
import { go } from '@codemirror/lang-go';
import { html as htmlLang } from '@codemirror/lang-html';
import { css as cssLang } from '@codemirror/lang-css';
import { markdown } from '@codemirror/lang-markdown';
import { json } from '@codemirror/lang-json';
import { sql } from '@codemirror/lang-sql';
import { api } from '../api';
import type { SearchResult, ViewContent } from '../types';

interface Props {
  result: SearchResult | null;
}

const codeHighlightStyle = HighlightStyle.define([
  { tag: tags.keyword, color: '#0000ff' },
  { tag: tags.controlKeyword, color: '#af00db' },
  { tag: tags.operatorKeyword, color: '#0000ff' },
  { tag: tags.comment, color: '#008000', fontStyle: 'italic' },
  { tag: tags.lineComment, color: '#008000', fontStyle: 'italic' },
  { tag: tags.blockComment, color: '#008000', fontStyle: 'italic' },
  { tag: tags.docComment, color: '#008000', fontStyle: 'italic' },
  { tag: tags.string, color: '#a31515' },
  { tag: tags.special(tags.string), color: '#a31515' },
  { tag: tags.number, color: '#098658' },
  { tag: tags.integer, color: '#098658' },
  { tag: tags.float, color: '#098658' },
  { tag: tags.bool, color: '#0000ff' },
  { tag: tags.null, color: '#0000ff' },
  { tag: tags.typeName, color: '#267f99' },
  { tag: tags.className, color: '#267f99' },
  { tag: tags.namespace, color: '#267f99' },
  { tag: tags.typeOperator, color: '#0000ff' },
  { tag: tags.self, color: '#001080' },
  { tag: tags.function(tags.variableName), color: '#795e26' },
  { tag: tags.function(tags.propertyName), color: '#795e26' },
  { tag: tags.definition(tags.variableName), color: '#001080' },
  { tag: tags.definition(tags.propertyName), color: '#001080' },
  { tag: tags.variableName, color: '#001080' },
  { tag: tags.propertyName, color: '#001080' },
  { tag: tags.attributeName, color: '#e50000' },
  { tag: tags.attributeValue, color: '#a31515' },
  { tag: tags.tagName, color: '#800000' },
  { tag: tags.angleBracket, color: '#800000' },
  { tag: tags.operator, color: '#000000' },
  { tag: tags.punctuation, color: '#000000' },
  { tag: tags.meta, color: '#af00db' },
  { tag: tags.modifier, color: '#0000ff' },
  { tag: tags.escape, color: '#ee0000' },
  { tag: tags.regexp, color: '#811f3f' },
]);

function langExtension(lang: string) {
  switch (lang) {
    case 'javascript': return javascript();
    case 'typescript': return javascript({ typescript: true });
    case 'jsx': return javascript({ jsx: true });
    case 'tsx': return javascript({ typescript: true, jsx: true });
    case 'python': return python();
    case 'rust': return rust();
    case 'cpp': case 'c': return cpp();
    case 'java': return java();
    case 'go': return go();
    case 'html': return htmlLang();
    case 'css': return cssLang();
    case 'markdown': return markdown();
    case 'json': return json();
    case 'sql': return sql();
    default: return [];
  }
}

function basename(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

function decodeBase64ToBytes(b64: string): Uint8Array {
  const binary = atob(b64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

function CodeEditor({ content, language }: { content: string; language: string }) {
  const containerRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);

  useEffect(() => {
    if (!containerRef.current) return;

    viewRef.current?.destroy();

    const extensions = [
      lineNumbers(),
      syntaxHighlighting(codeHighlightStyle),
      EditorState.readOnly.of(true),
      EditorView.theme({
        '&': { height: '100%', fontSize: '13px', fontFamily: "'Cascadia Code', 'Fira Code', Consolas, monospace" },
        '.cm-scroller': { overflow: 'auto' },
        '.cm-gutters': { background: '#f9fafb', borderRight: '1px solid #e5e7eb', color: '#9ca3af' },
        '.cm-lineNumbers .cm-gutterElement': { padding: '0 8px 0 4px', minWidth: '32px' },
        '.cm-content': { padding: '0' },
        '.cm-line': { padding: '0 12px' },
      }),
      langExtension(language),
    ].flat();

    const view = new EditorView({
      state: EditorState.create({ doc: content, extensions }),
      parent: containerRef.current,
    });

    viewRef.current = view;

    return () => {
      view.destroy();
      viewRef.current = null;
    };
  }, [content, language]);

  return <div ref={containerRef} className="viewer-code" />;
}

function PdfViewer({ data }: { data: string }) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    let cancelled = false;

    setLoading(true);
    setError(null);

    (async () => {
      try {
        const pdfjsLib = await import('pdfjs-dist');
        pdfjsLib.GlobalWorkerOptions.workerSrc = new URL(
          'pdfjs-dist/build/pdf.worker.min.mjs',
          import.meta.url,
        ).toString();

        const bytes = decodeBase64ToBytes(data);
        const loadingTask = pdfjsLib.getDocument({ data: bytes });
        const pdf = await loadingTask.promise;
        if (cancelled) return;

        container.innerHTML = '';

        for (let pageNum = 1; pageNum <= pdf.numPages; pageNum++) {
          if (cancelled) break;
          const page = await pdf.getPage(pageNum);
          const viewport = page.getViewport({ scale: 1.5 });

          const canvas = document.createElement('canvas');
          canvas.width = viewport.width;
          canvas.height = viewport.height;
          canvas.style.display = 'block';
          canvas.style.marginBottom = '8px';
          canvas.style.maxWidth = '100%';
          canvas.style.boxShadow = '0 1px 4px rgba(0,0,0,0.15)';

          const ctx = canvas.getContext('2d');
          if (!ctx || cancelled) break;

          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          await page.render({ canvasContext: ctx as any, viewport }).promise;
          if (cancelled) break;
          container.appendChild(canvas);
        }

        if (!cancelled) setLoading(false);
      } catch (e) {
        if (!cancelled) {
          setError(String(e));
          setLoading(false);
        }
      }
    })();

    return () => { cancelled = true; };
  }, [data]);

  return (
    <div className="viewer-pdf">
      {loading && !error && <div className="loading">Rendering PDF…</div>}
      {error && <div className="error-msg" style={{ margin: 16 }}>{error}</div>}
      <div ref={containerRef} style={{ padding: '16px' }} />
    </div>
  );
}

function DocxViewer({ data }: { data: string }) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    let cancelled = false;

    setLoading(true);
    setError(null);
    container.innerHTML = '';

    (async () => {
      try {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const { renderAsync } = await import('docx-preview') as any;
        const bytes = decodeBase64ToBytes(data);
        if (cancelled) return;
        await renderAsync(bytes.buffer, container, undefined, {
          className: 'docx-preview',
          injectStylesheet: true,
          renderHeaders: true,
          renderFooters: true,
          renderFootnotes: true,
          renderEndnotes: true,
        });
        if (!cancelled) setLoading(false);
      } catch (e) {
        if (!cancelled) {
          setError(String(e));
          setLoading(false);
        }
      }
    })();

    return () => { cancelled = true; };
  }, [data]);

  return (
    <div className="viewer-docx">
      {loading && !error && <div className="loading">Rendering document…</div>}
      {error && <div className="error-msg" style={{ margin: 16 }}>{error}</div>}
      <div ref={containerRef} style={{ padding: '16px' }} />
    </div>
  );
}

export function FileViewer({ result }: Props) {
  const [viewContent, setViewContent] = useState<ViewContent | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!result) {
      setViewContent(null);
      setError(null);
      return;
    }

    let cancelled = false;
    setLoading(true);
    setError(null);
    setViewContent(null);

    api.getViewContent(result.file_path)
      .then(content => {
        if (!cancelled) setViewContent(content);
      })
      .catch(e => {
        if (!cancelled) setError(String(e));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => { cancelled = true; };
  }, [result?.file_path]);

  if (!result) {
    return (
      <div className="viewer">
        <div className="empty-state" style={{ height: '100%' }}>
          <div className="empty-state-icon">📄</div>
          <div className="empty-state-title">No file selected</div>
          <div className="empty-state-desc">Click a search result to view the file.</div>
        </div>
      </div>
    );
  }

  const filename = basename(result.file_path);

  function renderBody() {
    if (loading) return <div className="loading">Loading…</div>;
    if (error) return <div className="error-msg" style={{ margin: 16 }}>{error}</div>;
    if (!viewContent) return null;

    switch (viewContent.type) {
      case 'code':
        return <CodeEditor content={viewContent.content} language={viewContent.language} />;

      case 'markdown':
        return (
          <div
            className="viewer-markdown"
            dangerouslySetInnerHTML={{ __html: String(marked.parse(viewContent.content)) }}
          />
        );

      case 'html':
        return (
          <div
            className="viewer-markdown"
            dangerouslySetInnerHTML={{ __html: viewContent.content }}
          />
        );

      case 'plain_text':
        return <pre className="viewer-plaintext">{viewContent.content}</pre>;

      case 'image':
        return (
          <div className="viewer-image">
            <img src={viewContent.data_uri} alt={filename} />
          </div>
        );

      case 'pdf':
        return <PdfViewer data={viewContent.data} />;

      case 'docx':
        return <DocxViewer data={viewContent.data} />;

      default:
        return <div className="error-msg" style={{ margin: 16 }}>Unknown content type.</div>;
    }
  }

  function langBadge() {
    if (!viewContent) return null;
    if (viewContent.type === 'code') return <span className="viewer-lang-badge">{viewContent.language}</span>;
    if (viewContent.type === 'markdown') return <span className="viewer-lang-badge">markdown</span>;
    if (viewContent.type === 'image') return <span className="viewer-lang-badge">{viewContent.mime}</span>;
    if (viewContent.type === 'pdf') return <span className="viewer-lang-badge">pdf</span>;
    if (viewContent.type === 'docx') return <span className="viewer-lang-badge">docx</span>;
    return null;
  }

  return (
    <div className="viewer">
      <div className="viewer-header">
        <div>
          <div className="viewer-filename">{filename}</div>
          <div className="viewer-filepath">{result.file_path}</div>
        </div>
        {langBadge()}
      </div>
      <div className="viewer-body">
        {renderBody()}
      </div>
    </div>
  );
}
