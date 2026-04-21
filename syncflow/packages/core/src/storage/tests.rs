use chrono::Utc;
use uuid::Uuid;

use super::*;

const CREATE_TABLES: &str = r#"
CREATE TABLE IF NOT EXISTS file_metadata (
    path TEXT PRIMARY KEY,
    hash TEXT NOT NULL,
    size INTEGER NOT NULL,
    modified_at TEXT NOT NULL,
    version_vector TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sync_state (
    peer_id TEXT PRIMARY KEY,
    last_sync_at TEXT,
    sync_status TEXT NOT NULL DEFAULT 'idle',
    pending_changes INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS file_versions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path TEXT NOT NULL,
    hash TEXT NOT NULL,
    version_vector TEXT NOT NULL,
    device_id TEXT NOT NULL,
    is_conflict INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS devices (
    device_id TEXT PRIMARY KEY,
    device_name TEXT NOT NULL,
    platform TEXT NOT NULL,
    public_key TEXT NOT NULL,
    last_seen_at TEXT
);
"#;

async fn create_test_engine() -> StorageEngine {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query(CREATE_TABLES).execute(&pool).await.unwrap();
    StorageEngine::new(pool)
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
