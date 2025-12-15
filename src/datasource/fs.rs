pub async fn init_fs(data_path: &str) {
    match tokio::fs::create_dir(data_path).await {
        Ok(_) => tracing::info!("created uploads directory"),
        Err(_) => tracing::warn!("uploads directory exists - using it"),
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_init_fs_creates_directory() {
        let temp_dir = TempDir::new().unwrap();
        let data_path = temp_dir.path().join("uploads");
        let data_path_str = data_path.to_str().unwrap();

        // Directory should not exist yet
        assert!(!data_path.exists());

        init_fs(data_path_str).await;

        // Directory should now exist
        assert!(data_path.exists());
        assert!(data_path.is_dir());
    }

    #[tokio::test]
    async fn test_init_fs_directory_already_exists() {
        let temp_dir = TempDir::new().unwrap();
        let data_path = temp_dir.path().join("existing");
        let data_path_str = data_path.to_str().unwrap();

        // Create the directory first
        tokio::fs::create_dir(&data_path).await.unwrap();
        assert!(data_path.exists());

        // Calling init_fs again should not fail
        init_fs(data_path_str).await;

        // Directory should still exist
        assert!(data_path.exists());
        assert!(data_path.is_dir());
    }

    #[tokio::test]
    async fn test_init_fs_nested_path() {
        let temp_dir = TempDir::new().unwrap();
        let parent = temp_dir.path().join("parent");
        tokio::fs::create_dir(&parent).await.unwrap();

        let data_path = parent.join("nested");
        let data_path_str = data_path.to_str().unwrap();

        assert!(!data_path.exists());

        init_fs(data_path_str).await;

        assert!(data_path.exists());
        assert!(data_path.is_dir());
    }

    #[tokio::test]
    async fn test_init_fs_creates_sibling_directories() {
        let temp_dir = TempDir::new().unwrap();
        let dir1 = temp_dir.path().join("dir1");
        let dir2 = temp_dir.path().join("dir2");

        let dir1_str = dir1.to_str().unwrap();
        let dir2_str = dir2.to_str().unwrap();

        init_fs(dir1_str).await;
        init_fs(dir2_str).await;

        // Both directories should exist
        assert!(dir1.exists());
        assert!(dir2.exists());
    }
}
