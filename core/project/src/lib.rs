use chrono::Utc;
use knowlix_common::{KnowlixError, Project, Result};
use uuid::Uuid;

pub async fn create_project(name: String, description: Option<String>) -> Result<Project> {
    if name.trim().is_empty() {
        return Err(KnowlixError::Validation("Project name cannot be empty".into()));
    }
    if knowlix_storage::get_project_by_name(&name).await?.is_some() {
        return Err(KnowlixError::Duplicate(format!(
            "Project '{}' already exists",
            name
        )));
    }
    let now = Utc::now();
    let project = Project {
        id: Uuid::new_v4().to_string(),
        name,
        description,
        folders: vec![],
        created_at: now,
        updated_at: now,
    };
    knowlix_storage::insert_project(&project).await?;
    Ok(project)
}

pub async fn add_folder(project_id: &str, path: &str) -> Result<()> {
    let mut project = knowlix_storage::get_project(project_id)
        .await?
        .ok_or_else(|| KnowlixError::ProjectNotFound(project_id.into()))?;

    if !std::path::Path::new(path).exists() {
        return Err(KnowlixError::Validation(format!(
            "Path does not exist: {}",
            path
        )));
    }
    if project.folders.contains(&path.to_string()) {
        return Ok(());
    }
    project.folders.push(path.to_string());
    project.updated_at = Utc::now();
    knowlix_storage::update_project(&project).await
}

pub async fn remove_folder(project_id: &str, path: &str) -> Result<()> {
    let mut project = knowlix_storage::get_project(project_id)
        .await?
        .ok_or_else(|| KnowlixError::ProjectNotFound(project_id.into()))?;

    project.folders.retain(|f| f != path);
    project.updated_at = Utc::now();
    knowlix_storage::update_project(&project).await
}

pub async fn list_projects() -> Result<Vec<Project>> {
    knowlix_storage::list_projects().await
}

pub async fn get_project(project_id: &str) -> Result<Option<Project>> {
    knowlix_storage::get_project(project_id).await
}

pub async fn delete_project(project_id: &str) -> Result<()> {
    if knowlix_storage::get_project(project_id).await?.is_none() {
        return Err(KnowlixError::ProjectNotFound(project_id.into()));
    }
    knowlix_storage::delete_project(project_id).await
}

pub async fn update_project(
    project_id: &str,
    name: String,
    description: Option<String>,
) -> Result<Project> {
    if name.trim().is_empty() {
        return Err(KnowlixError::Validation("Project name cannot be empty".into()));
    }
    let mut project = knowlix_storage::get_project(project_id)
        .await?
        .ok_or_else(|| KnowlixError::ProjectNotFound(project_id.into()))?;

    if project.name != name {
        if knowlix_storage::get_project_by_name(&name).await?.is_some() {
            return Err(KnowlixError::Duplicate(format!(
                "Project '{}' already exists",
                name
            )));
        }
    }

    project.name = name;
    project.description = description;
    project.updated_at = Utc::now();
    knowlix_storage::update_project(&project).await?;
    Ok(project)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::sync::Mutex;

    static TEMP: std::sync::OnceLock<TempDir> = std::sync::OnceLock::new();
    static LOCK: std::sync::OnceLock<Mutex<()>> = std::sync::OnceLock::new();

    async fn setup() -> tokio::sync::MutexGuard<'static, ()> {
        let lock = LOCK.get_or_init(|| Mutex::new(()));
        let guard = lock.lock().await;
        let dir = TEMP.get_or_init(|| TempDir::new().unwrap());
        knowlix_storage::init_with_dir(dir.path().to_path_buf())
            .await
            .unwrap();
        guard
    }

    #[tokio::test]
    async fn test_create_project() {
        let _guard = setup().await;
        let p = create_project("MyProject".into(), None).await.unwrap();
        assert!(!p.id.is_empty());
        assert_eq!(p.name, "MyProject");
        assert!(p.folders.is_empty());
    }

    #[tokio::test]
    async fn test_empty_name_rejected() {
        let _guard = setup().await;
        let err = create_project("   ".into(), None).await.unwrap_err();
        assert!(matches!(err, KnowlixError::Validation(_)));
    }

    #[tokio::test]
    async fn test_duplicate_name_rejected() {
        let _guard = setup().await;
        let name = format!("Dup-{}", Uuid::new_v4());
        create_project(name.clone(), None).await.unwrap();
        let err = create_project(name, None).await.unwrap_err();
        assert!(matches!(err, KnowlixError::Duplicate(_)));
    }

    #[tokio::test]
    async fn test_delete_nonexistent_project() {
        let _guard = setup().await;
        let err = delete_project("nonexistent-id").await.unwrap_err();
        assert!(matches!(err, KnowlixError::ProjectNotFound(_)));
    }

    #[tokio::test]
    async fn test_update_project() {
        let _guard = setup().await;
        let p = create_project(format!("Up-{}", Uuid::new_v4()), Some("old".into()))
            .await
            .unwrap();
        let updated = update_project(&p.id, "NewName".into(), Some("new".into()))
            .await
            .unwrap();
        assert_eq!(updated.name, "NewName");
        assert_eq!(updated.description, Some("new".into()));
    }

    #[tokio::test]
    async fn test_list_projects() {
        let _guard = setup().await;
        let name = format!("List-{}", Uuid::new_v4());
        create_project(name.clone(), None).await.unwrap();
        let all = list_projects().await.unwrap();
        assert!(all.iter().any(|p| p.name == name));
    }
}
