use chrono::Utc;
use uuid::Uuid;

use super::*;

async fn create_test_engine() -> StorageEngine {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    initialize_schema(&pool).await.unwrap();
    StorageEngine::new(pool)
}

#[tokio::test]
async fn test_add_list_and_remove_synced_spaces() {
    let engine = create_test_engine().await;
    let now = Utc::now();
    let space = SyncedSpace {
        id: Uuid::new_v4(),
        name: "Documents".to_string(),
        root_path: "/tmp/documents".to_string(),
        status: "Monitoring".to_string(),
        created_at: now,
        last_scanned_at: Some(now),
    };

    let inserted = engine.add_synced_space(&space).await.unwrap();
    assert_eq!(inserted.id, space.id);
    assert_eq!(inserted.root_path, "/tmp/documents");

    let spaces = engine.get_synced_spaces().await.unwrap();
    assert_eq!(spaces.len(), 1);
    assert_eq!(spaces[0].name, "Documents");
    assert_eq!(spaces[0].status, "Monitoring");
    assert!(spaces[0].last_scanned_at.is_some());

    let fetched = engine.get_synced_space(&space.id).await.unwrap().unwrap();
    assert_eq!(fetched.created_at.timestamp(), now.timestamp());

    let removed = engine.remove_synced_space(&space.id).await.unwrap();
    assert!(removed);
    assert!(engine.get_synced_spaces().await.unwrap().is_empty());
}

#[tokio::test]
async fn test_add_synced_space_deduplicates_root_path() {
    let engine = create_test_engine().await;
    let root_path = "/tmp/shared".to_string();

    let first = SyncedSpace {
        id: Uuid::new_v4(),
        name: "Shared".to_string(),
        root_path: root_path.clone(),
        status: "Monitoring".to_string(),
        created_at: Utc::now(),
        last_scanned_at: None,
    };

    let second = SyncedSpace {
        id: Uuid::new_v4(),
        name: "Other Name".to_string(),
        root_path,
        status: "Monitoring".to_string(),
        created_at: Utc::now(),
        last_scanned_at: Some(Utc::now()),
    };

    let inserted_first = engine.add_synced_space(&first).await.unwrap();
    let inserted_second = engine.add_synced_space(&second).await.unwrap();

    assert_eq!(inserted_first.id, inserted_second.id);
    assert_eq!(engine.get_synced_spaces().await.unwrap().len(), 1);
}

#[tokio::test]
async fn test_save_and_get_file_meta() {
    let engine = create_test_engine().await;
    let meta = FileMetadata {
        path: "docs/readme.md".to_string(),
        hash: "abc123".to_string(),
        size: 1024,
        modified_at: Utc::now(),
        version_vector: "v1".to_string(),
        created_at: Utc::now(),
    };

    engine.save_file_meta(&meta).await.unwrap();

    let fetched = engine
        .get_file_meta("docs/readme.md")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fetched.path, "docs/readme.md");
    assert_eq!(fetched.hash, "abc123");
    assert_eq!(fetched.size, 1024);
    assert_eq!(fetched.version_vector, "v1");

    // Test upsert: update the same file
    let updated = FileMetadata {
        path: "docs/readme.md".to_string(),
        hash: "def456".to_string(),
        size: 2048,
        modified_at: Utc::now(),
        version_vector: "v2".to_string(),
        created_at: Utc::now(),
    };
    engine.save_file_meta(&updated).await.unwrap();

    let fetched = engine
        .get_file_meta("docs/readme.md")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fetched.hash, "def456");
    assert_eq!(fetched.size, 2048);
    assert_eq!(fetched.version_vector, "v2");

    // Test nonexistent file
    let missing = engine.get_file_meta("nonexistent.md").await.unwrap();
    assert!(missing.is_none());
}

#[tokio::test]
async fn test_save_and_get_sync_state() {
    let engine = create_test_engine().await;

    let peer_id = Uuid::new_v4();
    let state = SyncState {
        peer_id,
        last_sync_at: Some(Utc::now()),
        sync_status: SyncStatus::Idle,
        pending_changes: 0,
    };

    engine.save_sync_state(&state).await.unwrap();

    let fetched = engine.get_sync_state(&peer_id).await.unwrap().unwrap();
    assert_eq!(fetched.peer_id, peer_id);
    assert_eq!(fetched.sync_status, SyncStatus::Idle);
    assert_eq!(fetched.pending_changes, 0);

    // Update status
    let syncing = SyncState {
        peer_id,
        last_sync_at: Some(Utc::now()),
        sync_status: SyncStatus::Syncing,
        pending_changes: 5,
    };
    engine.save_sync_state(&syncing).await.unwrap();

    let fetched = engine.get_sync_state(&peer_id).await.unwrap().unwrap();
    assert_eq!(fetched.sync_status, SyncStatus::Syncing);
    assert_eq!(fetched.pending_changes, 5);

    // Test nonexistent peer
    let missing = engine.get_sync_state(&Uuid::new_v4()).await.unwrap();
    assert!(missing.is_none());
}

#[tokio::test]
async fn test_save_and_get_device_info() {
    let engine = create_test_engine().await;

    let device_id = Uuid::new_v4();
    let info = DeviceInfo {
        device_id,
        device_name: "My Laptop".to_string(),
        platform: "linux".to_string(),
        public_key: "ed25519-pubkey-base64".to_string(),
        last_seen_at: Some(Utc::now()),
    };

    engine.save_device_info(&info).await.unwrap();

    let devices = engine.get_known_devices().await.unwrap();
    assert_eq!(devices.len(), 1);
    let fetched = &devices[0];
    assert_eq!(fetched.device_id, device_id);
    assert_eq!(fetched.device_name, "My Laptop");
    assert_eq!(fetched.platform, "linux");
    assert_eq!(fetched.public_key, "ed25519-pubkey-base64");

    // Add another device
    let device2 = DeviceInfo {
        device_id: Uuid::new_v4(),
        device_name: "My Phone".to_string(),
        platform: "android".to_string(),
        public_key: "ed25519-pubkey2".to_string(),
        last_seen_at: None,
    };
    engine.save_device_info(&device2).await.unwrap();

    let devices = engine.get_known_devices().await.unwrap();
    assert_eq!(devices.len(), 2);
}
