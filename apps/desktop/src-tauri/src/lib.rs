mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|_app| {
            tauri::async_runtime::block_on(knowlix_storage::init())
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
            // Indexer commands
            commands::indexer::index_file,
            commands::indexer::reindex_project,
            commands::indexer::get_index_status,
            // Viewer commands
            commands::viewer::get_view_content,
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
