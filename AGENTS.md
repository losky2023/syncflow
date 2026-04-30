# Repository Guidelines

## Project Structure & Module Organization

The main product is `syncflow/`, a Rust workspace for a Tauri desktop LAN sync app. `syncflow/packages/core/` contains shared Rust modules for `crypto`, `storage`, `sync`, `transport`, and `auth`. `syncflow/packages/client/` contains the desktop app: React/TypeScript UI in `src/`, Tauri backend code in `src-tauri/src/`, and app assets under `src-tauri/icons/`. `syncflow/packages/server/` is a deprecated signal server kept for reference. Design notes, plans, and runbooks live in `docs/superpowers/`.

## Build, Test, and Development Commands

- `cargo check --workspace --manifest-path syncflow/Cargo.toml`: fast Rust compile check.
- `cargo test --workspace --manifest-path syncflow/Cargo.toml`: run all Rust unit and doc tests.
- `cargo fmt --all --manifest-path syncflow/Cargo.toml`: format Rust code.
- `cargo clippy --workspace --manifest-path syncflow/Cargo.toml`: run Rust lints.
- `npm --prefix syncflow/packages/client install`: install frontend dependencies.
- `npm --prefix syncflow/packages/client run build`: type-check and build the React UI.
- `cd syncflow/packages/client && npx tauri dev`: run the desktop app in development mode.

## Coding Style & Naming Conventions

Use Rust 2021 idioms, `rustfmt`, and explicit `Result`-based error handling. Keep storage concerns in `storage`, peer discovery/WebRTC in `transport`, sync runtime logic in `sync` or `src-tauri/src/runtime`, and Tauri IPC in `commands.rs`. React components use PascalCase filenames, shared TypeScript types belong in `src/types/`, and Tauri invoke wrappers belong in `src/lib/tauriClient.ts`.

## Testing Guidelines

Add or update Rust tests for storage migrations, filesystem safety, sync queue behavior, conflict handling, and transport orchestration. Prefer descriptive names such as `test_save_and_get_local_account`. Run `cargo test --workspace --manifest-path syncflow/Cargo.toml` before submitting backend changes and `npm --prefix syncflow/packages/client run build` when touching UI or TypeScript.

## Commit & Pull Request Guidelines

Follow the existing conventional commit style: `feat:`, `fix:`, `docs:`, or `chore:`. Recent examples include `feat: add persisted workbench file browser` and `fix: pre-create SQLite database file before connecting on Windows`. PRs should include a concise summary, test results, linked issue or spec when relevant, and screenshots for UI changes.

## Security & Configuration Tips

Do not hardcode secrets, account material, or pairing keys. Preserve `space_id + relative_path` path validation and avoid direct filesystem access from UI inputs. Treat `username` as a display name only; stable local identity comes from persisted account material. Keep `sync_key` internal until explicit share/join flows replace manual pairing.
