import { invoke } from '@tauri-apps/api/core';
import type {
  AiAnswer,
  AiConfig,
  AiHealthStatus,
  AiTier,
  EmbeddingModelStatus,
  IndexStats,
  IndexStatus,
  Project,
  SearchResult,
  ViewContent,
  WikiPage,
} from './types';

export const api = {
  createProject: (name: string, description?: string) =>
    invoke<Project>('create_project', { name, description }),

  listProjects: () => invoke<Project[]>('list_projects'),

  deleteProject: (projectId: string) =>
    invoke<void>('delete_project', { projectId }),

  addFolder: (projectId: string, path: string) =>
    invoke<void>('add_folder', { projectId, path }),

  removeFolder: (projectId: string, path: string) =>
    invoke<void>('remove_folder', { projectId, path }),

  search: (query: string, projectId?: string, limit = 20) =>
    invoke<SearchResult[]>('search', { query, projectId, limit }),

  searchKeyword: (query: string, projectId?: string, limit = 20) =>
    invoke<SearchResult[]>('search_keyword', { query, projectId, limit }),

  searchSemantic: (query: string, projectId?: string, limit = 20) =>
    invoke<SearchResult[]>('search_semantic', { query, projectId, limit }),

  reindexProject: (projectId: string) =>
    invoke<IndexStats>('reindex_project', { projectId }),

  getIndexStatus: (projectId: string) =>
    invoke<IndexStatus>('get_index_status', { projectId }),

  getViewContent: (filePath: string) =>
    invoke<ViewContent>('get_view_content', { filePath }),

  startFileWatcher: (projectId: string) =>
    invoke<void>('start_file_watcher', { projectId }),

  stopFileWatcher: (projectId: string) =>
    invoke<void>('stop_file_watcher', { projectId }),

  getEmbeddingModelStatus: () =>
    invoke<EmbeddingModelStatus>('get_embedding_model_status'),

  ensureEmbeddingModel: () =>
    invoke<void>('ensure_embedding_model'),

  // ── AI / Phase 5 ────────────────────────────────────────

  getAiConfig: () => invoke<AiConfig>('get_ai_config'),

  saveAiConfig: (config: AiConfig) =>
    invoke<void>('save_ai_config', { config }),

  healthCheck: () => invoke<AiHealthStatus>('health_check'),

  getAiTier: () => invoke<AiTier>('get_ai_tier'),

  answerQuestion: (query: string, projectId?: string) =>
    invoke<AiAnswer>('answer_question', { query, projectId }),

  generateProjectWiki: (projectId: string, forceRegenerate: boolean) =>
    invoke<void>('generate_project_wiki', { projectId, forceRegenerate }),

  generateGlobalWiki: (forceRegenerate: boolean) =>
    invoke<void>('generate_global_wiki', { forceRegenerate }),

  getProjectWiki: (projectId: string) =>
    invoke<WikiPage[]>('get_project_wiki', { projectId }),

  getGlobalWiki: () => invoke<WikiPage[]>('get_global_wiki'),
};
