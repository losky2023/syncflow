# SyncFlow Baidu Netdisk Cloud Sync Design

Date: 2026-04-27

## Summary

SyncFlow will shift its primary synchronization path from LAN peer-to-peer WebRTC transfer to a cloud-mediated model backed by Baidu Netdisk Open Platform APIs. In this design, Baidu Netdisk is not just a private relay object store. It is the visible cloud copy of each synchronized space.

The approved first version uses:

- official Baidu Netdisk Open Platform APIs,
- a Baidu app-scoped directory such as `/apps/SyncFlow/<space>/`,
- plaintext files in Baidu Netdisk,
- bidirectional synchronization between local folders and their Baidu Netdisk folders,
- conservative conflict handling that never silently overwrites divergent local and cloud edits.

The existing LAN/P2P transport remains in the codebase during migration, but cloud sync becomes the default runtime path for new spaces.

## Goals

- Replace the current end-to-end P2P sync flow with Baidu Netdisk mediated cloud sync.
- Bind each local sync space to a Baidu Netdisk app directory.
- Support bidirectional file and directory synchronization:
  - local create/modify/delete uploads to Baidu Netdisk,
  - cloud create/modify/delete downloads to the local folder.
- Keep Baidu Netdisk files plaintext so users can see and open them from Baidu Netdisk clients and web UI.
- Use official OAuth and file APIs instead of depending on the desktop Baidu Netdisk client.
- Reuse existing local watcher, path safety, metadata, and conflict-resolution foundations where practical.
- Make conflicts explicit and recoverable instead of using last-writer-wins overwrites.

## Non-goals

- Arbitrary Baidu Netdisk folder sync outside the app-scoped directory in the first version.
- End-to-end encryption for cloud files in this mode.
- LAN P2P optimization or WebRTC transport in the new default sync flow.
- Real-time push notifications from Baidu Netdisk unless the official platform provides a stable change feed for the app.
- Full rsync-style binary delta transfer.
- UI for advanced upload internals such as slice retry visualization.
- Multi-account Baidu login in the first version.
- Collaborative editing or file locking.

## Approved Decisions

- **Cloud provider**: Baidu Netdisk Open Platform.
- **Cloud directory model**: app-scoped visible directory, expected to be `/apps/SyncFlow/<space>/` or equivalent official app directory path.
- **Sync direction**: bidirectional.
- **Cloud file format**: plaintext original files.
- **Conflict behavior**: detect divergence and preserve both sides; do not silently overwrite.
- **Migration approach**: add a separate cloud sync runtime instead of trying to reshape the WebRTC transport into a cloud API adapter.

## External API Assumptions

This design depends on Baidu Netdisk Open Platform capabilities that must be verified against official documentation during implementation:

- OAuth authorization for user login and token refresh.
- File listing for directories under the app-scoped path.
- File upload, including the platform's required pre-create, upload, and create/commit flow when applicable.
- File download URL acquisition or equivalent download API.
- File management APIs for mkdir, delete, move, rename, and metadata lookup.
- Stable metadata fields such as file id, path, size, server modified time, md5, and directory/file marker.
- Error codes for token expiration, quota exhaustion, permission failure, rate limiting, and file conflicts.

Useful official starting points to verify before coding:

- `https://pan.baidu.com/union/doc/`
- `https://pan.baidu.com/union/document/entrance`
- Baidu Netdisk Open Platform OAuth documentation.
- Baidu Netdisk Open Platform file management, upload, and download API documentation.

If the official platform restricts writes to `/apps/<app-name>/`, SyncFlow will treat that as the supported cloud root. If the platform later allows broader user-selected folders with stable permission scopes, arbitrary cloud-folder binding can be designed as a later phase.

## Current State

SyncFlow currently centers on LAN-discovered peers and WebRTC data channels.

Relevant current areas:

- `syncflow/packages/core/src/sync/mod.rs` indexes local files, enqueues tasks, encrypts payloads, sends data through `TransportLayer`, and applies received files.
- `syncflow/packages/core/src/sync/queue.rs` represents upload/delete tasks targeted at peers.
- `syncflow/packages/core/src/transport/` handles mDNS discovery, SDP exchange, WebRTC peer connections, and data channel delivery.
- `syncflow/packages/client/src-tauri/src/runtime/manager.rs` starts per-space watchers, queue loops, peer onboarding, and inbound transport message handling.
- `syncflow/packages/core/src/storage/` persists spaces, file metadata, conflicts, and local account material.
- Existing conflict work already stores conflict rows and supports text snapshot based workflows.

Why the current transport should not be directly reused:

- WebRTC sends point-to-point messages; Baidu Netdisk exposes remote filesystem state.
- Peer readiness is event-driven; cloud sync needs polling, remote metadata comparison, and retry/backoff.
- P2P payloads are encrypted by `sync_key`; approved cloud sync stores plaintext files.
- Existing queue tasks target `peer_id`; cloud tasks target `remote_path` and API operations.

## Proposed Architecture

### Cloud provider abstraction

Add a cloud provider boundary in core:

```text
CloudProvider
  authenticate / refresh_token
  list_directory(remote_path)
  get_metadata(remote_path)
  create_directory(remote_path)
  upload_file(local_path, remote_path, expected_remote?)
  download_file(remote_path, local_path)
  delete_path(remote_path, expected_remote?)
  move_path(from, to, expected_remote?)
```

The first implementation is `BaiduNetdiskProvider`.

This boundary keeps Baidu-specific HTTP details out of sync orchestration and allows tests to use an in-memory fake provider.

### Space cloud binding

Each local `synced_spaces` row gets cloud binding data, either in a new table or dedicated nullable columns. A separate table is preferred:

```text
cloud_space_bindings
  space_id
  provider
  remote_root_path
  remote_root_id?
  sync_mode
  plaintext
  created_at
  updated_at
```

Example binding:

```text
space_id: 018f...
provider: baidu_netdisk
remote_root_path: /apps/SyncFlow/Project Notes
sync_mode: bidirectional
plaintext: true
```

### OAuth token storage

Add provider account storage:

```text
cloud_accounts
  provider
  account_id?
  display_name?
  access_token_encrypted
  refresh_token_encrypted
  expires_at
  scopes
  created_at
  updated_at
```

Tokens must be encrypted at rest using the existing local secure storage pattern or an OS-backed secret store when available. They must never be logged.

### Remote metadata cache

Add a cache for last-known cloud state:

```text
remote_file_metadata
  space_id
  provider
  remote_path
  local_relative_path
  remote_file_id?
  is_directory
  size
  md5?
  server_mtime?
  remote_revision?
  last_seen_at
  last_synced_at
  tombstone
```

This table is the basis for detecting:

- cloud-only additions,
- cloud modifications,
- cloud deletions,
- local-only additions,
- local modifications,
- local deletions,
- divergent local and cloud changes.

## Sync Data Flow

### Initial bind

When a user binds a local space to Baidu Netdisk:

1. Complete Baidu OAuth login if no valid account token exists.
2. Create or select `/apps/SyncFlow/<space-name>/`.
3. Scan the local root using existing safe path traversal.
4. List the remote root recursively.
5. Build local and remote manifests.
6. Reconcile initial state:
   - identical relative paths with identical content metadata become synced,
   - local-only files upload,
   - remote-only files download,
   - divergent files create conflicts.
7. Start local watcher and cloud polling loop.

### Local to cloud

For local create or modify:

1. Watcher emits a file event.
2. Runtime resolves the event under the space root and validates the relative path.
3. Local metadata is indexed.
4. A cloud upload task is enqueued with expected previous remote metadata when known.
5. Provider uploads the plaintext file to the corresponding remote path.
6. Remote metadata cache is refreshed for that path.
7. UI status and pending counts update.

For local delete:

1. Watcher emits delete event.
2. Runtime marks local metadata removed.
3. A cloud delete task is enqueued with expected previous remote metadata.
4. Provider deletes the remote path if it still matches the known remote state.
5. If the cloud changed meanwhile, create a conflict instead of deleting.

### Cloud to local

A cloud polling loop runs per active cloud-bound space:

1. List remote directory recursively or consume official incremental change API if available.
2. Compare remote manifest with `remote_file_metadata` and local metadata.
3. For cloud additions or modifications, enqueue downloads.
4. For cloud deletions, delete local paths only if local state has not changed since last sync.
5. Suppress watcher echo for files written by cloud downloads.
6. Update metadata and status.

Polling should use backoff:

- fast interval after local activity,
- normal interval during active sync,
- slower interval after repeated no-change scans,
- longer backoff on rate-limit or transient provider errors.

## Conflict Handling

A conflict occurs when both local and cloud versions changed since the last synced baseline for the same relative path.

First-version conflict policy:

- Never overwrite local or cloud divergent content automatically.
- Download the cloud version to a conflict filename locally:
  - `name (baidu conflict 2026-04-27 153000).ext`
- Keep the original local file unchanged.
- Insert a `sync_conflicts` row with remote provider metadata.
- For text files, reuse or extend existing conflict snapshot support so the UI can compare local and cloud text content.
- Leave both files visible locally so users can recover even if the UI workflow is incomplete.

For cloud-side conflicts, the uploaded local conflict copy can either:

- remain local-only until the user resolves, or
- upload under a conflict filename as well.

The recommended first version keeps conflict copies local-only to avoid cluttering the Baidu Netdisk folder until explicit resolution behavior is designed.

## Delete Semantics

Deletes are high risk in bidirectional cloud sync. The first version should use guarded deletes:

- A local delete propagates to cloud only when remote metadata still matches the last synced remote metadata.
- A cloud delete propagates to local only when local metadata still matches the last synced local metadata.
- If either side changed after the last baseline, record a conflict and preserve content.
- Directory deletes are expanded into file-level decisions where possible.

A later version can add trash/recycle-bin integration if Baidu APIs expose it reliably.

## UI Changes

Add a Baidu cloud setup flow:

- connect Baidu Netdisk account,
- show authorized account status,
- create or bind a sync space to `/apps/SyncFlow/<space>/`,
- show plaintext warning and privacy implications,
- display cloud sync status per space.

Update runtime status UI:

- provider: Baidu Netdisk,
- cloud path,
- account status,
- last cloud scan time,
- pending upload count,
- pending download count,
- last successful sync time,
- last provider error,
- quota/rate-limit warning when known.

Existing peer-oriented indicators can remain during migration but should not be primary for cloud-bound spaces.

## Error Handling

Provider errors map to user-actionable categories:

- `AuthExpired`: refresh token or ask user to reconnect.
- `PermissionDenied`: explain app directory permission issue.
- `QuotaExceeded`: pause uploads and show cloud quota warning.
- `RateLimited`: back off and show delayed sync status.
- `NetworkUnavailable`: retry with exponential backoff.
- `RemoteConflict`: convert to sync conflict.
- `PathInvalid`: reject unsafe or unsupported names.
- `ProviderUnavailable`: retry and keep local changes queued.

All retryable tasks must persist enough state to resume after app restart.

## Security And Privacy

- Files are plaintext in Baidu Netdisk by approved product decision.
- The UI must clearly state that Baidu Netdisk and any client logged into the account can access file contents.
- OAuth tokens are sensitive secrets and must be encrypted at rest.
- Logs must not include tokens, authorization headers, download URLs, or full file contents.
- Existing path validation remains mandatory: all local writes must stay under the bound space root.
- Remote paths must be normalized and rejected if they map to unsafe local paths.
- `sync_key` should no longer be treated as the cloud file encryption key for plaintext cloud spaces.

## Migration Strategy

### Phase 1: Foundations

- Add provider trait and fake provider tests.
- Add Baidu OAuth account model and token storage.
- Add cloud binding and remote metadata tables.
- Add Tauri commands for OAuth status and connect/disconnect.

### Phase 2: One-way upload path

- Convert local watcher events into cloud upload/delete tasks.
- Upload plaintext local files into `/apps/SyncFlow/<space>/`.
- Persist remote metadata after successful operations.
- Show cloud status in the UI.

### Phase 3: Cloud polling and downloads

- Recursively list the cloud folder.
- Detect remote additions/modifications/deletions.
- Download cloud changes safely into the local space.
- Suppress watcher echoes for cloud-applied writes.

### Phase 4: Bidirectional conflict safety

- Add baseline-aware conflict detection.
- Preserve both versions on divergence.
- Integrate conflicts with existing conflict list and resolution UI.
- Add guarded delete behavior.

### Phase 5: Make cloud sync default

- New spaces default to Baidu cloud sync.
- LAN/P2P remains disabled or hidden unless explicitly enabled as legacy/experimental.
- Update docs and validation runbooks.

## Testing Strategy

Core tests should use a fake cloud provider:

- upload local-only file to remote path,
- download remote-only file to local path,
- update local modified file in cloud,
- update cloud modified file locally,
- detect local/cloud divergent edit as conflict,
- guard local delete when cloud changed,
- guard cloud delete when local changed,
- reject unsafe remote paths,
- resume queued tasks after simulated restart.

Integration tests with real Baidu APIs should be opt-in because they require credentials and network access:

- OAuth callback flow,
- mkdir/list/upload/download/delete under `/apps/SyncFlow-test/`,
- token refresh,
- rate-limit/backoff behavior where practical.

Manual validation should cover:

- two devices bound to the same Baidu account and space,
- web UI edit followed by local download,
- local edit followed by Baidu web/client visibility,
- same-path edit conflict,
- offline local edits followed by reconnect,
- cloud quota or permission failure messaging.

## Open Questions

- What exact app directory name will Baidu assign: `SyncFlow`, localized app name, or platform-controlled app folder?
- Does the official API expose a reliable incremental change feed, or must SyncFlow use recursive polling?
- Which metadata field is reliable enough as a remote revision: `fs_id`, `md5`, `server_mtime`, or a combination?
- What upload size thresholds require multipart upload?
- Does the API allow safe conditional delete/update, or must SyncFlow emulate conditional operations by re-reading metadata immediately before writes?
- How should existing P2P spaces migrate: opt-in per space, one-time migration wizard, or new cloud spaces only?

## Approval Gate

This design is approved at the product direction level with the following fixed choices:

- Baidu Netdisk official API,
- app-scoped Baidu directory,
- bidirectional sync,
- plaintext cloud files,
- separate cloud sync runtime.

Implementation should not start until official Baidu API details above are verified and the resulting implementation plan is reviewed.
