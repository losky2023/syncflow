# SyncFlow LAN Manual Validation Runbook

Date: 2026-04-22

## Goal

Verify that two desktop clients on the same LAN can:

1. discover each other automatically,
2. establish one WebRTC connection,
3. use the same `sync_key` to map one logical sync space,
4. transfer file changes from Device A to Device B.

This runbook is intentionally manual. The project has reached the point where real two-device verification is more valuable than more local unit coverage.

## Preconditions

- Two machines on the same LAN.
- Both can run the Tauri desktop client build.
- Both use builds from the same code revision.
- No firewall rule blocks local HTTP on port `18080`.

Recommended commands from repository root:

```bash
cargo test --workspace --manifest-path "syncflow/Cargo.toml"
npm --prefix "syncflow/packages/client" run build
cd syncflow/packages/client && npx tauri dev
```

## Test Setup

Choose one shared logical sync space name, for example `demo-space`.

Prepare two different local folders:

- Device A: `D:\syncflow-demo-a`
- Device B: `D:\syncflow-demo-b`

Create the folder on Device A first and add it in the workbench. Note the generated `sync key` shown in the space card.

On Device B, add its own local folder but paste Device A's `sync key` into the optional sync-key input before creating the space.

Result: both devices now represent the same logical space, but with different local roots.

## Validation Steps

### 1. Session and runtime startup

- Login on both devices.
- Confirm the workbench opens.
- Confirm each device shows a local session state of initialized.
- Start sync for the paired space on both devices.
- Confirm runtime state moves through `starting` and `indexing` to `watching`.

### 2. Discovery and connection

- Wait up to 15 seconds after both apps are open.
- Confirm each device appears in the other device's status bar.
- Expected states:
  - first `discovered`
  - then `connected`
- Confirm connected count becomes `1` on both devices.
- Confirm the selected space shows a recent transport event in the status bar, for example `peer connected: ...` or `data received from ...`.

If this fails:

- check both devices are on the same subnet,
- check port `18080` is reachable,
- check only one client instance is bound to that port per machine.

### 3. Create flow

- On Device A, create a new small text file under the synced folder, for example `notes/hello.txt`.
- Put unique content in the file, for example `syncflow-phase-e-create`.
- Wait up to 10 seconds.
- On Device B, confirm the file appears under the paired local folder.
- Open the file and confirm content matches exactly.

### 4. Modify flow

- On Device A, modify the same file and append a second line.
- Wait up to 10 seconds.
- On Device B, confirm content updates.

### 5. Delete flow

- On Device A, delete the same file.
- Wait up to 10 seconds.
- On Device B, confirm the file is removed.

Note: if delete propagation does not complete yet, record it as a known gap rather than assuming the whole sync loop is broken.

### 6. Restart resilience

- Stop the client on Device B.
- On Device A, create another file in the paired folder.
- Restart Device B and login again.
- Start the paired space runtime again.
- Confirm discovery and connection recover.
- Confirm Device B eventually receives the file created while it was offline, or explicitly record that restart catch-up is not implemented yet.

### 7. Conflict visibility

- Create the same relative file on both devices while temporarily disconnected or before connection stabilizes.
- Change contents differently on both sides.
- Reconnect both devices.
- Confirm at least one conflict record appears in the details pane for that space.

## Expected Evidence

Capture these items during testing:

- screenshot of both devices showing the same space with the same `sync_key`,
- screenshot of both devices showing connected peer count,
- screenshot of the status bar transport event text for the selected space,
- before/after file listings for create and delete,
- file contents for modify verification,
- any runtime error banners,
- relevant terminal logs from both machines.

## Pass Criteria

Minimum pass:

- discovery works,
- one connection is established,
- create and modify sync from Device A to Device B works using shared `sync_key`.

Stronger pass:

- delete works,
- restart recovery works,
- conflict records appear when expected.

## Failure Classification

Use these buckets when recording issues:

- `discovery`: peer never appears,
- `connect`: peer discovered but never becomes connected,
- `route`: connected but incoming data cannot be mapped to a local space,
- `write`: mapped payload received but file is not created or updated,
- `conflict`: concurrent edits do not create visible conflict records,
- `recovery`: restart/offline catch-up fails.
