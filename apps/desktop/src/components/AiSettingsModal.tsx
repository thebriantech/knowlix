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

export function AiSettingsModal({ open, onClose }: Props) {
  const [config, setConfig] = useState<AiConfig>(DEFAULT_CONFIG);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [healthStatus, setHealthStatus] = useState<AiHealthStatus | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setHealthStatus(null);
    setSaveError(null);
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
    // Save first so health check uses the current URL/model
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
      <div className="modal" style={{ width: 480 }}>
        <div className="modal-header">
          <div>
            <div className="modal-title">AI Settings</div>
            <div className="modal-subtitle">Configure local LLM provider (Ollama)</div>
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
              <option value="ollama">Ollama (local)</option>
            </select>
          </div>

          {config.provider === 'ollama' && (
            <>
              {/* Ollama URL */}
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

              {/* Model */}
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

              {/* Test connection */}
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
                        background: healthStatus.reachable
                          ? 'var(--success)'
                          : 'var(--danger)',
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
            </>
          )}

          {saveError && (
            <div className="error-msg">{saveError}</div>
          )}
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
          <button
            className="btn btn-primary"
            onClick={handleSave}
            disabled={saving}
          >
            {saving ? 'Saving…' : 'Save'}
          </button>
        </div>
      </div>
    </div>
  );
}
