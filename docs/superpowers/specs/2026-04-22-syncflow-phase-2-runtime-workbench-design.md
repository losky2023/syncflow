# SyncFlow Phase 2 Sync Runtime Workbench Design

Date: 2026-04-22

## Summary

Phase 2 moves SyncFlow from a read-only local file workbench into a local sync control console. The workbench now exposes real per-space sync runtime state, device discovery state, and persisted conflict state while preserving the Phase 1 file browser, preview, and details workflow.

The implementation deliberately keeps the scope narrow: it establishes the local runtime loop and observability layer first. It does not yet implement a full merge UI, remote version acceptance, large-file transfer optimization, or advanced background scheduling.

## Goals

- Manage sync runtime per sync space instead of using one global `SyncEngine`.
- Keep session initialization separate from starting a space runtime.
- Index a sync space when it starts and persist metadata by `space_id + relative_path`.
- Watch started spaces for local file changes.
- Expose runtime status, file count, and conflict count to the workbench.
- Aggregate known, discovered, and connected devices into a single device state list.
- Persist version-vector conflicts and show them in the UI.
- Preserve the Phase 1 safe path boundary and file browser behavior.

## Non-goals

- Rich conflict resolution or diff/merge UI.
- Accept-remote conflict resolution that requires remote content caching.
- Full automatic cross-device transfer orchestration beyond the existing queue and transport foundations.
- Replacing polling with a frontend event bus.
- Reworking the workbench layout.

## Backend Runtime Architecture

The Tauri backend now owns a runtime subsystem under:

- `packages/client/src-tauri/src/runtime/mod.rs`
- `packages/client/src-tauri/src/runtime/manager.rs`
- `packages/client/src-tauri/src/runtime/space_runtime.rs`
- `packages/client/src-tauri/src/runtime/dto.rs`

`TauriState` holds:

- shared `StorageEngine`,
- shared `TransportLayer`,
- `SessionSyncContext`,
- `SyncRuntimeManager`,
- local `device_id` and `device_name`.

`SessionSyncContext` stores the session root key after `start_sync(password, device_name)`. This keeps compatibility with the old global start button while making actual sync work space-scoped.

`SyncRuntimeManager` stores an in-memory map:

```text
SpaceId -> SpaceRuntime
```

Each `SpaceRuntime` tracks:

- `space_id`
- root path
- runtime status
- indexed file count
- pending count
- conflict count
- last indexed time
- last error
- watcher task handle

Runtime statuses are:

- `stopped`
- `starting`
- `indexing`
- `watching`
- `syncing`
- `error`

## Runtime Flow

### Session initialization

`start_sync(password, device_name)` derives the root key and stores it in `SessionSyncContext`. It does not start every registered space automatically.

`stop_sync()` stops all runtimes through `SyncRuntimeManager::stop_all()` and clears the session context.

### Per-space start

`start_space_sync(space_id)` performs this flow:

1. Reads the initialized root key from `SessionSyncContext`.
2. Loads the registered sync space from storage.
3. Creates or updates the in-memory `SpaceRuntime`.
4. Marks the runtime as `starting`, then `indexing`.
5. Canonicalizes the sync-space root path.
6. Creates a `SyncEngine` for that space runtime.
7. Scans the root recursively and stores file metadata.
8. Updates `synced_spaces.last_scanned_at`.
9. Starts a recursive watcher with `notify-debouncer-mini`.
10. Stores the watcher task handle and marks the runtime as `watching`.

Repeated starts are idempotent while a runtime is already active.

### Per-space stop

`stop_space_sync(space_id)` aborts the watcher task for that space, clears the last error, and marks the runtime as `stopped`.

## Storage Changes

`file_metadata` is now keyed by `(space_id, relative_path)` instead of a raw path string:

```sql
CREATE TABLE IF NOT EXISTS file_metadata (
    space_id TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    hash TEXT NOT NULL,
    size INTEGER NOT NULL,
    modified_at TEXT NOT NULL,
    version_vector TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (space_id, relative_path)
);
```

A new `sync_conflicts` table persists conflicts:

```sql
CREATE TABLE IF NOT EXISTS sync_conflicts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    space_id TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    local_version TEXT NOT NULL,
    remote_version TEXT NOT NULL,
    remote_device_id TEXT NOT NULL,
    detected_at TEXT NOT NULL
);
```

New storage queries include:

- `count_files_for_space(space_id)`
- `update_space_last_scanned_at(space_id, scanned_at)`
- `save_conflict(conflict)`
- `get_conflicts_for_space(space_id)`
- `get_all_conflicts()`
- `count_conflicts_for_space(space_id)`

## Sync Engine Changes

`SyncEngine` now supports space-aware file operations:

- `index_local_file(space_id, relative_path, resolved_path)`
- `handle_space_file_event(space_id, relative_path, resolved_path, event)`
- `receive_space_file(from, space_root, expected_space_id, data)`

File metadata is saved using `space_id + relative_path`. Incoming remote writes use a safe join helper that rejects absolute paths and parent traversal before writing under a known sync-space root.

When an incoming version vector conflicts with a local version vector, `SyncEngine` persists a `SyncConflict` and returns without overwriting the local file.

## Device State

`get_device_info()` now returns aggregated `DeviceStateDto` values from:

- known devices stored in SQLite,
- devices currently discovered over mDNS,
- connected WebRTC peers.

The manager filters out the local device and normalizes state to:

- `connected`
- `discovered`
- `offline`

Discovered devices are saved back into the known-device store with updated last-seen timestamps.

## Tauri Commands

Phase 2 adds or updates these command semantics:

- `start_sync(password, device_name)` initializes session context only.
- `stop_sync()` stops all space runtimes and clears session context.
- `start_space_sync(space_id)` starts indexing and watching one space.
- `stop_space_sync(space_id)` stops one space.
- `get_sync_status(space_id)` returns one runtime snapshot.
- `get_all_sync_statuses()` returns all known runtime snapshots.
- `get_device_info()` returns aggregated device state.
- `get_conflicts(space_id?)` returns persisted conflicts.

The frontend accesses these through `packages/client/src/lib/tauriClient.ts`, which maps command payloads to camelCase TypeScript types.

## Frontend Integration

The existing workbench structure remains intact.

`Workbench.tsx` now polls:

- all runtime statuses,
- device state,
- selected-space conflicts.

`SpaceList.tsx` shows for each space:

- runtime status badge,
- file count,
- conflict count,
- last runtime error,
- start/stop sync button.

`DetailsPane.tsx` shows a read-only conflict section for the selected space, including:

- relative path,
- remote device,
- detected time,
- local version vector,
- remote version vector.

The top status bar shows selected runtime state plus connected, discovered, and conflict counts.

## Safety Model

The Phase 1 command-boundary filesystem safety model remains required:

- parse and validate `space_id`,
- load the registered space,
- reject absolute and parent-traversal relative paths,
- canonicalize existing targets,
- verify targets remain under the canonical space root.

Phase 2 extends the same idea into sync metadata and remote writes: sync identity is `space_id + relative_path`, not arbitrary absolute paths.

## Verification

The implementation was verified with:

```bash
cargo test --workspace --manifest-path "syncflow/Cargo.toml"
npm --prefix "syncflow/packages/client" run build
cargo fmt --all --manifest-path "syncflow/Cargo.toml"
cargo clippy --workspace --manifest-path "syncflow/Cargo.toml"
```

Clippy warnings were cleaned after the main implementation.

Manual validation should include:

1. Login and initialize a session.
2. Add or select a sync space.
3. Start sync for that space.
4. Confirm status transitions to `watching`.
5. Confirm indexed file counts appear in the space list.
6. Modify local files and confirm metadata/status refresh.
7. Confirm device state appears in the top/bottom workbench areas.
8. Create or simulate a version-vector conflict and confirm it appears in the details pane.
9. Stop a single space and confirm other runtime state remains intact.
