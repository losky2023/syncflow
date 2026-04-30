# SyncFlow Baidu Netdisk Cloud Sync Implementation Plan

Date: 2026-04-27

Design: `docs/superpowers/specs/2026-04-27-baidu-netdisk-cloud-sync-design.md`

> Goal: replace the default SyncFlow data plane with bidirectional plaintext synchronization through Baidu Netdisk app-scoped directories.

## API Verification Notes

Implementation must use official Baidu Netdisk Open Platform documentation as the source of truth. The design assumes these platform capabilities and they must be checked against the current app credentials and granted scopes before each phase is considered complete:

- OAuth login and refresh-token flow.
- App-scoped file access under an application directory such as `/apps/<app-name>/`.
- Directory creation and metadata lookup.
- File list APIs that expose path, file id, directory marker, size, md5 or equivalent hash, and server modified time.
- Upload flow, including pre-create, block upload, and create/commit where required by Baidu.
- Download URL or download API for files in the app directory.
- Delete and move/rename APIs.
- Stable error codes for expired token, invalid scope, quota exceeded, rate limit, and missing file.

Primary official entry points to verify manually during coding:

- `https://pan.baidu.com/union/doc/`
- `https://pan.baidu.com/union/document/entrance`

If official API behavior differs from the assumptions, update the design before continuing implementation.

## Phase A: Provider Boundary And Storage Foundations

- [ ] Add a core cloud provider module.
- [ ] Define provider-neutral DTOs for remote entries, upload results, provider errors, and account state.
- [ ] Implement a fake in-memory provider for tests.
- [ ] Add storage tables for cloud accounts, cloud space bindings, remote metadata, and persisted cloud tasks.
- [ ] Add Rust models and query helpers for the new tables.
- [ ] Add token encryption-at-rest plumbing or an explicit secure-storage adapter.

Primary files:

- `syncflow/packages/core/src/lib.rs`
- `syncflow/packages/core/src/storage/schema.rs`
- `syncflow/packages/core/src/storage/models.rs`
- `syncflow/packages/core/src/storage/queries.rs`
- `syncflow/packages/core/src/cloud/mod.rs`
- `syncflow/packages/core/src/cloud/provider.rs`
- `syncflow/packages/core/src/cloud/fake.rs`

Acceptance:

- Core builds with a provider trait independent of Baidu HTTP details.
- SQLite migrations create all cloud sync tables idempotently.
- Tests can create bindings, cache remote metadata, and enqueue/dequeue cloud tasks.
- Tokens are never stored or logged in plaintext.

## Phase B: Baidu OAuth And Account UI

- [ ] Add Baidu provider configuration for client id, redirect URI, and requested scopes.
- [ ] Add OAuth start command returning the authorization URL.
- [ ] Add OAuth callback handling and token exchange.
- [ ] Persist encrypted access and refresh tokens.
- [ ] Implement automatic token refresh before API calls.
- [ ] Add disconnect/reconnect commands.
- [ ] Add UI for connecting a Baidu account and showing account status.

Primary files:

- `syncflow/packages/core/src/cloud/baidu.rs`
- `syncflow/packages/client/src-tauri/src/commands.rs`
- `syncflow/packages/client/src-tauri/src/main.rs`
- `syncflow/packages/client/src/lib/tauriClient.ts`
- `syncflow/packages/client/src/types/`
- `syncflow/packages/client/src/components/`

Acceptance:

- User can start Baidu authorization from the app.
- App can exchange the callback code for tokens.
- Expired access tokens refresh without user action when refresh token is valid.
- UI clearly shows connected, expired, disconnected, and reconnect-required states.

## Phase C: Space Binding Flow

- [ ] Add commands to create a Baidu-bound sync space.
- [ ] Create or verify the remote root under `/apps/SyncFlow/<space-name-or-id>/`.
- [ ] Store a `cloud_space_bindings` row for the local space.
- [ ] Add UI copy explaining plaintext cloud storage.
- [ ] Add a migration/binding action for existing local spaces.
- [ ] Keep legacy P2P spaces intact until explicitly converted or replaced.

Primary files:

- `syncflow/packages/client/src-tauri/src/commands.rs`
- `syncflow/packages/client/src-tauri/src/runtime/manager.rs`
- `syncflow/packages/core/src/storage/queries.rs`
- `syncflow/packages/client/src/App.tsx`
- `syncflow/packages/client/src/components/`

Acceptance:

- New cloud-bound spaces have a local root and a remote Baidu app directory.
- Existing local spaces can be bound without losing local metadata.
- The UI shows the bound remote path and plaintext warning.

## Phase D: Local-To-Baidu Upload Runtime

- [ ] Add cloud task types for upload, delete, mkdir, and metadata refresh.
- [ ] Route local watcher events for cloud-bound spaces into the cloud task queue instead of peer-targeted queue tasks.
- [ ] Implement plaintext file upload to the corresponding remote path.
- [ ] Implement guarded cloud delete for local deletions.
- [ ] Refresh remote metadata after successful writes.
- [ ] Persist failed retryable tasks for restart recovery.
- [ ] Update runtime status with pending upload count and last Baidu error.

Primary files:

- `syncflow/packages/core/src/cloud/tasks.rs`
- `syncflow/packages/core/src/sync/watcher.rs`
- `syncflow/packages/client/src-tauri/src/runtime/space_runtime.rs`
- `syncflow/packages/client/src-tauri/src/runtime/manager.rs`
- `syncflow/packages/client/src-tauri/src/runtime/dto.rs`

Acceptance:

- Creating or modifying a local file uploads a plaintext file to Baidu Netdisk.
- Deleting a local file deletes the remote copy only when the cached remote baseline still matches.
- Network or rate-limit failures leave tasks pending and retry later.
- Watcher events outside the space root are rejected.

## Phase E: Baidu-To-Local Polling Runtime

- [ ] Add a cloud scan loop per active cloud-bound space.
- [ ] Recursively list the remote root or use an official incremental API if available.
- [ ] Compare remote entries with cached remote metadata and local file metadata.
- [ ] Download remote-only and remote-modified files into the local root.
- [ ] Apply guarded local deletes for remote deletions.
- [ ] Suppress watcher echoes for files written by cloud downloads.
- [ ] Update `last_cloud_scan_at`, pending download count, and sync status.

Primary files:

- `syncflow/packages/core/src/cloud/reconcile.rs`
- `syncflow/packages/client/src-tauri/src/runtime/manager.rs`
- `syncflow/packages/client/src-tauri/src/runtime/space_runtime.rs`
- `syncflow/packages/core/src/storage/queries.rs`

Acceptance:

- A file added through Baidu Netdisk appears locally after the next scan.
- A remote edit downloads locally when the local copy has not diverged.
- A remote delete deletes locally only when local metadata still matches the synced baseline.
- Cloud-applied writes do not immediately re-upload as local edits.

## Phase F: Bidirectional Conflict Safety

- [ ] Define baseline comparison using local metadata plus cached remote metadata.
- [ ] Detect same-path local/cloud divergent edits.
- [ ] Preserve local file and download the cloud version as a conflict copy.
- [ ] Insert conflict rows with provider metadata.
- [ ] Reuse text conflict snapshots where possible for compare UI.
- [ ] Add guarded conflict behavior for delete-vs-modify cases.
- [ ] Add user-facing conflict messages for cloud-bound spaces.

Primary files:

- `syncflow/packages/core/src/cloud/reconcile.rs`
- `syncflow/packages/core/src/sync/mod.rs`
- `syncflow/packages/core/src/storage/schema.rs`
- `syncflow/packages/core/src/storage/queries.rs`
- `syncflow/packages/client/src-tauri/src/commands.rs`
- `syncflow/packages/client/src/components/details/DetailsPane.tsx`

Acceptance:

- If local and cloud both modify the same file, neither side is silently overwritten.
- The local folder contains both the original local version and a clearly named Baidu conflict copy.
- The conflict list shows the conflict for the affected space.
- Text conflicts can be inspected using existing conflict detail UI where practical.

## Phase G: UI And Status Polish

- [ ] Replace peer-first status copy for cloud-bound spaces.
- [ ] Show provider, remote path, account status, last scan time, pending upload/download counts, and last error.
- [ ] Add clear privacy copy for plaintext Baidu storage.
- [ ] Add retry/reconnect actions for common provider failures.
- [ ] Keep LAN/P2P controls hidden or explicitly marked legacy/experimental.

Primary files:

- `syncflow/packages/client/src/App.tsx`
- `syncflow/packages/client/src/components/`
- `syncflow/packages/client/src/types/`
- `syncflow/packages/client/src/lib/tauriClient.ts`

Acceptance:

- Users can understand whether a space is syncing with Baidu or waiting on an error.
- Provider errors are actionable instead of raw API messages.
- The UI no longer implies peer connectivity is required for a cloud-bound space.

## Phase H: Validation And Rollout

- [ ] Add fake-provider unit tests for reconciliation scenarios.
- [ ] Add storage migration tests for cloud tables.
- [ ] Add opt-in real Baidu API smoke tests gated by environment variables.
- [ ] Create a manual runbook for two machines using the same Baidu account and cloud space.
- [ ] Run Rust tests and frontend build.
- [ ] Update README and docs to describe Baidu cloud sync as the default mode.

Primary commands:

- `cargo test --workspace --manifest-path syncflow/Cargo.toml`
- `npm --prefix syncflow/packages/client run build`

Acceptance:

- Fake-provider tests cover upload, download, delete, divergent edit, delete conflict, unsafe path rejection, and restart recovery.
- Real API smoke tests can create, upload, list, download, and delete under a test app directory.
- Manual validation passes for two devices and Baidu web UI changes.
- Documentation reflects plaintext cloud storage and OAuth setup requirements.

## Recommended First Coding Slice

Start with Phase A only:

1. Add the provider trait and fake provider.
2. Add storage migrations and query helpers.
3. Add tests for cloud binding, remote metadata, and persisted task queue.

This creates a safe base without depending on Baidu credentials or network access. After Phase A passes, implement Baidu OAuth as the first real provider integration.
