# SyncFlow

End-to-end encrypted file synchronization across devices via WebRTC P2P.

## Features

- **End-to-end encryption** — Files are encrypted on-device before sync. The server never sees plaintext.
- **P2P via WebRTC** — Direct device-to-device transfer over LAN via mDNS discovery and local SDP exchange.
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
└─────────────────────────────────────────────────────┘
         │ mDNS discovery (LAN)
         │ local HTTP SDP exchange (port 18080)
         ▼
┌─────────────────────────────────────────────────────┐
│              Direct P2P via WebRTC                  │
│  Devices on the same LAN discover each other via    │
│  mDNS, exchange SDP offers over local HTTP, then   │
│  communicate directly via WebRTC data channels.     │
│  No central server required.                        │
└─────────────────────────────────────────────────────┘
```

## Tech Stack

| Layer | Technology |
|-------|------------|
| Desktop | Tauri 2.0 (Rust + React/TypeScript) |
| Local Server | Axum + tokio (SDP exchange, port 18080) |
| P2P | WebRTC (webrtc-rs) + mDNS (mdns-sd) |
| Local DB | SQLite (sqlx) |
| Key Derivation | Argon2id (64 MiB, 3 iterations) |
| Encryption | XChaCha20-Poly1305 AEAD |
| Signing | Ed25519 |
| Hashing | BLAKE3 |

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
│   │       ├── transport/  # WebRTC peer connections, mDNS discovery, local SDP exchange
│   │       └── auth/       # Session management, device keypairs
│   ├── server/             # Local SDP exchange server (axum, port 18080)
│   │   └── src/
│   │       ├── sdp.rs      # SDP offer/answer HTTP endpoints
│   │       └── mdns.rs     # mDNS service discovery
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

### LAN Discovery

```
Devices on the same LAN will automatically discover each other via mDNS.
```

### Run Desktop Client

```bash
cd packages/client/src-tauri
cargo tauri dev
```

## TODO (Phase 6)

- [ ] LAN relay mode (for devices on different subnets)
- [ ] Android support
- [ ] Incremental sync optimization (chunked transfer)
- [ ] Large file resume support

## License

MIT
