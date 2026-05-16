export interface Project {
  id: string;
  name: string;
  description?: string;
  folders: string[];
  created_at: string;
  updated_at: string;
}

export interface SearchResult {
  file_id: string;
  file_path: string;
  chunk_id: string;
  snippet: string;
  score: number;
  rank_bm25?: number;
  rank_vec?: number;
  source: 'bm25' | 'vector' | 'hybrid';
}

export type IndexFileStatus = 'indexed' | 'skipped' | 'failed';

export interface IndexFileResult {
  path: string;
  status: IndexFileStatus;
  error?: string;
}

export interface IndexStats {
  total_files: number;
  indexed: number;
  skipped: number;
  failed: number;
  duration_ms: number;
  file_results: IndexFileResult[];
}

export interface IndexStatus {
  total_files: number;
  indexed_files: number;
  in_progress: boolean;
  last_indexed: string | null;
}

export interface IndexProgress {
  current: number;
  total: number;
  current_file: string;
}

export type ViewContent =
  | { type: 'code'; content: string; language: string }
  | { type: 'markdown'; content: string }
  | { type: 'html'; content: string }
  | { type: 'image'; data_uri: string; mime: string }
  | { type: 'plain_text'; content: string }
  | { type: 'pdf'; data: string }
  | { type: 'docx'; data: string };
