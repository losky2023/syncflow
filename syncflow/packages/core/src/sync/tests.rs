#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::crypto::{derive_space_key, encrypt_data, hash_data};
    use crate::storage::{initialize_schema, StorageEngine, SyncedSpace};
    use crate::sync::version_vector::*;
    use crate::sync::watcher::FileEvent;
    use crate::sync::SyncEngine;
    use crate::transport::TransportLayer;
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn test_version_vector_new() {
        let vv = VersionVector::new("device_a");
        assert_eq!(vv.get("device_a"), 0);
    }

    #[test]
    fn test_version_vector_increment() {
        let mut vv = VersionVector::new("device_a");
        vv.increment("device_a");
        assert_eq!(vv.get("device_a"), 1);
        vv.increment("device_a");
        assert_eq!(vv.get("device_a"), 2);
    }

    #[test]
    fn test_version_vector_concurrent_means_conflict() {
        let mut vv_a = VersionVector::new("device_a");
        vv_a.increment("device_a");

        let mut vv_b = VersionVector::new("device_b");
        vv_b.increment("device_b");

        assert!(vv_a.is_conflicting(&vv_b));
        assert!(vv_b.is_conflicting(&vv_a));
    }

    #[test]
    fn test_version_vector_causally_ordered() {
        let mut vv_a = VersionVector::new("device_a");
        vv_a.increment("device_a");

        let mut vv_b = vv_a.clone();
        vv_b.increment("device_b");

        assert!(!vv_a.is_conflicting(&vv_b));
        assert!(vv_b.is_newer_than(&vv_a));
    }

    #[test]
    fn test_version_vector_serialize() {
        let mut vv = VersionVector::new("device_a");
        vv.increment("device_a");
        let json = vv.to_json().unwrap();
        let restored = VersionVector::from_json(&json).unwrap();
        assert_eq!(vv.get("device_a"), restored.get("device_a"));
    }

    #[test]
    fn test_file_event_classify() {
        use crate::sync::watcher::FileEvent;
        use std::path::PathBuf;

        let create_event = FileEvent::Created(PathBuf::from("/test/new.txt"));
        assert_eq!(create_event.path(), "/test/new.txt");

        let modify_event = FileEvent::Modified(PathBuf::from("/test/existing.txt"));
        assert_eq!(modify_event.path(), "/test/existing.txt");

        let delete_event = FileEvent::Deleted(PathBuf::from("/test/removed.txt"));
        assert_eq!(delete_event.path(), "/test/removed.txt");
    }

    #[tokio::test]
    async fn test_receive_delete_removes_local_file_and_metadata() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        initialize_schema(&pool).await.unwrap();
        let storage = Arc::new(StorageEngine::new(pool));
        let space_id = Uuid::new_v4();
        let space = SyncedSpace {
            id: space_id,
            sync_key: "sync-key-delete".to_string(),
            name: "Delete Test".to_string(),
            root_path: std::env::temp_dir()
                .join(format!("syncflow-delete-test-{}", Uuid::new_v4()))
                .to_string_lossy()
                .to_string(),
            status: "Monitoring".to_string(),
            created_at: Utc::now(),
            last_scanned_at: None,
        };
        storage.add_synced_space(&space).await.unwrap();

        let root = std::path::PathBuf::from(&space.root_path);
        tokio::fs::create_dir_all(&root).await.unwrap();
        let file_path = root.join("hello.txt");

        let transport = Arc::new(TransportLayer::new("device-local".to_string(), 18080));
        let engine = SyncEngine::new(storage.clone(), transport, "device-local".to_string());

        let mut vv = VersionVector::new("device-remote");
        vv.increment("device-remote");
        let content = b"hello delete";
        let metadata = serde_json::json!({
            "type": "metadata",
            "sync_key": space.sync_key,
            "relative_path": "hello.txt",
            "hash": hash_data(content),
            "size": content.len(),
            "version_vector": vv.to_json().unwrap(),
        });
        let mut message = metadata.to_string().into_bytes();
        message.push(0);
        message.extend(encrypt_data(content, &derive_space_key(&space.sync_key)).unwrap());

        engine
            .receive_space_file("device-remote", Some(&root), Some(space_id), &message)
            .await
            .unwrap();

        assert!(file_path.exists());
        assert!(storage
            .get_file_meta(&space_id, "hello.txt")
            .await
            .unwrap()
            .is_some());

        let delete = serde_json::json!({
            "type": "delete",
            "sync_key": space.sync_key,
            "relative_path": "hello.txt",
        });
        engine
            .receive_space_file(
                "device-remote",
                Some(&root),
                Some(space_id),
                delete.to_string().as_bytes(),
            )
            .await
            .unwrap();

        assert!(!file_path.exists());
        assert!(storage
            .get_file_meta(&space_id, "hello.txt")
            .await
            .unwrap()
            .is_none());

        let _ = tokio::fs::remove_dir_all(&root).await;
    }

    #[tokio::test]
    async fn test_unchanged_local_file_event_does_not_reindex() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        initialize_schema(&pool).await.unwrap();
        let storage = Arc::new(StorageEngine::new(pool));
        let space_id = Uuid::new_v4();
        let root =
            std::env::temp_dir().join(format!("syncflow-unchanged-event-test-{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(&root).await.unwrap();
        let file_path = root.join("hello.txt");
        tokio::fs::write(&file_path, b"same content").await.unwrap();

        let transport = Arc::new(TransportLayer::new("device-local".to_string(), 18080));
        let engine = SyncEngine::new(storage.clone(), transport, "device-local".to_string());

        engine
            .index_local_file(space_id, "hello.txt", &file_path)
            .await
            .unwrap();
        let before = storage
            .get_file_meta(&space_id, "hello.txt")
            .await
            .unwrap()
            .unwrap()
            .version_vector;

        engine
            .handle_space_file_event(
                space_id,
                "hello.txt",
                &file_path,
                &FileEvent::Modified(file_path.clone()),
            )
            .await
            .unwrap();

        let after = storage
            .get_file_meta(&space_id, "hello.txt")
            .await
            .unwrap()
            .unwrap()
            .version_vector;
        assert_eq!(after, before);

        let _ = tokio::fs::remove_dir_all(&root).await;
    }

    #[tokio::test]
    async fn test_text_conflict_stores_remote_snapshot() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        initialize_schema(&pool).await.unwrap();
        let storage = Arc::new(StorageEngine::new(pool));
        let space_id = Uuid::new_v4();
        let space = SyncedSpace {
            id: space_id,
            sync_key: "sync-key-conflict".to_string(),
            name: "Conflict Test".to_string(),
            root_path: std::env::temp_dir()
                .join(format!("syncflow-conflict-test-{}", Uuid::new_v4()))
                .to_string_lossy()
                .to_string(),
            status: "Monitoring".to_string(),
            created_at: Utc::now(),
            last_scanned_at: None,
        };
        storage.add_synced_space(&space).await.unwrap();

        let root = std::path::PathBuf::from(&space.root_path);
        tokio::fs::create_dir_all(&root).await.unwrap();
        let file_path = root.join("note.txt");
        tokio::fs::write(&file_path, b"local body").await.unwrap();

        let transport = Arc::new(TransportLayer::new("device-local".to_string(), 18080));
        let engine = SyncEngine::new(storage.clone(), transport, "device-local".to_string());

        engine
            .index_local_file(space_id, "note.txt", &file_path)
            .await
            .unwrap();

        let mut vv = VersionVector::new("device-remote");
        vv.increment("device-remote");
        let remote_content = b"remote body";
        let metadata = serde_json::json!({
            "type": "metadata",
            "sync_key": space.sync_key,
            "relative_path": "note.txt",
            "hash": hash_data(remote_content),
            "size": remote_content.len(),
            "version_vector": vv.to_json().unwrap(),
        });
        let mut message = metadata.to_string().into_bytes();
        message.push(0);
        message.extend(encrypt_data(remote_content, &derive_space_key(&space.sync_key)).unwrap());

        engine
            .receive_space_file("device-remote", Some(&root), Some(space_id), &message)
            .await
            .unwrap();

        let conflicts = storage.get_conflicts_for_space(&space_id).await.unwrap();
        assert_eq!(conflicts.len(), 1);

        let snapshots = storage
            .get_conflict_snapshots(conflicts[0].id)
            .await
            .unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].snapshot_kind, "remote_text");
        assert_eq!(snapshots[0].content_text.as_deref(), Some("remote body"));

        let _ = tokio::fs::remove_dir_all(&root).await;
    }
}
