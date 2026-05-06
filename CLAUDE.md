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

```
packages/
├── core/          # Shared Rust library (crypto, storage, sync, transport)
├── server/        # Deprecated: old signal server (kept for reference)
└── client/        # Tauri 2.0 desktop client (src-tauri/ = Rust backend, src/ = React UI)
```

## Key Modules (core)

| Module | File | Purpose |
|--------|------|---------|
| crypto | `crypto/mod.rs` | Argon2id KDF, XChaCha20-Poly1305 AEAD, BLAKE3 hashing, Ed25519 signing |
| storage | `storage/mod.rs` | SQLite via sqlx, models: FileMetadata, SyncState, FileVersion, DeviceInfo |
| sync | `sync/mod.rs` | SyncEngine, file watcher (notify-debouncer-mini), version vectors, sync queue |
| transport | `transport/mod.rs` | WebRTC peer connections, mDNS discovery (mdns-sd), local SDP exchange (axum) |
| auth | `auth/mod.rs` | UserSession with SecretBox root key, device Ed25519 keypairs |
| error | `error.rs` | SyncFlowError enum + Result<T> alias |

## Tech Stack

- **Rust**: workspace with resolver = "2"
- **Tauri 2.0**: cross-platform desktop client (React + TypeScript frontend)
- **WebRTC**: P2P data channels via webrtc-rs 0.12
- **mDNS**: LAN device discovery via mdns-sd 0.12 (Bonjour/Avahi compatible)
- **Local HTTP**: SDP offer/answer exchange via lightweight axum server on port 18080
- **SQLite**: local metadata (sqlx, 4 tables: file_metadata, sync_state, file_versions, devices)
- **Encryption**: Argon2id (64 MiB, 3 iters), XChaCha20-Poly1305, Ed25519, BLAKE3
- **HTTP Client**: reqwest 0.12 (for SDP exchange between peers)

## Connection Flow

1. Each device starts an mDNS broadcaster + browser on launch
2. mDNS discovers peers on the same LAN (type: `_syncflow._tcp.local.`)
3. Discovered peer → HTTP POST to `http://{ip}:18080/sdp/offer` with WebRTC SDP
4. Peer responds with SDP answer via the same HTTP endpoint
5. WebRTC Data Channel established — direct P2P, no relay
6. File sync happens over the encrypted Data Channel

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
