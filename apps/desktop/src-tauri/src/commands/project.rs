use knowlix_common::Project;

#[tauri::command]
pub async fn create_project(name: String, description: Option<String>) -> Result<Project, String> {
    knowlix_project::create_project(name, description)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_projects() -> Result<Vec<Project>, String> {
    knowlix_project::list_projects().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_project(project_id: String) -> Result<Option<Project>, String> {
    knowlix_project::get_project(&project_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_project(project_id: String) -> Result<(), String> {
    knowlix_project::delete_project(&project_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_project(
    project_id: String,
    name: String,
    description: Option<String>,
) -> Result<Project, String> {
    knowlix_project::update_project(&project_id, name, description)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_folder(project_id: String, path: String) -> Result<(), String> {
    knowlix_project::add_folder(&project_id, &path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_folder(project_id: String, path: String) -> Result<(), String> {
    knowlix_project::remove_folder(&project_id, &path)
        .await
        .map_err(|e| e.to_string())
}
