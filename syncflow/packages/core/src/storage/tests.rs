use chrono::Utc;
use uuid::Uuid;

use super::*;

async fn create_test_engine() -> StorageEngine {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    initialize_schema(&pool).await.unwrap();
    StorageEngine::new(pool)
}

#[tokio::test]
async fn test_initialize_schema_migrates_legacy_file_metadata() {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query(
        r#"
        CREATE TABLE file_metadata (
            path TEXT PRIMARY KEY,
            hash TEXT NOT NULL,
            size INTEGER NOT NULL,
            modified_at TEXT NOT NULL,
            version_vector TEXT NOT NULL,
            created_at TEXT NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO file_metadata (path, hash, size, modified_at, version_vector, created_at)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind("legacy/readme.md")
    .bind("legacy-hash")
    .bind(42_i64)
    .bind(&now)
    .bind("{}")
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    initialize_schema(&pool).await.unwrap();
    let engine = StorageEngine::new(pool);

    let legacy_space = Uuid::nil();
    let migrated = engine
        .get_file_meta(&legacy_space, "legacy/readme.md")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(migrated.hash, "legacy-hash");
    assert_eq!(migrated.size, 42);

    let new_space = Uuid::new_v4();
    let meta = FileMetadata {
        space_id: new_space,
        relative_path: "fresh.txt".to_string(),
        hash: "fresh-hash".to_string(),
        size: 7,
        modified_at: Utc::now(),
        version_vector: "{}".to_string(),
        created_at: Utc::now(),
    };
    engine.save_file_meta(&meta).await.unwrap();
    assert!(engine
        .get_file_meta(&new_space, "fresh.txt")
        .await
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn test_add_list_and_remove_synced_spaces() {
    let engine = create_test_engine().await;
    let now = Utc::now();
    let space = SyncedSpace {
        id: Uuid::new_v4(),
        sync_key: "sync-key-docs".to_string(),
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
    assert_eq!(fetched.sync_key, "sync-key-docs");

    let fetched_by_sync_key = engine
        .get_synced_space_by_sync_key("sync-key-docs")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fetched_by_sync_key.id, space.id);

    let removed = engine.remove_synced_space(&space.id).await.unwrap();
    assert!(removed);
    assert!(engine.get_synced_spaces().await.unwrap().is_empty());
}

#[tokio::test]
async fn test_save_and_get_local_account() {
    let engine = create_test_engine().await;
    let account_id = Uuid::new_v4();
    let now = Utc::now();
    let account = AccountRecord {
        account_id,
        display_name: "Alice".to_string(),
        password_salt: vec![1, 2, 3, 4],
        encrypted_account_secret: vec![5, 6, 7, 8],
        created_at: now,
        last_unlocked_at: None,
    };

    engine.save_account(&account).await.unwrap();

    let fetched = engine.get_local_account().await.unwrap().unwrap();
    assert_eq!(fetched.account_id, account_id);
    assert_eq!(fetched.display_name, "Alice");
    assert_eq!(fetched.password_salt, vec![1, 2, 3, 4]);
    assert_eq!(fetched.encrypted_account_secret, vec![5, 6, 7, 8]);
    assert!(fetched.last_unlocked_at.is_none());

    let unlocked_at = Utc::now();
    engine
        .update_account_last_unlocked_at(&account_id, unlocked_at)
        .await
        .unwrap();
    let fetched = engine.get_local_account().await.unwrap().unwrap();
    assert!(fetched.last_unlocked_at.is_some());
}

#[tokio::test]
async fn test_replace_local_account() {
    let engine = create_test_engine().await;
    let first = AccountRecord {
        account_id: Uuid::new_v4(),
        display_name: "First".to_string(),
        password_salt: vec![1],
        encrypted_account_secret: vec![2],
        created_at: Utc::now(),
        last_unlocked_at: None,
    };
    let second = AccountRecord {
        account_id: Uuid::new_v4(),
        display_name: "Second".to_string(),
        password_salt: vec![3],
        encrypted_account_secret: vec![4],
        created_at: Utc::now(),
        last_unlocked_at: Some(Utc::now()),
    };

    engine.save_account(&first).await.unwrap();
    engine.replace_local_account(&second).await.unwrap();

    let fetched = engine.get_local_account().await.unwrap().unwrap();
    assert_eq!(fetched.account_id, second.account_id);
    assert_eq!(fetched.display_name, "Second");
    assert_eq!(fetched.password_salt, vec![3]);
}

#[tokio::test]
async fn test_add_synced_space_deduplicates_root_path() {
    let engine = create_test_engine().await;
    let root_path = "/tmp/shared".to_string();

    let first = SyncedSpace {
        id: Uuid::new_v4(),
        sync_key: "sync-key-shared".to_string(),
        name: "Shared".to_string(),
        root_path: root_path.clone(),
        status: "Monitoring".to_string(),
        created_at: Utc::now(),
        last_scanned_at: None,
    };

    let second = SyncedSpace {
        id: Uuid::new_v4(),
        sync_key: "sync-key-other".to_string(),
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
    let space_id = Uuid::new_v4();
    let meta = FileMetadata {
        space_id,
        relative_path: "docs/readme.md".to_string(),
        hash: "abc123".to_string(),
        size: 1024,
        modified_at: Utc::now(),
        version_vector: "v1".to_string(),
        created_at: Utc::now(),
    };

    engine.save_file_meta(&meta).await.unwrap();

    let fetched = engine
        .get_file_meta(&space_id, "docs/readme.md")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fetched.relative_path, "docs/readme.md");
    assert_eq!(fetched.hash, "abc123");
    assert_eq!(fetched.size, 1024);
    assert_eq!(fetched.version_vector, "v1");

    let updated = FileMetadata {
        space_id,
        relative_path: "docs/readme.md".to_string(),
        hash: "def456".to_string(),
        size: 2048,
        modified_at: Utc::now(),
        version_vector: "v2".to_string(),
        created_at: Utc::now(),
    };
    engine.save_file_meta(&updated).await.unwrap();

    let fetched = engine
        .get_file_meta(&space_id, "docs/readme.md")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fetched.hash, "def456");
    assert_eq!(fetched.size, 2048);
    assert_eq!(fetched.version_vector, "v2");

    let missing = engine
        .get_file_meta(&space_id, "nonexistent.md")
        .await
        .unwrap();
    assert!(missing.is_none());

    let removed = engine
        .remove_file_meta(&space_id, "docs/readme.md")
        .await
        .unwrap();
    assert!(removed);
    assert_eq!(engine.count_files_for_space(&space_id).await.unwrap(), 0);
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

#[tokio::test]
async fn test_conflict_insert_deduplicates_matching_record() {
    let engine = create_test_engine().await;
    let conflict = SyncConflict {
        id: 0,
        space_id: Uuid::new_v4(),
        relative_path: "docs/readme.md".to_string(),
        local_version: "{\"a\":1}".to_string(),
        remote_version: "{\"b\":1}".to_string(),
        remote_device_id: Uuid::new_v4().to_string(),
        detected_at: Utc::now(),
    };

    engine.save_conflict(&conflict).await.unwrap();
    engine.save_conflict(&conflict).await.unwrap();

    let conflicts = engine
        .get_conflicts_for_space(&conflict.space_id)
        .await
        .unwrap();
    assert_eq!(conflicts.len(), 1);
}

#[tokio::test]
async fn test_save_get_and_remove_conflict_snapshots() {
    let engine = create_test_engine().await;
    let conflict = SyncConflict {
        id: 0,
        space_id: Uuid::new_v4(),
        relative_path: "docs/readme.md".to_string(),
        local_version: "{\"a\":1}".to_string(),
        remote_version: "{\"b\":1}".to_string(),
        remote_device_id: Uuid::new_v4().to_string(),
        detected_at: Utc::now(),
    };

    engine.save_conflict(&conflict).await.unwrap();
    let saved = engine
        .find_matching_conflict(&conflict)
        .await
        .unwrap()
        .unwrap();

    let snapshot = ConflictSnapshot {
        id: 0,
        conflict_id: saved.id,
        space_id: saved.space_id,
        relative_path: saved.relative_path.clone(),
        snapshot_kind: "remote_text".to_string(),
        content_text: Some("remote body".to_string()),
        content_truncated: false,
        content_size: 11,
        created_at: Utc::now(),
    };

    engine.save_conflict_snapshot(&snapshot).await.unwrap();
    engine.save_conflict_snapshot(&snapshot).await.unwrap();

    let snapshots = engine.get_conflict_snapshots(saved.id).await.unwrap();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].content_text.as_deref(), Some("remote body"));

    let removed = engine.remove_conflict(saved.id).await.unwrap();
    assert!(removed);
    assert!(engine.get_conflict_by_id(saved.id).await.unwrap().is_none());
    assert!(engine
        .get_conflict_snapshots(saved.id)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn test_save_get_and_remove_cloud_api_config() {
    let engine = create_test_engine().await;
    let now = Utc::now();
    let config = CloudApiConfig {
        provider: "baidu_netdisk".to_string(),
        device_id: Some("app-id".to_string()),
        client_id: "client-id".to_string(),
        client_secret: Some("client-secret".to_string()),
        redirect_uri: "http://127.0.0.1:18082/baidu/oauth/callback".to_string(),
        scopes: vec!["basic".to_string(), "netdisk".to_string()],
        created_at: now,
        updated_at: now,
    };

    engine.save_cloud_api_config(&config).await.unwrap();
    let fetched = engine
        .get_cloud_api_config("baidu_netdisk")
        .await
        .unwrap()
        .unwrap();

    assert_eq!(fetched.provider, config.provider);
    assert_eq!(fetched.device_id, config.device_id);
    assert_eq!(fetched.client_id, config.client_id);
    assert_eq!(fetched.client_secret, config.client_secret);
    assert_eq!(fetched.redirect_uri, config.redirect_uri);
    assert_eq!(fetched.scopes, config.scopes);

    assert!(engine
        .remove_cloud_api_config("baidu_netdisk")
        .await
        .unwrap());
    assert!(engine
        .get_cloud_api_config("baidu_netdisk")
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn test_save_and_get_cloud_account() {
    let engine = create_test_engine().await;
    let now = Utc::now();
    let account = CloudAccount {
        provider: "baidu_netdisk".to_string(),
        account_id: Some("baidu-user-1".to_string()),
        display_name: Some("Baidu User".to_string()),
        access_token_encrypted: b"encrypted-access".to_vec(),
        refresh_token_encrypted: b"encrypted-refresh".to_vec(),
        expires_at: Some(now),
        scopes: vec!["basic".to_string(), "netdisk".to_string()],
        created_at: now,
        updated_at: now,
    };

    engine.save_cloud_account(&account).await.unwrap();
    let fetched = engine
        .get_cloud_account("baidu_netdisk")
        .await
        .unwrap()
        .unwrap();

    assert_eq!(fetched.provider, account.provider);
    assert_eq!(fetched.account_id, account.account_id);
    assert_eq!(fetched.display_name, account.display_name);
    assert_eq!(
        fetched.access_token_encrypted,
        account.access_token_encrypted
    );
    assert_eq!(
        fetched.refresh_token_encrypted,
        account.refresh_token_encrypted
    );
    assert_eq!(fetched.scopes, account.scopes);

    assert!(engine.remove_cloud_account("baidu_netdisk").await.unwrap());
    assert!(engine
        .get_cloud_account("baidu_netdisk")
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn test_save_cloud_space_binding_and_remote_metadata() {
    let engine = create_test_engine().await;
    let now = Utc::now();
    let space_id = Uuid::new_v4();
    let binding = CloudSpaceBinding {
        space_id,
        provider: "baidu_netdisk".to_string(),
        remote_root_path: "/apps/SyncFlow/Notes".to_string(),
        remote_root_id: Some("root-fs-id".to_string()),
        sync_mode: "bidirectional".to_string(),
        plaintext: true,
        created_at: now,
        updated_at: now,
    };

    engine.save_cloud_space_binding(&binding).await.unwrap();
    let fetched = engine
        .get_cloud_space_binding(&space_id, "baidu_netdisk")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fetched, binding);

    let metadata = RemoteFileMetadata {
        space_id,
        provider: "baidu_netdisk".to_string(),
        remote_path: "/apps/SyncFlow/Notes/docs/readme.md".to_string(),
        local_relative_path: "docs/readme.md".to_string(),
        remote_file_id: Some("fs-123".to_string()),
        is_directory: false,
        size: 128,
        md5: Some("md5-value".to_string()),
        server_mtime: Some(now),
        remote_revision: Some("rev-1".to_string()),
        last_remote_file_id: Some("fs-123".to_string()),
        last_remote_md5: Some("md5-value".to_string()),
        last_remote_size: Some(128),
        last_remote_server_mtime: Some(now),
        last_remote_revision: Some("rev-1".to_string()),
        last_local_hash: Some("local-hash".to_string()),
        last_local_modified_at: Some(now),
        last_local_size: Some(128),
        last_seen_at: now,
        last_synced_at: Some(now),
        tombstone: false,
    };

    engine.save_remote_file_metadata(&metadata).await.unwrap();
    let fetched = engine
        .get_remote_file_metadata(&space_id, "baidu_netdisk", "docs/readme.md")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fetched, metadata);
    let all = engine
        .list_remote_file_metadata(&space_id, "baidu_netdisk")
        .await
        .unwrap();
    assert_eq!(all, vec![metadata]);
}

#[tokio::test]
async fn test_enqueue_get_and_remove_cloud_sync_task() {
    let engine = create_test_engine().await;
    let now = Utc::now();
    let task = CloudSyncTask {
        id: 0,
        space_id: Uuid::new_v4(),
        provider: "baidu_netdisk".to_string(),
        task_kind: "upload".to_string(),
        local_relative_path: "docs/readme.md".to_string(),
        remote_path: "/apps/SyncFlow/Notes/docs/readme.md".to_string(),
        expected_remote_revision: Some("rev-1".to_string()),
        payload_json: Some("{\"reason\":\"watcher\"}".to_string()),
        attempts: 0,
        last_error: None,
        created_at: now,
        updated_at: now,
        next_attempt_at: Some(now),
    };

    let task_id = engine.enqueue_cloud_sync_task(&task).await.unwrap();
    let due = engine
        .get_due_cloud_sync_tasks("baidu_netdisk", now, 10)
        .await
        .unwrap();
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].id, task_id);
    assert_eq!(due[0].task_kind, "upload");
    assert_eq!(due[0].expected_remote_revision.as_deref(), Some("rev-1"));

    assert!(engine.remove_cloud_sync_task(task_id).await.unwrap());
    assert!(engine
        .get_due_cloud_sync_tasks("baidu_netdisk", now, 10)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn test_enqueue_cloud_sync_task_deduplicates_pending_path() {
    let engine = create_test_engine().await;
    let now = Utc::now();
    let space_id = Uuid::new_v4();
    let mut task = CloudSyncTask {
        id: 0,
        space_id,
        provider: "baidu_netdisk".to_string(),
        task_kind: "upload".to_string(),
        local_relative_path: "docs/readme.md".to_string(),
        remote_path: "/apps/SyncFlow/Notes/docs/readme.md".to_string(),
        expected_remote_revision: Some("rev-1".to_string()),
        payload_json: None,
        attempts: 2,
        last_error: Some("old error".to_string()),
        created_at: now,
        updated_at: now,
        next_attempt_at: Some(now + chrono::Duration::minutes(5)),
    };

    let first_id = engine.enqueue_cloud_sync_task(&task).await.unwrap();
    task.expected_remote_revision = Some("rev-2".to_string());
    task.attempts = 0;
    task.last_error = None;
    task.updated_at = now + chrono::Duration::seconds(1);
    task.next_attempt_at = Some(now);
    let second_id = engine.enqueue_cloud_sync_task(&task).await.unwrap();

    assert_eq!(first_id, second_id);
    let tasks = engine
        .list_cloud_sync_tasks_for_space(&space_id, "baidu_netdisk", 10)
        .await
        .unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].expected_remote_revision.as_deref(), Some("rev-2"));
    assert_eq!(tasks[0].attempts, 0);
    assert!(tasks[0].last_error.is_none());
    assert_eq!(tasks[0].next_attempt_at, Some(now));
}

#[tokio::test]
async fn test_retry_and_remove_cloud_sync_task_for_space() {
    let engine = create_test_engine().await;
    let now = Utc::now();
    let space_id = Uuid::new_v4();
    let other_space_id = Uuid::new_v4();
    let task = CloudSyncTask {
        id: 0,
        space_id,
        provider: "baidu_netdisk".to_string(),
        task_kind: "upload".to_string(),
        local_relative_path: "docs/readme.md".to_string(),
        remote_path: "/apps/SyncFlow/Notes/docs/readme.md".to_string(),
        expected_remote_revision: None,
        payload_json: None,
        attempts: 3,
        last_error: Some("network error".to_string()),
        created_at: now,
        updated_at: now,
        next_attempt_at: Some(now + chrono::Duration::minutes(5)),
    };

    let task_id = engine.enqueue_cloud_sync_task(&task).await.unwrap();
    assert!(!engine
        .retry_cloud_sync_task(task_id, &other_space_id, "baidu_netdisk", now)
        .await
        .unwrap());
    assert!(engine
        .retry_cloud_sync_task(task_id, &space_id, "baidu_netdisk", now)
        .await
        .unwrap());

    let tasks = engine
        .list_cloud_sync_tasks_for_space(&space_id, "baidu_netdisk", 10)
        .await
        .unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].attempts, 0);
    assert!(tasks[0].last_error.is_none());
    assert_eq!(tasks[0].next_attempt_at, Some(now));

    assert!(!engine
        .remove_cloud_sync_task_for_space(task_id, &other_space_id, "baidu_netdisk")
        .await
        .unwrap());
    assert!(engine
        .remove_cloud_sync_task_for_space(task_id, &space_id, "baidu_netdisk")
        .await
        .unwrap());
}
