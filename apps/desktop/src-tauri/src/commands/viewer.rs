use knowlix_viewer::ViewContent;

#[tauri::command]
pub async fn get_view_content(file_path: String) -> Result<ViewContent, String> {
    knowlix_viewer::get_view_content(&file_path)
        .await
        .map_err(|e| e.to_string())
}
