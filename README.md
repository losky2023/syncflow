# SyncFlow

End-to-end encrypted file synchronization across devices via WebRTC P2P.

## Features

- **End-to-end encryption** — Files are encrypted on-device before sync. The server never sees plaintext.
- **P2P via WebRTC** — Direct device-to-device transfer over LAN or internet.
- **Cross-platform** — Windows, macOS (Linux planned).
- **Conflict detection** — Version vectors detect concurrent modifications with manual resolution.
- **Real-time file watching** — Debounced file system events trigger automatic sync.
- **Account-based E2E key derivation** — Password + Argon2id → root key → per-file encryption keys.

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                    Tauri Client                     │
│  ┌──────────┐  ┌──────────┐  ┌───────────────────┐ │
│  │ React UI │  │ Commands │  │    Core Library   │ │
│  │          │◄─┤  (IPC)   │◄─┤ ┌────┐ ┌───────┐ │ │
│  │          │  │          │  │ │Crypto│ │Storage│ │ │
│  └──────────┘  └──────────┘  │ │Sync │ │Transport│ │
│                               │ └────┘ └───────┘ │
│                               └───────────────────┘ │
└──────────────────┬──────────────────────────────────┘
                   │ WebRTC data channel
                   │ + WebSocket signaling
┌──────────────────▼──────────────────────────────────┐
│                  Signal Server                      │
│  ┌────────┐  ┌──────────┐  ┌─────────────────────┐ │
│  │  Auth  │  │  Signal  │  │  Device Registry    │ │
│  │ (JWT)  │  │ (SDP fwd)│  │  (WebSocket pool)   │ │
│  └────────┘  └──────────┘  └─────────────────────┘ │
│  SQLite: users, server_devices                      │
└─────────────────────────────────────────────────────┘
```

## Tech Stack

| Layer | Technology |
|-------|------------|
| Desktop | Tauri 2.0 (Rust + React/TypeScript) |
| Server | Axum + tokio |
| P2P | WebRTC (webrtc-rs) |
| Local DB | SQLite (sqlx) |
| Key Derivation | Argon2id (64 MiB, 3 iterations) |
| Encryption | XChaCha20-Poly1305 AEAD |
| Signing | Ed25519 |
| Hashing | BLAKE3 |
| Auth | JWT (30-day expiry) |

## Project Structure

```
syncflow/
├── Cargo.toml              # Workspace root
├── CLAUDE.md               # Development guide for AI assistants
├── packages/
│   ├── core/               # Shared Rust library
│   │   └── src/
│   │       ├── crypto/     # Encryption, hashing, key derivation
│   │       ├── storage/    # SQLite models and queries
│   │       ├── sync/       # SyncEngine, file watcher, version vectors, queue
│   │       ├── transport/  # WebRTC peer connections, signal client
│   │       └── auth/       # Session management, device keypairs
│   ├── server/             # Signal server (axum)
│   │   └── src/
│   │       ├── auth.rs     # Register, login, JWT
│   │       ├── signal.rs   # WebSocket handler, device registry
│   │       ├── device.rs   # Device registration
│   │       └── stun.rs     # STUN config endpoint
│   └── client/             # Tauri desktop app
│       ├── src/            # React frontend
│       └── src-tauri/      # Rust backend commands
└── target/                 # Build output (gitignored)
```

## Getting Started

### Prerequisites

- Rust 1.75+
- Node.js 18+
- SQLite

### Build

```bash
# Install workspace dependencies
cargo fetch

# Run all tests
cargo test --workspace

# Build workspace
cargo build --workspace
```

### Run Signal Server

```bash
cd packages/server
cargo run
# Server starts at http://localhost:3000
```

### Run Desktop Client

```bash
cd packages/client/src-tauri
cargo tauri dev
```

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| POST | `/auth/register` | Create account |
| POST | `/auth/login` | Login with JWT |
| POST | `/device/register` | Register device (E2E keypair) |
| GET | `/devices?user_id=` | List user's devices |
| GET | `/stun/config` | Get STUN/TURN servers |
| WS | `/ws/signal?token=` | WebSocket signaling channel |

## TODO (Phase 6)

- [ ] WebSocket backup channel (P2P failure fallback)
- [ ] Android support
- [ ] Incremental sync optimization (chunked transfer)
- [ ] Large file resume support

## License

MIT
