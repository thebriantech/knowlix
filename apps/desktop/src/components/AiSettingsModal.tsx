import { useEffect, useState } from 'react';
import { api } from '../api';
import type { AiConfig, AiHealthStatus } from '../types';

interface Props {
  open: boolean;
  onClose: () => void;
}

const DEFAULT_CONFIG: AiConfig = {
  provider: 'none',
  ollama_model: 'phi3.5',
  ollama_url: 'http://localhost:11434',
  api_key: null,
  api_base_url: 'https://api.openai.com/v1',
  api_model: null,
};

const API_BASE_PRESETS = [
  { label: 'OpenAI', url: 'https://api.openai.com/v1' },
  { label: 'Custom', url: '' },
];

const API_MODEL_SUGGESTIONS = [
  'gpt-4o',
  'gpt-4o-mini',
  'gpt-4-turbo',
  'claude-sonnet-4-6',
  'claude-haiku-4-5-20251001',
];

export function AiSettingsModal({ open, onClose }: Props) {
  const [config, setConfig] = useState<AiConfig>(DEFAULT_CONFIG);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [healthStatus, setHealthStatus] = useState<AiHealthStatus | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [showKey, setShowKey] = useState(false);

  useEffect(() => {
    if (!open) return;
    setHealthStatus(null);
    setSaveError(null);
    setShowKey(false);
    api.getAiConfig()
      .then(setConfig)
      .catch(e => console.error('Failed to load AI config:', e));
  }, [open]);

  if (!open) return null;

  async function handleSave() {
    setSaving(true);
    setSaveError(null);
    try {
      await api.saveAiConfig(config);
      onClose();
    } catch (e) {
      setSaveError(String(e));
    } finally {
      setSaving(false);
    }
  }

  async function handleTestConnection() {
    setTesting(true);
    setHealthStatus(null);
    try {
      await api.saveAiConfig(config);
    } catch (_) {
      // best-effort save before test
    }
    try {
      const status = await api.healthCheck();
      setHealthStatus(status);
    } catch (e) {
      setHealthStatus({
        tier: 'none',
        model: null,
        reachable: false,
        error: String(e),
      });
    } finally {
      setTesting(false);
    }
  }

  function handleOverlayClick(e: React.MouseEvent<HTMLDivElement>) {
    if (e.target === e.currentTarget) onClose();
  }

  return (
    <div className="modal-overlay" onClick={handleOverlayClick}>
      <div className="modal" style={{ width: 500 }}>
        <div className="modal-header">
          <div>
            <div className="modal-title">AI Settings</div>
            <div className="modal-subtitle">Configure AI provider for Q&amp;A and wiki generation</div>
          </div>
          <button className="modal-close" onClick={onClose}>×</button>
        </div>

        <div style={{ padding: '16px 18px', display: 'flex', flexDirection: 'column', gap: 14 }}>
          {/* Provider select */}
          <div className="ai-settings-row">
            <label className="ai-settings-label">Provider</label>
            <select
              className="search-filter-select"
              style={{ flex: 1 }}
              value={config.provider}
              onChange={e => {
                const provider = e.target.value as AiConfig['provider'];
                setConfig(c => ({ ...c, provider }));
                setHealthStatus(null);
              }}
            >
              <option value="none">None (AI disabled)</option>
              <option value="ollama">Ollama (local, offline)</option>
              <option value="api">API Key (OpenAI-compatible)</option>
            </select>
          </div>

          {/* Ollama settings */}
          {config.provider === 'ollama' && (
            <>
              <div className="ai-settings-row">
                <label className="ai-settings-label">Ollama URL</label>
                <input
                  className="ai-settings-input"
                  type="text"
                  value={config.ollama_url}
                  onChange={e => setConfig(c => ({ ...c, ollama_url: e.target.value }))}
                  placeholder="http://localhost:11434"
                />
              </div>

              <div className="ai-settings-row">
                <label className="ai-settings-label">Model</label>
                <input
                  className="ai-settings-input"
                  type="text"
                  value={config.ollama_model ?? ''}
                  onChange={e =>
                    setConfig(c => ({ ...c, ollama_model: e.target.value || null }))
                  }
                  placeholder="phi3.5"
                />
              </div>
            </>
          )}

          {/* API Key settings */}
          {config.provider === 'api' && (
            <>
              <div className="ai-settings-row">
                <label className="ai-settings-label">Base URL</label>
                <div style={{ flex: 1, display: 'flex', gap: 6 }}>
                  <select
                    className="search-filter-select"
                    style={{ width: 110, flexShrink: 0 }}
                    value={
                      API_BASE_PRESETS.find(p => p.url === config.api_base_url)?.label ?? 'Custom'
                    }
                    onChange={e => {
                      const preset = API_BASE_PRESETS.find(p => p.label === e.target.value);
                      if (preset?.url) setConfig(c => ({ ...c, api_base_url: preset.url }));
                    }}
                  >
                    {API_BASE_PRESETS.map(p => (
                      <option key={p.label}>{p.label}</option>
                    ))}
                  </select>
                  <input
                    className="ai-settings-input"
                    type="text"
                    value={config.api_base_url}
                    onChange={e => setConfig(c => ({ ...c, api_base_url: e.target.value }))}
                    placeholder="https://api.openai.com/v1"
                    style={{ flex: 1 }}
                  />
                </div>
              </div>

              <div className="ai-settings-row">
                <label className="ai-settings-label">API Key</label>
                <div style={{ flex: 1, display: 'flex', gap: 6, alignItems: 'center' }}>
                  <input
                    className="ai-settings-input"
                    type={showKey ? 'text' : 'password'}
                    value={config.api_key ?? ''}
                    onChange={e =>
                      setConfig(c => ({ ...c, api_key: e.target.value || null }))
                    }
                    placeholder="sk-..."
                    style={{ flex: 1 }}
                    autoComplete="off"
                  />
                  <button
                    className="btn btn-ghost btn-sm"
                    type="button"
                    onClick={() => setShowKey(v => !v)}
                    style={{ flexShrink: 0 }}
                  >
                    {showKey ? 'Hide' : 'Show'}
                  </button>
                </div>
              </div>

              <div className="ai-settings-row">
                <label className="ai-settings-label">Model</label>
                <div style={{ flex: 1, display: 'flex', gap: 6 }}>
                  <input
                    className="ai-settings-input"
                    type="text"
                    list="api-model-suggestions"
                    value={config.api_model ?? ''}
                    onChange={e =>
                      setConfig(c => ({ ...c, api_model: e.target.value || null }))
                    }
                    placeholder="gpt-4o"
                    style={{ flex: 1 }}
                  />
                  <datalist id="api-model-suggestions">
                    {API_MODEL_SUGGESTIONS.map(m => (
                      <option key={m} value={m} />
                    ))}
                  </datalist>
                </div>
              </div>

              <div
                style={{
                  fontSize: 11,
                  color: 'var(--text-muted)',
                  padding: '4px 0',
                  lineHeight: 1.5,
                }}
              >
                Key stored encrypted in local database. Chunks are sent to the configured API
                endpoint during Q&amp;A and wiki generation.
              </div>
            </>
          )}

          {/* Test connection (Ollama or API) */}
          {config.provider !== 'none' && (
            <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
              <button
                className="btn btn-ghost"
                onClick={handleTestConnection}
                disabled={testing}
                style={{ flexShrink: 0 }}
              >
                {testing ? 'Testing…' : 'Test Connection'}
              </button>

              {healthStatus && (
                <div style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 12 }}>
                  <span
                    className="ai-status-dot"
                    style={{
                      background: healthStatus.reachable ? 'var(--success)' : 'var(--danger)',
                    }}
                  />
                  {healthStatus.reachable ? (
                    <span style={{ color: 'var(--success)' }}>
                      Connected
                      {healthStatus.model && ` · ${healthStatus.model}`}
                    </span>
                  ) : (
                    <span style={{ color: 'var(--danger)' }}>
                      {healthStatus.error ?? 'Unreachable'}
                    </span>
                  )}
                </div>
              )}
            </div>
          )}

          {saveError && <div className="error-msg">{saveError}</div>}
        </div>

        <div
          style={{
            display: 'flex',
            justifyContent: 'flex-end',
            gap: 8,
            padding: '12px 18px',
            borderTop: '1px solid var(--panel-border)',
          }}
        >
          <button className="btn btn-ghost" onClick={onClose}>
            Cancel
          </button>
          <button className="btn btn-primary" onClick={handleSave} disabled={saving}>
            {saving ? 'Saving…' : 'Save'}
          </button>
        </div>
      </div>
    </div>
  );
}
