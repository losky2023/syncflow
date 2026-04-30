pub mod baidu;
pub mod fake;
pub mod provider;

pub use baidu::*;
pub use fake::FakeCloudProvider;
pub use provider::*;

#[cfg(test)]
mod tests {
    use super::{CloudProvider, FakeCloudProvider};

    #[tokio::test]
    async fn fake_provider_upload_lists_and_downloads_file() {
        let provider = FakeCloudProvider::new();
        provider
            .create_directory("/apps/SyncFlow/Notes")
            .await
            .unwrap();

        let temp_dir =
            std::env::temp_dir().join(format!("syncflow-fake-provider-{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();
        let source = temp_dir.join("readme.md");
        let target = temp_dir.join("downloaded.md");
        tokio::fs::write(&source, b"hello baidu cloud")
            .await
            .unwrap();

        let upload = provider
            .upload_file(&source, "/apps/SyncFlow/Notes/readme.md", None)
            .await
            .unwrap();
        assert_eq!(upload.entry.size, 17);

        let entries = provider
            .list_directory("/apps/SyncFlow/Notes")
            .await
            .unwrap();
        assert!(entries
            .iter()
            .any(|entry| entry.remote_path == "/apps/SyncFlow/Notes/readme.md"));

        provider
            .download_file("/apps/SyncFlow/Notes/readme.md", &target)
            .await
            .unwrap();
        assert_eq!(
            tokio::fs::read(&target).await.unwrap(),
            b"hello baidu cloud"
        );

        tokio::fs::remove_dir_all(&temp_dir).await.ok();
    }
}
