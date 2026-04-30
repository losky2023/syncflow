# SyncFlow - CLAUDE.md

> End-to-end encrypted file sync across devices via WebRTC P2P (LAN only, no server required).

## Quick Commands

```bash
# Run all tests
cargo test --workspace

# Run a single test
cargo test -p syncflow-core test_name

# Build workspace
cargo build --workspace

# Run Tauri client (dev mode with hot reload)
cd packages/client && npx tauri dev

# Format
cargo fmt --all
cargo clippy --workspace
```

## Architecture

```text
packages/
├── core/          # Shared Rust library (crypto, storage, sync, transport)
├── server/        # Deprecated: old signal server (kept for reference)
└── client/        # Tauri 2.0 desktop client (src-tauri/ = Rust backend, src/ = React UI)
```

## Current implementation focus

The desktop client now has two implemented workbench phases:

- **Phase 1**: persistent sync spaces, safe `space_id + relative_path` browsing, three-column workbench, text/image preview, and details pane.
- **Phase 2**: per-space sync runtime management, first-pass indexing, recursive local file watching, device state aggregation, persisted conflict records, and workbench status integration.

The current product is best understood as a **local sync control console** built on top of the existing LAN/WebRTC foundations.

## Key Modules (core)

| Module | File | Purpose |
|--------|------|---------|
| crypto | `crypto/mod.rs` | Argon2id KDF, XChaCha20-Poly1305 AEAD, BLAKE3 hashing, Ed25519 signing |
| storage | `storage/mod.rs` | SQLite via sqlx, sync spaces, file metadata, conflicts, device info |
| sync | `sync/mod.rs` | SyncEngine, file watcher (notify-debouncer-mini), version vectors, sync queue |
| transport | `transport/mod.rs` | WebRTC peer connections, mDNS discovery (mdns-sd), local SDP exchange (axum) |
| auth | `auth/mod.rs` | UserSession with SecretBox root key, device Ed25519 keypairs |
| error | `error.rs` | SyncFlowError enum + Result<T> alias |

## Key Modules (client backend)

| Module | File | Purpose |
|--------|------|---------|
| commands | `packages/client/src-tauri/src/commands.rs` | Tauri command boundary for workbench, runtime, devices, and conflicts |
| runtime manager | `packages/client/src-tauri/src/runtime/manager.rs` | Per-space runtime lifecycle, indexing, watcher startup, device aggregation |
| runtime state | `packages/client/src-tauri/src/runtime/space_runtime.rs` | Runtime status enum and per-space runtime snapshot |
| fs safety | `packages/client/src-tauri/src/fs_safety.rs` | Safe path resolution using `space_id + relative_path` |

## Key Modules (client frontend)

| Module | File | Purpose |
|--------|------|---------|
| workbench app | `packages/client/src/app/Workbench.tsx` | Main three-column workbench and polling-based state orchestration |
| tauri client | `packages/client/src/lib/tauriClient.ts` | Typed wrapper around Tauri commands |
| workbench types | `packages/client/src/types/workbench.ts` | Shared TS types for spaces, runtime state, devices, conflicts, previews |

## Tech Stack

- **Rust**: workspace with resolver = "2"
- **Tauri 2.0**: cross-platform desktop client (React + TypeScript frontend)
- **WebRTC**: P2P data channels via webrtc-rs 0.12
- **mDNS**: LAN device discovery via mdns-sd 0.12 (Bonjour/Avahi compatible)
- **Local HTTP**: SDP offer/answer exchange via lightweight axum server on port 18080
- **SQLite**: local metadata for sync spaces, file metadata, sync state, file versions, conflicts, and devices
- **Encryption**: Argon2id (64 MiB, 3 iters), XChaCha20-Poly1305, Ed25519, BLAKE3
- **HTTP Client**: reqwest 0.12 (for SDP exchange between peers)

## Connection Flow

1. Each device starts an mDNS broadcaster + browser on launch.
2. mDNS discovers peers on the same LAN (type: `_syncflow._tcp.local.`).
3. Discovered peer → HTTP POST to `http://{ip}:18080/sdp/offer` with WebRTC SDP.
4. Peer responds with SDP answer via the same HTTP endpoint.
5. WebRTC Data Channel established — direct P2P, no relay.
6. File sync happens over the encrypted Data Channel.

## Sync Runtime Model

The old single global sync engine flow has been replaced by a per-space runtime manager.

- `start_sync(password, device_name)` initializes the session context and root key.
- `start_space_sync(space_id)` starts indexing + watcher lifecycle for one sync space.
- `stop_space_sync(space_id)` stops only that space runtime.
- `stop_sync()` stops all runtimes and clears the session context.
- `get_sync_status(space_id)` and `get_all_sync_statuses()` expose runtime state to the UI.

Runtime states currently include:

- `stopped`
- `starting`
- `indexing`
- `watching`
- `syncing`
- `error`

## Storage Notes

Important schema details:

- `synced_spaces` persists registered local roots.
- `file_metadata` is keyed by `(space_id, relative_path)`.
- `sync_conflicts` persists detected conflicts for UI display.
- `devices` stores known devices and last-seen data.

This means sync metadata identity is no longer based on raw absolute file paths.

## Workbench Behavior

After login, the app opens a three-column workbench:

- **Top bar**: device/session/runtime summary.
- **Left sidebar**: sync spaces, per-space start/stop controls, file counts, conflict counts, and file tree.
- **Center pane**: welcome, directory, text, image, or fallback preview state.
- **Right pane**: file details plus read-only conflict list for the selected space.

The frontend currently uses polling to refresh runtime state, device state, and conflicts.

## Conflict Handling

Version-vector conflicts are now persisted instead of being only logged.

The UI can display for each conflict:

- `relativePath`
- `remoteDevice`
- `localVersion`
- `remoteVersion`
- `detectedAt`

Conflict resolution actions are not implemented yet; current support is visibility only.

## Development Workflow

1. **TDD**: write tests first, then implement
2. **Code review**: use `Agent` tool with code-reviewer after each task
3. **Commits**: conventional commits format (`feat:`, `fix:`, `chore:`)
4. **Security**: no hardcoded secrets, validate all inputs, use `secrecy::SecretBox` for sensitive data

## Coding Standards

- Immutable data patterns (create new, don't mutate)
- Files < 800 lines, functions < 50 lines
- Error handling at every level with user-friendly messages
- Min test coverage: 80%
- Follow existing patterns in the codebase
