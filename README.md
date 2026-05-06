# SyncFlow

End-to-end encrypted file synchronization across devices via WebRTC P2P. No server required.

## Features

- **End-to-end encryption** — Files are encrypted on-device before sync. No server ever sees plaintext.
- **Zero server cost** — Pure P2P via mDNS LAN discovery and local HTTP SDP exchange.
- **Cross-platform** — Windows, macOS, iOS (via Tauri 2.0).
- **Conflict detection** — Version vectors detect concurrent modifications with manual resolution.
- **Real-time file watching** — Debounced file system events trigger automatic sync.
- **Password-based auth** — Password + Argon2id → root key → per-file encryption keys.

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
| P2P | WebRTC (webrtc-rs) |
| Discovery | mDNS (mdns-sd, Bonjour/Avahi compatible) |
| SDP Exchange | Axum HTTP server (port 18080, embedded in client) |
| Local DB | SQLite (sqlx) |
| Key Derivation | Argon2id (64 MiB, 3 iterations) |
| Encryption | XChaCha20-Poly1305 AEAD |
| Signing | Ed25519 |
| Hashing | BLAKE3 |

## Project Structure

```
syncflow/
├── Cargo.toml              # Workspace root
├── packages/
│   ├── core/               # Shared Rust library
│   │   └── src/
│   │       ├── crypto/     # Encryption, hashing, key derivation
│   │       ├── storage/    # SQLite models and queries
│   │       ├── sync/       # SyncEngine, file watcher, version vectors, queue
│   │       ├── transport/  # mDNS discovery, local SDP exchange, WebRTC peers
│   │       └── auth/       # Session management, device keypairs
│   ├── server/             # Deprecated: old signal server (kept for reference)
│   └── client/             # Tauri desktop app
│       ├── src/            # React frontend (App.tsx, main.tsx)
│       └── src-tauri/      # Rust backend (main.rs, commands.rs)
└── target/                 # Build output (gitignored)
```

## Getting Started

### Prerequisites

- Rust 1.75+
- Node.js 18+

### Build

```bash
# Install workspace dependencies
cargo fetch

# Run all tests
cargo test --workspace

# Build workspace
cargo build --workspace
```

### Run Desktop Client (dev mode)

```bash
cd packages/client
npm install
npx tauri dev
```

This starts:
1. Vite dev server on `http://localhost:1420` (React frontend)
2. Tauri app window with Rust backend
3. mDNS discovery broadcasts and listens for LAN peers
4. Local HTTP server on port 18080 for SDP exchange

### How It Works

1. Launch the app on two devices on the same WiFi
2. Devices auto-discover each other via mDNS
3. SDP offer/answer exchanged over local HTTP (port 18080)
4. WebRTC Data Channel connects directly between devices
5. Files are encrypted before transfer — no server involved

## TODO (Phase 6)

- [ ] Incremental sync optimization (chunked transfer)
- [ ] Large file resume support
- [ ] Manual conflict resolution UI
- [ ] Android support

## License

MIT
