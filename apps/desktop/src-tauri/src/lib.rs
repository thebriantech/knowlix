mod commands;

use commands::watcher::WatcherState;
use std::collections::HashMap;
use std::sync::Mutex as StdMutex;
use tauri::Manager;
use tokio::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(WatcherState(Mutex::new(HashMap::new())))
        .setup(|app| {
            // Use app_data_dir (same as storage — known to work) + /logs subdir
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to resolve app data dir");

            let log_dir = data_dir.join("logs");
            std::fs::create_dir_all(&log_dir).expect("Failed to create log dir");
            let log_path = log_dir.join("knowlix.log");
            let log_file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
                .expect("Failed to open log file");

            tracing_subscriber::fmt()
                .with_max_level(tracing::Level::INFO)
                .with_writer(StdMutex::new(log_file))
                .with_ansi(false)
                .init();

            tracing::info!("Knowlix starting. log={}", log_path.display());

            let embeddings_cache = data_dir.join("cache").join("embeddings");
            knowlix_storage::set_embedding_cache_dir(embeddings_cache);

            tauri::async_runtime::block_on(knowlix_storage::init_with_dir(data_dir))
                .expect("Storage initialization failed");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Project commands
            commands::project::create_project,
            commands::project::list_projects,
            commands::project::get_project,
            commands::project::delete_project,
            commands::project::update_project,
            commands::project::add_folder,
            commands::project::remove_folder,
            // Search commands
            commands::search::search,
            commands::search::search_keyword,
            commands::search::search_semantic,
            // Embedding model commands
            commands::embeddings::get_embedding_model_status,
            commands::embeddings::ensure_embedding_model,
            // Indexer commands
            commands::indexer::index_file,
            commands::indexer::reindex_project,
            commands::indexer::get_index_status,
            // Viewer commands
            commands::viewer::get_view_content,
            // Watcher commands
            commands::watcher::start_file_watcher,
            commands::watcher::stop_file_watcher,
            // Wiki commands (Phase 5)
            commands::wiki::generate_project_wiki,
            commands::wiki::generate_global_wiki,
            commands::wiki::get_project_wiki,
            commands::wiki::get_global_wiki,
            // AI Agent commands (Phase 5)
            commands::ai_agent::answer_question,
            commands::ai_agent::get_ai_tier,
            commands::ai_agent::health_check,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
