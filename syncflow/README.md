# SyncFlow

> End-to-end encrypted file sync across devices via WebRTC P2P on LAN, with a Tauri workbench for browsing local sync spaces and observing sync runtime state.

## Current status

SyncFlow is currently focused on the desktop client and local LAN sync loop:

- A Tauri 2.0 desktop app with React/TypeScript UI.
- Persistent sync spaces stored in SQLite.
- A light three-column workbench for space selection, lazy file browsing, preview, details, device status, and conflict visibility.
- Per-space sync runtime management: initialize a session once, then start or stop sync independently for each space.
- Local indexing and recursive file watching for started spaces.
- mDNS discovery and WebRTC transport foundations for LAN devices.

## Workspace layout

```text
packages/
├── core/          # Shared Rust library: crypto, storage, sync, transport
└── client/        # Tauri desktop client: Rust backend + React UI
```

## Quick commands

Run from `syncflow/` unless noted otherwise.

```bash
# Run Rust tests
cargo test --workspace

# Format Rust code
cargo fmt --all

# Run clippy
cargo clippy --workspace

# Build the Rust workspace
cargo build --workspace

# Build the React frontend
npm --prefix "packages/client" run build

# Run the desktop app in dev mode
cd packages/client && npx tauri dev
```

From the repository root, use `--manifest-path` for Rust commands:

```bash
cargo test --workspace --manifest-path "syncflow/Cargo.toml"
cargo fmt --all --manifest-path "syncflow/Cargo.toml"
cargo clippy --workspace --manifest-path "syncflow/Cargo.toml"
```

## Desktop workbench

After login, the client opens the workbench instead of a modal-style folder manager.

The workbench contains:

- **Top status bar**: local device, session initialization, selected space runtime state, connected/discovered counts, and conflict count.
- **Left sidebar**: persistent sync spaces, per-space start/stop controls, runtime counts, and lazy file tree.
- **Center preview**: welcome state, directory state, text preview, image preview, and fallback open-file card.
- **Right details pane**: selected file/folder metadata and read-only conflict list for the selected space.

File browsing, preview, details, and open-file commands use `space_id + relative_path`. The frontend does not pass arbitrary absolute file paths after a space has been registered.

## Sync runtime model

The Tauri backend uses a per-space runtime manager instead of a single global `SyncEngine`.

Key files:

- `packages/client/src-tauri/src/runtime/manager.rs`
- `packages/client/src-tauri/src/runtime/space_runtime.rs`
- `packages/client/src-tauri/src/runtime/dto.rs`
- `packages/client/src-tauri/src/commands.rs`
- `packages/core/src/sync/mod.rs`

Runtime concepts:

- `start_sync(password, device_name)` initializes the session root key and device context.
- `stop_sync()` stops all space runtimes and clears the session context.
- `start_space_sync(space_id)` starts one space:
  1. reads the registered sync space,
  2. canonicalizes the root path,
  3. indexes existing files into `file_metadata`,
  4. updates `last_scanned_at`,
  5. starts a recursive watcher,
  6. reports `watching` when ready.
- `stop_space_sync(space_id)` stops only that space's watcher.
- `get_sync_status(space_id)` and `get_all_sync_statuses()` expose runtime snapshots to the UI.

Runtime statuses are:

- `stopped`
- `starting`
- `indexing`
- `watching`
- `syncing`
- `error`

The current implementation prioritizes a verifiable local runtime loop: indexing, watching, status visibility, and conflict visibility. More advanced transfer scheduling and merge workflows can be layered on top.

## Storage model

SQLite is initialized by `packages/core/src/storage/schema.rs`.

Important tables:

- `synced_spaces`: registered local sync roots.
- `file_metadata`: indexed file metadata keyed by `(space_id, relative_path)`.
- `sync_conflicts`: persisted version-vector conflicts for UI display.
- `devices`: known LAN devices and last-seen timestamps.
- `sync_state` and `file_versions`: existing sync bookkeeping foundations.

`file_metadata` no longer uses an absolute path as the primary identity. The safe identity for sync metadata is now:

```text
(space_id, relative_path)
```

This matches the workbench safety boundary and prevents remote or frontend inputs from directly selecting arbitrary absolute paths.

## Device state

Device status shown in the workbench is aggregated from:

- known devices stored in SQLite,
- currently discovered mDNS devices,
- currently connected WebRTC peers.

`get_device_info()` returns unified device states:

- `connected`
- `discovered`
- `offline`

The local device is filtered out of the device list.

## Conflict handling

Conflict detection uses version vectors in `packages/core/src/sync/mod.rs`.

When an incoming file version conflicts with the local version, SyncFlow now persists a record in `sync_conflicts` instead of only logging a warning. The workbench reads conflicts through `get_conflicts(space_id?)` and displays:

- relative path,
- local version vector,
- remote version vector,
- remote device,
- detected time.

Conflict display is read-only for now. Diff, merge, accept-local, and accept-remote workflows are intentionally left for a later phase.

## Tauri command boundary

Frontend code should call `packages/client/src/lib/tauriClient.ts`, not `invoke(...)` directly.

Core workbench and sync commands include:

- `pick_folder()`
- `get_synced_folders()`
- `add_synced_folder(path)`
- `remove_synced_folder(space_id)`
- `get_tree_children(space_id, parent_relative_path?)`
- `get_file_details(space_id, relative_path)`
- `preview_file_text(space_id, relative_path, max_bytes?)`
- `preview_file_image(space_id, relative_path, max_bytes?)`
- `open_file(space_id, relative_path)`
- `start_sync(password, device_name)`
- `stop_sync()`
- `start_space_sync(space_id)`
- `stop_space_sync(space_id)`
- `get_sync_status(space_id)`
- `get_all_sync_statuses()`
- `get_device_info()`
- `get_discovered_devices()`
- `get_conflicts(space_id?)`

## Filesystem safety

The Tauri backend validates file access at the command boundary:

1. Parse and validate `space_id`.
2. Load the registered sync space.
3. Resolve relative paths against the sync-space root.
4. Reject absolute paths and parent traversal.
5. Canonicalize existing targets.
6. Verify the final path remains inside the canonical sync-space root.

Remote file writes also use safe relative-path joining and reject paths that escape the target sync space.

## Validation

The phase 2 runtime and UI integration were verified with:

```bash
cargo test --workspace --manifest-path "syncflow/Cargo.toml"
npm --prefix "syncflow/packages/client" run build
cargo fmt --all --manifest-path "syncflow/Cargo.toml"
cargo clippy --workspace --manifest-path "syncflow/Cargo.toml"
```

Manual checks should cover:

1. Login opens the workbench.
2. Add or select a sync space.
3. Start that space's sync runtime.
4. Confirm status moves through `starting` / `indexing` to `watching`.
5. Modify files under the root and confirm metadata/status refresh.
6. Check device discovery/status counts.
7. Confirm conflict records appear in the details pane when conflicts are created.
