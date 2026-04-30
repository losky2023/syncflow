# SyncFlow End-to-End Sync Closure Plan

Date: 2026-04-22

> Goal: move SyncFlow from a local sync workbench into a verifiable LAN P2P sync product for the desktop client.

## Current Position

The repository has already completed the local control plane:

- Tauri desktop client with React workbench.
- Persistent sync spaces in SQLite.
- Safe file browsing based on `space_id + relative_path`.
- Per-space runtime state, indexing, watcher lifecycle, and conflict visibility.
- Transport foundations for mDNS discovery, local SDP exchange, and WebRTC data channels.

What is still missing is the actual data-plane closure between two devices. Today the codebase looks like a local sync console built on top of partially integrated transport code, not a fully closed end-to-end sync system.

## Confirmed Gaps

1. **Network bootstrap is not wired in the client**
   `packages/client/src-tauri/src/main.rs` creates `TransportLayer` only. It does not start `DiscoveryService` or `start_sdp_server()`.

2. **Transport events are not connected to sync runtime**
   `TransportEvent::DataReceived` is emitted by the transport layer, but there is no long-lived task routing incoming payloads into `SyncEngine::receive_space_file()`.

3. **Outgoing sync queue is not continuously processed**
   `SyncEngine::process_queue()` exists, but there is no runtime-owned background worker that drains it while a space is active.

4. **Peer connection orchestration is incomplete**
   Discovered devices can exist in memory without a deterministic policy for when to call `connect_peer()`, how to retry, or how to avoid duplicate/symmetric connection attempts.

5. **Cross-device space identity is undefined**
   Local metadata uses random `space_id` values. That is safe locally, but it is not enough to decide which remote space should receive incoming files.

6. **End-to-end validation is missing**
   Current tests prove units and local safety boundaries, not a two-device sync session.

## Recommended Execution Order

### Phase A: Bootstrap LAN runtime

- [x] Start `DiscoveryService::new(...)` during Tauri startup.
- [x] Start the local SDP HTTP server during Tauri startup.
- [x] Store service/task handles in application state so they can be stopped cleanly.
- [x] Pipe discovered devices into `TransportLayer::register_discovered_device(...)`.

Primary files:

- `syncflow/packages/client/src-tauri/src/main.rs`
- `syncflow/packages/core/src/transport/discovery.rs`
- `syncflow/packages/core/src/transport/sdp_exchange.rs`

Acceptance:

- App launch starts discovery and SDP server without manual action.
- `get_discovered_devices()` returns real LAN devices.

### Phase B: Peer session orchestration

- [x] Add a background task that subscribes to transport events.
- [x] Define a connection policy after discovery.
- [x] Recommended rule: the lexicographically smaller `device_id` initiates the connection to avoid both sides dialing at once.
- [x] Add retry/backoff and dedupe for repeated discovery events.

Primary files:

- `syncflow/packages/core/src/transport/mod.rs`
- `syncflow/packages/client/src-tauri/src/main.rs`
- `syncflow/packages/client/src-tauri/src/runtime/manager.rs`

Acceptance:

- Two devices on the same LAN converge to one live WebRTC connection.
- Repeated discovery events do not create duplicate peer sessions.

### Phase C: Runtime-owned sync data plane

- [x] Make each active `SpaceRuntime` own its `SyncEngine` and background tasks instead of creating an engine only for initial indexing.
- [x] Start a queue-drain loop when a space enters `watching`.
- [x] Subscribe incoming `TransportEvent::DataReceived` messages and route them to the correct active space runtime.
- [x] Ensure watcher events always enqueue messages with explicit `space_id` and `relative_path`.

Primary files:

- `syncflow/packages/client/src-tauri/src/runtime/space_runtime.rs`
- `syncflow/packages/client/src-tauri/src/runtime/manager.rs`
- `syncflow/packages/core/src/sync/mod.rs`
- `syncflow/packages/core/src/sync/queue.rs`

Acceptance:

- Local file create/modify/delete in an active space produces outbound transport messages.
- Incoming file payloads are persisted to the correct local space and update metadata.

### Phase D: Define shared space identity

- [x] Add a stable cross-device space key to `synced_spaces`, for example `sync_key TEXT NOT NULL`.
- [x] Do not reuse local `space_id` as the network identity.
- [x] Exchange `sync_key` during sync messages and reject payloads for unknown spaces.
- [x] Add a simple pairing path for now: manual creation/import of the same `sync_key` on both devices.
- [ ] Replace manual `sync_key` input with account-based share/join flows while keeping `sync_key` as an internal shared-space identifier.

Primary files:

- `syncflow/packages/core/src/storage/schema.rs`
- `syncflow/packages/core/src/storage/models.rs`
- `syncflow/packages/core/src/storage/queries.rs`
- `syncflow/packages/core/src/sync/mod.rs`
- `syncflow/packages/client/src-tauri/src/commands.rs`

Acceptance:

- Each incoming payload can be mapped to exactly one local sync space.
- Two devices can use different local folder paths while still syncing the same logical space.

### Phase E: End-to-end verification

- [ ] Add an integration-style harness or manual test script for a two-device LAN session.
- [ ] Verify create, modify, delete, restart, and conflict scenarios.
- [ ] Record expected logs and UI status transitions.
- [ ] Update `README.md` only after the closed loop is actually working.

Acceptance:

- Two desktop clients can complete a full sync loop on a shared space with reproducible manual steps.

Status note:

- A manual validation runbook now exists at `docs/superpowers/plans/2026-04-22-syncflow-lan-manual-validation.md`.
- A PC-to-mobile development validation runbook now exists at `docs/superpowers/plans/2026-04-23-syncflow-pc-mobile-manual-validation.md`.
- The remaining work is real two-device execution and bug fixing based on observed behavior.

## Scope To Avoid For Now

Keep these out of the critical path:

- conflict resolution UI,
- chunked transfer,
- large-file resume,
- mobile support,
- replacing polling with frontend push updates.

Those are valid later phases, but they should not block the first real cross-device sync closure.

## Delivery Definition

The work should be considered complete only when this exact scenario succeeds:

1. Device A and Device B launch the desktop client on the same LAN.
2. They discover each other automatically.
3. They establish one WebRTC data channel.
4. Both have a synced space with the same `sync_key`.
5. A file created in Device A's local folder appears in Device B's mapped local folder.
6. Metadata, runtime status, and conflict counts remain consistent after restart.

Until that scenario passes, the project should still be described as a local runtime workbench with transport foundations, not a finished P2P sync product.
