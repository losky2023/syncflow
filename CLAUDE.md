# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repo layout

The actual product code lives under `syncflow/`.

- `syncflow/packages/core` — shared Rust library for auth, crypto, storage, cloud sync, local sync/runtime foundations, and LAN transport.
- `syncflow/packages/client` — Tauri desktop client (`src/` React frontend, `src-tauri/` Rust backend).
- `docs/superpowers/specs/` and `docs/superpowers/plans/` — current feature design and implementation plans; the Baidu Netdisk cloud-sync docs describe the intended product direction.

Run Rust commands from `syncflow/` unless you use `--manifest-path syncflow/Cargo.toml` from the repository root.

## Common commands

```bash
# Rust workspace
cargo test --workspace
cargo test -p syncflow-core test_name
cargo build --workspace
cargo fmt --all
cargo clippy --workspace

# Frontend / Tauri
npm --prefix packages/client run build
cd packages/client && npx tauri dev

# Alternative from repo root
cargo test --workspace --manifest-path syncflow/Cargo.toml
cargo build --workspace --manifest-path syncflow/Cargo.toml
cargo fmt --all --manifest-path syncflow/Cargo.toml
cargo clippy --workspace --manifest-path syncflow/Cargo.toml
npm --prefix syncflow/packages/client run build
```

## High-level architecture

SyncFlow is a Tauri desktop app whose backend owns the sync runtime and local data model, while the React workbench is a thin UI over typed Tauri commands.

### Runtime layers

1. `packages/client/src/App.tsx` mounts the workbench UI.
2. `packages/client/src/app/Workbench.tsx` orchestrates the main desktop workflow: sync spaces, file tree, previews, conflict handling, Baidu account state, and runtime status.
3. `packages/client/src/lib/tauriClient.ts` is the frontend command boundary. Prefer adding typed wrappers here instead of calling `invoke(...)` directly from components.
4. `packages/client/src-tauri/src/commands.rs` exposes the Tauri IPC surface for login, synced spaces, file operations, conflicts, cloud sync actions, invites, and runtime control.
5. `packages/client/src-tauri/src/runtime/` manages per-space runtime state and background orchestration.
6. `packages/core/src/*` contains the reusable backend logic for storage, sync logic, cloud providers, transport, crypto, and auth.

### Current product direction

The repository originally centered on LAN peer-to-peer sync over mDNS + WebRTC, but the active product direction is Baidu Netdisk-backed cloud sync.

- LAN discovery and WebRTC transport still exist in `packages/core/src/transport/` and are still started by the Tauri backend.
- The newer cloud path lives under `packages/core/src/cloud/` plus cloud-related storage tables and runtime status fields.
- The current desktop workbench already exposes Baidu OAuth/configuration, cloud-bound sync spaces, cloud task diagnostics, and cloud conflict handling.

When reading the codebase, treat it as a hybrid system in migration: cloud sync is being layered onto an existing LAN/P2P foundation rather than replacing it all at once.

### Tauri backend startup model

`packages/client/src-tauri/src/main.rs` boots the desktop backend by:

- creating the app data directory and SQLite database,
- initializing storage schema,
- constructing the shared `TauriState`,
- starting discovery + SDP server + transport event handling,
- registering the Tauri commands used by the frontend.

This means app startup, storage initialization, transport lifecycle, and runtime-manager wiring are centralized in `main.rs`.

### Storage model

SQLite schema is defined in `packages/core/src/storage/schema.rs` and query/model code lives in `packages/core/src/storage/`.

Important persisted concepts:

- `synced_spaces` — local sync roots.
- `file_metadata` — local indexed files, keyed by `(space_id, relative_path)` rather than absolute path.
- `sync_conflicts` and `sync_conflict_snapshots` — persisted conflict records and compareable text snapshots.
- `cloud_api_configs` and `cloud_accounts` — Baidu OAuth/app configuration and encrypted token storage.
- `cloud_space_bindings` — mapping from a local sync space to a cloud provider remote root.
- `remote_file_metadata` and `cloud_sync_tasks` — cached remote state and queued cloud work.
- `devices` — known device state used by the legacy/hybrid transport model.

The important safety boundary is `space_id + relative_path`. File access should resolve through the registered sync root instead of passing raw absolute paths through the UI boundary.

### Sync/runtime model

The runtime is per-space, not one global sync engine.

Key runtime files:

- `packages/client/src-tauri/src/runtime/manager.rs`
- `packages/client/src-tauri/src/runtime/space_runtime.rs`
- `packages/client/src-tauri/src/runtime/dto.rs`

Core ideas:

- session initialization and per-space runtime control are separate,
- each space can be started or stopped independently,
- runtime status is surfaced back to the workbench as DTOs,
- status now includes both local/runtime health and cloud-sync-related counts/errors.

### Cloud provider boundary

`packages/core/src/cloud/` defines the provider-oriented cloud sync layer.

- `provider.rs` holds the abstraction used by the sync/runtime logic.
- `baidu.rs` implements the Baidu Netdisk integration.
- `fake.rs` provides an in-memory provider used by tests.

Keep provider-specific HTTP behavior inside the cloud module instead of leaking it into the Tauri commands or frontend.

### Frontend workbench model

The desktop UI is a workbench, not a wizard flow.

`packages/client/src/app/Workbench.tsx` coordinates:

- sync space selection and creation,
- lazy file-tree browsing,
- file preview and text editing,
- sync runtime status and diagnostics,
- Baidu account connection/configuration,
- conflict inspection and resolution actions.

Most visible user behavior is driven by polling commands from the Tauri backend rather than a client-side state machine.

## Existing guidance to keep in mind

The checked-in docs and current codebase indicate these repo-specific expectations:

- Prefer typed wrappers in `packages/client/src/lib/tauriClient.ts` for frontend/backend integration.
- Preserve the safe filesystem boundary based on registered sync spaces and relative paths.
- Treat `docs/superpowers/specs/2026-04-27-baidu-netdisk-cloud-sync-design.md` as the high-level product-direction document when cloud-sync behavior seems inconsistent with older LAN/P2P assumptions.
