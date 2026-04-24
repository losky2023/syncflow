# SyncFlow Conflict Viewing And Resolution Design

Date: 2026-04-24

## Summary

SyncFlow already detects and persists version-vector conflicts, but the current workbench only exposes a read-only conflict list. Users can see that a conflict exists, but they cannot inspect meaningful content, choose a resolution, or clear resolved items from the UI.

This design adds a first usable conflict workflow:

- view conflict details,
- compare local and remote text content side by side for new text conflicts,
- resolve by keeping local or keeping remote,
- dismiss a conflict after manual handling,
- deduplicate repeated conflict records for the same file state.

The implementation intentionally stays narrow. It does not attempt a full merge editor, batch conflict processing, or line-level diff rendering in this phase.

## Goals

- Turn conflicts into a complete workbench workflow instead of a read-only list.
- Preserve the existing safe behavior that conflicts do not overwrite local files automatically.
- Add remote text snapshots for newly detected text conflicts so the UI can show a side-by-side compare view.
- Support resolving a conflict by:
  - keeping local,
  - keeping remote when a remote snapshot exists,
  - dismissing a conflict after manual handling.
- Keep old conflicts compatible even though they do not have remote snapshots.
- Prevent duplicate conflict rows for the same file state.

## Non-goals

- Rich merge editing or in-place manual merge save.
- Batch conflict resolution.
- Syntax-aware diffing or line-level highlight rendering.
- Remote snapshot support for binary file compare in this phase.
- Retrofitting old conflicts with remote content that was never stored.

## Current State

Conflicts are currently persisted in `sync_conflicts` and exposed through `get_conflicts(spaceId?)`.

Relevant files:

- `syncflow/packages/core/src/sync/mod.rs`
- `syncflow/packages/core/src/storage/models.rs`
- `syncflow/packages/core/src/storage/queries.rs`
- `syncflow/packages/core/src/storage/schema.rs`
- `syncflow/packages/client/src-tauri/src/commands.rs`
- `syncflow/packages/client/src/components/details/DetailsPane.tsx`

Current limitations:

- The backend only stores conflict metadata, not remote content.
- The frontend only shows a read-only list with version vectors.
- Users cannot resolve or dismiss conflicts.
- Duplicate conflict rows can be inserted for the same file when the same conflict is detected repeatedly.

## Approved Product Scope

This phase implements the following product boundary:

- New text conflicts store a remote text snapshot.
- New text conflicts support side-by-side compare.
- New text conflicts support `keep local`, `keep remote`, and `dismiss`.
- Old conflicts without snapshots remain visible and resolvable with:
  - `keep local`
  - `dismiss`
- Old conflicts without snapshots do not support `keep remote`.
- Non-text conflicts remain metadata-only and do not support content compare in this phase.

## Data Model

### Keep `sync_conflicts` as the primary conflict table

The existing `sync_conflicts` table remains the primary record for:

- space identity,
- file identity,
- local version vector,
- remote version vector,
- remote device id,
- detected time.

It continues to drive:

- conflict counts,
- conflict list rendering,
- per-space conflict loading.

### Add `sync_conflict_snapshots`

Add a new table for stored compare payloads:

```sql
CREATE TABLE IF NOT EXISTS sync_conflict_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conflict_id INTEGER NOT NULL,
    space_id TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    snapshot_kind TEXT NOT NULL,
    content_text TEXT,
    content_truncated INTEGER NOT NULL DEFAULT 0,
    content_size INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);
```

Initial `snapshot_kind` values:

- `remote_text`

This table is intentionally simple. It stores only what the Phase 1 conflict UI needs. It does not attempt a generic snapshot/version archive system.

### Why a separate table

Using a separate table keeps the conflict list lightweight and avoids forcing every conflict row to carry optional content fields. It also lets old conflict rows remain valid without backfilling any new columns.

## Conflict Snapshot Rules

When a remote file arrives and the version vectors conflict:

- If the file is recognized as a text file, the backend stores the remote text snapshot before returning.
- If the file is not recognized as text, the conflict is still persisted, but no compare snapshot is stored.
- The local file is not overwritten.

Old conflicts remain unchanged:

- they still appear in the list,
- they simply have no snapshot row.

## Conflict Deduplication

The backend should avoid inserting repeated conflict rows for the same state.

Deduplication key:

- `space_id`
- `relative_path`
- `local_version`
- `remote_version`
- `remote_device_id`

If a matching unresolved conflict already exists, the backend should not insert a second row or a second identical snapshot.

This is sufficient for this phase and directly addresses the duplicate rows already observed during manual validation.

## Backend Commands

### Keep existing list command

Retain:

- `get_conflicts(space_id?)`

This command remains the source for:

- the conflict list,
- per-space conflict counts,
- workbench summary UI.

### Add `get_conflict_detail(conflict_id)`

This command returns the full detail required for the resolution UI.

Suggested response shape:

- conflict metadata
- local file metadata
- local text preview when the file is text
- remote snapshot when one exists
- capability flags

Suggested fields:

```text
id
spaceId
spaceName
relativePath
remoteDevice
detectedAt
localVersion
remoteVersion
localFileExists
isText
localTextContent?
localTextTruncated?
remoteTextContent?
remoteTextTruncated?
canKeepLocal
canKeepRemote
canCompareText
missingRemoteSnapshotReason?
```

### Add `resolve_conflict_keep_local(conflict_id)`

Behavior:

- do not modify file contents,
- delete the conflict row,
- delete related snapshot rows,
- return success.

This is appropriate for:

- users who already manually fixed the file,
- users who trust the current local content,
- old conflicts with no remote snapshot.

### Add `resolve_conflict_keep_remote(conflict_id)`

Behavior:

- only valid when a remote text snapshot exists,
- rewrite the local file from the stored remote snapshot,
- update local `file_metadata`,
- clear the conflict row and its snapshots,
- return success.

If the target file does not exist locally anymore, the backend should recreate it under the space root using the conflict relative path.

If no remote snapshot exists, the backend must return a clear error such as:

```text
This legacy conflict has no remote snapshot and cannot keep remote
```

### Add `dismiss_conflict(conflict_id)`

Behavior:

- clear the conflict row,
- clear related snapshots,
- do not modify file contents.

This action is intended for cases where the user resolved the conflict outside SyncFlow and only wants to clear the stale record.

## Sync Engine Changes

Conflict detection remains in:

- `syncflow/packages/core/src/sync/mod.rs`

Current behavior should remain:

- detect conflict using version vectors,
- persist a conflict,
- return without overwriting the local file.

New behavior for text conflicts:

1. Parse metadata and decrypt remote content as today.
2. Detect conflict.
3. If the file is text, decode a bounded text snapshot from the remote payload.
4. Save the conflict row if not already present.
5. Save the remote snapshot linked to that conflict row.
6. Return without writing the remote file locally.

This preserves SyncFlow's safety model while adding enough retained data for the resolution UI.

## Frontend Design

### Overall approach

Do not create a separate global conflict center in this phase.

Keep the existing workbench layout and upgrade the right-side conflict section from read-only output to an interactive conflict workflow. This minimizes UI churn and fits the current architecture.

### Conflict list behavior

Within the details pane, each conflict row should show:

- relative path,
- remote device,
- detected time,
- whether text compare is available,
- whether `keep remote` is available.

Selecting a conflict row opens a focused detail view inside the same panel.

### Conflict detail behavior

The conflict detail view should include:

- file path,
- space name,
- remote device,
- detected time,
- current status text describing what actions are available.

For new text conflicts with snapshots:

- show a two-column compare view,
- left column: local current content,
- right column: stored remote conflict snapshot.

For old conflicts without snapshots:

- show metadata only,
- clearly explain that the conflict was created before snapshot support existed.

For non-text conflicts:

- show metadata only,
- explain that content compare is not available for this file type in this phase.

### Actions

Available actions:

- `Keep Local`
- `Keep Remote`
- `Dismiss`

Button rules:

- `Keep Local`: always enabled for an existing conflict
- `Keep Remote`: enabled only when the backend reports `canKeepRemote`
- `Dismiss`: always enabled for an existing conflict

Each destructive action should require a confirmation step with clear wording.

## UI Refresh Behavior

After any successful conflict action, the frontend must refresh:

- selected space conflict list,
- selected space runtime status,
- selected file preview when it points at the same relative path,
- file tree contents if local files were changed by `keep remote`.

If the conflict no longer exists by the time the user acts on it, the frontend should show a non-fatal message and refresh the list.

## Error Handling

### Keep remote without snapshot

The backend rejects the operation. The frontend shows a user-facing message that old conflicts do not have remote content available.

### Local write failure during keep remote

The backend must not delete the conflict record if writing the replacement file fails.

### Missing local file

`keep remote` may recreate the file using the stored relative path.

`keep local` and `dismiss` do not depend on the local file existing.

### Stale conflict id

All mutation commands should return a clear not-found style error if the conflict has already been removed. The frontend should treat this as a refreshable state, not as a fatal error.

## Text Snapshot Bounds

This phase should store bounded text snapshots rather than arbitrary-size payloads.

Suggested behavior:

- reuse the existing text preview byte limit concept,
- store up to a fixed byte threshold for remote text conflicts,
- mark snapshots as truncated when the remote content exceeds the threshold.

This keeps the database predictable and avoids turning conflict snapshots into a large-file storage path.

## TypeScript Model Changes

Extend the frontend types in:

- `syncflow/packages/client/src/types/workbench.ts`

Add:

- `ConflictDetail`
- conflict capability fields
- snapshot availability fields

Keep the existing `ConflictInfo` list type lightweight for list rendering.

## Verification

### Backend

- schema migration creates `sync_conflict_snapshots` without breaking old databases
- old `sync_conflicts` rows remain readable
- duplicate conflict detection prevents repeated inserts
- text conflicts create remote snapshots
- non-text conflicts do not create snapshots
- `resolve_conflict_keep_local` removes the conflict and snapshots
- `resolve_conflict_keep_remote` rewrites the local file, updates metadata, and removes the conflict
- `dismiss_conflict` removes the conflict without touching the file

### Frontend

- conflict list loads and opens detail views
- new text conflicts render side-by-side compare
- old conflicts degrade gracefully
- `keep remote` is disabled when unsupported
- mutation actions refresh counts, list, preview, and tree state

### Manual validation

1. Create a new text conflict between two devices.
2. Confirm the conflict appears in the selected space.
3. Open conflict detail and verify side-by-side compare.
4. Resolve with `Keep Remote` and verify local file replacement.
5. Create another conflict and resolve with `Keep Local`.
6. Confirm the conflict count decreases immediately.
7. Verify an old pre-snapshot conflict still appears but cannot `Keep Remote`.
8. Verify duplicate detections do not create repeated rows for the same conflict state.

## Recommended Implementation Order

1. Add schema and storage support for conflict snapshots and deduplication.
2. Extend sync conflict detection to persist remote text snapshots.
3. Add Tauri commands for conflict detail and resolution actions.
4. Extend TypeScript client bindings and types.
5. Upgrade the details pane to list, inspect, and resolve conflicts.
6. Add backend tests for migration, deduplication, and resolution.
7. Add frontend verification through build and manual dual-instance conflict tests.

## Deferred Work

The following are intentionally deferred:

- line-level visual diff rendering,
- manual merge editing,
- batch conflict actions,
- binary compare support,
- backfilling remote snapshots for old conflicts,
- a standalone conflict center page.
