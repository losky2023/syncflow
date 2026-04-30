# SyncFlow Phase 1 Light Notes Workbench Design

Date: 2026-04-22

## Summary

SyncFlow will move from a tool-style "add folder + manage files" page to a light, notes-app-like three-column workbench. After login, users land directly in the workbench and can add sync spaces, browse a lazy file tree, preview selected content, and inspect details without opening a separate management modal.

Phase 1 is read-only for file content. It does not introduce text editing, save semantics, conflict resolution UI, or advanced sync controls.

## Approved Direction

Use a three-column workbench:

- Left: sync spaces and the selected space's file tree.
- Center: welcome/empty states and file preview.
- Right: selected file or folder details.
- Top: device and sync status.
- Bottom: connected/discovered device status.

The left sidebar uses the "space list + current space file tree" model. Users first choose a sync space, then browse that space's tree below it.

The center preview supports:

- Text files: read-only text preview.
- Image files: direct image preview when technically available.
- Other files: file card with a system-open action.

Image preview in Phase 1 should use a dedicated Tauri command that resolves `space_id + relative_path`, validates the path, reads only supported image formats, and returns a bounded data URL payload:

- `dataUrl`: base64 data URL for the image.
- `mimeType`: detected image MIME type.
- `size`: byte length.
- `truncated`: always `false` for accepted images.

Images larger than 5 MB should not be read into a data URL in Phase 1. If the image is too large or unsupported, the preview pane falls back to the file card.

## Goals

- Replace the old post-login list page with a light three-column workbench.
- Remove the "manage files" modal from the main browsing flow.
- Persist sync spaces across app restarts.
- Use `spaceId + relativePath` for file tree, preview, details, and open-file operations.
- Keep filesystem path validation centralized in the Tauri boundary.
- Structure the frontend into small, focused components.

## Non-goals

- Text editing or save support.
- Conflict resolution workflows.
- Full media preview for every binary format.
- Remote sharing, server relay, or account management.
- Complex compatibility layers for the old in-memory folder list.

## Information Architecture

### Login

`App.tsx` keeps the existing local login gate. On successful login, it starts sync and renders the workbench.

### Workbench Layout

The workbench has these regions:

1. Top status bar
   - App title.
   - Current device name/id.
   - Sync running state.
   - Lightweight status messages.

2. Left sidebar
   - Sync space list.
   - Add-space action.
   - Selected space's lazy file tree.
   - Empty and error states for spaces and tree loading.

3. Center preview pane
   - Welcome state when nothing is selected.
   - Directory state when a folder is selected.
   - Text preview for supported text files.
   - Image preview for supported image files.
   - Fallback file card for unsupported files.

4. Right details pane
   - Name, type, extension, size, modified time.
   - Space name and relative path.
   - Directory/file distinction.
   - System-open action where applicable.

5. Bottom device status
   - Connected peer count/list.
   - Discovered LAN devices.
   - Non-blocking sync/device status.

Phase 1 status updates should use pull-based polling rather than event subscriptions. `Workbench` should poll existing device and discovery commands on a short interval, while the top bar can reuse the login/start-sync state already held in the frontend plus lightweight refreshes from existing device status commands if needed.

## Data Model

Add a persistent `synced_spaces` table managed by core storage.

Minimum fields:

- `id`: stable UUID string used by frontend commands.
- `name`: display name, defaulting to the folder name.
- `root_path`: absolute root path, stored only in backend data.
- `status`: lightweight state such as `ready`, `scanning`, or `error`.
- `created_at`: RFC3339 timestamp.
- `last_scanned_at`: nullable RFC3339 timestamp.

The existing `get_synced_folders` command should switch from the in-memory `synced_folders` vector to this table.

Phase 1 may start fresh for local sync-space registrations. There is no requirement to migrate previously added in-memory folders into `synced_spaces`, because the current model is not persisted across restart anyway.

For Phase 1, `synced_spaces` is a workbench navigation model and registration record for local sync roots. Existing `file_metadata`, `file_versions`, `sync_state`, and `devices` tables do not need to reference `space_id` yet. If future sync metadata needs per-space identity, that migration should be designed in a later phase.

## Tauri Command Boundary

Frontend components must not call `invoke(...)` directly. Add `src/lib/tauriClient.ts` as a typed wrapper around command names and argument shapes.

Phase 1 commands:

- `pick_folder()`
- `get_synced_folders()`
- `add_synced_folder(path)`
- `remove_synced_folder(space_id)`
- `get_tree_children(space_id, parent_relative_path?)`
- `get_file_details(space_id, relative_path)`
- `preview_file_text(space_id, relative_path, max_bytes?)`
- `preview_file_image(space_id, relative_path, max_bytes?)`
- `open_file(space_id, relative_path)`
- `get_device_info()`
- Existing sync/device commands as needed for the top and bottom status areas.

The existing folder-named commands can remain for Phase 1 to reduce churn, but their returned objects should use the new sync-space shape: `id`, `name`, `rootPath`, `status`, `createdAt`, and `lastScannedAt`. `tauriClient.ts` should expose them to React as space-oriented functions such as `listSyncedSpaces`, `addSyncedSpace`, and `removeSyncedSpace`.

At the frontend boundary, `tauriClient.ts` should expose typed camelCase functions and params such as `spaceId` and `relativePath`. Inside the Tauri command payload, argument names can remain snake_case to match Rust conventions. The mapping between the two belongs in `tauriClient.ts`.

`remove_synced_folder` should take only `space_id`. `preview_file_text`, `preview_file_image`, and `open_file` should use `space_id + relative_path` instead of raw absolute paths.

## Filesystem Safety

The frontend must not pass arbitrary absolute paths for browsing, preview, details, or open-file operations after a sync space is registered.

For every `spaceId + relativePath` command, the backend must:

1. Load the sync space by id.
2. Resolve the relative path against the space root.
3. Reject absolute paths, parent traversal components, and empty-invalid path segments before joining.
4. Canonicalize existing targets and verify the canonical target is still inside the canonical sync-space root.
5. Treat symlinks as valid only when their final canonical target remains inside the sync-space root; otherwise reject them.
6. Return a not-found error for non-existent targets for details, preview, and open-file commands.
7. Reject traversal attempts or invalid paths with a user-friendly error.

This validation belongs at the Tauri command boundary.

## Tree Loading

Use a lazy children API:

```text
get_tree_children(space_id, parent_relative_path?) -> TreeNode[]
```

A `TreeNode` includes:

- `name`
- `relativePath`
- `nodeType`: `file` or `directory`
- `hasChildren`
- `extension`
- `size`
- `modifiedAt`

Sort directories before files, then by display name.

Symlink behavior for tree loading should match the safety boundary: symlink entries may be listed only if their canonical target remains inside the sync-space root. Symlinks that resolve outside the root, are broken, or cannot be canonicalized should be omitted from the tree for Phase 1.

The frontend stores:

- Selected space id.
- Selected node.
- Expanded directory paths.
- Children loaded per directory path.
- Loading/error state per directory path.

This keeps the UI ready for large folders and future incremental refresh.

## Frontend Structure

`App.tsx` becomes a thin login shell.

New files:

- `src/app/Workbench.tsx`
- `src/components/sidebar/SpaceList.tsx`
- `src/components/sidebar/FileTree.tsx`
- `src/components/sidebar/FileTreeNode.tsx`
- `src/components/preview/PreviewPane.tsx`
- `src/components/preview/TextPreview.tsx`
- `src/components/preview/ImagePreview.tsx`
- `src/components/preview/FileFallbackCard.tsx`
- `src/components/details/DetailsPane.tsx`
- `src/lib/tauriClient.ts`
- `src/types/workbench.ts`
- A light workbench stylesheet, either centralized or split by component.

Avoid new abstractions beyond these focused units unless needed during implementation.

## State Flow

1. User logs in.
2. `App` starts sync and renders `Workbench`.
3. `Workbench` loads sync spaces and device status.
4. If spaces exist, select the first space by default.
5. Selecting a space loads root children through `get_tree_children` and leaves the tree selection empty until the user selects a node.
6. Expanding a directory lazily loads that directory's children.
7. Selecting a node updates preview and details:
   - Directory: directory placeholder and directory metadata.
   - Text file: text preview and file details.
   - Image file: image preview and file details.
   - Other file: fallback card and file details.

Text preview should remain bounded, with a default maximum of exactly 100 KB in Phase 1. Image preview should initially support common raster image types such as PNG, JPG/JPEG, GIF, and WEBP when they can be rendered safely. SVG should be excluded in Phase 1 unless a later implementation adds explicit sanitization and safe rendering rules.
8. Adding a space persists it, refreshes spaces, and selects the new space.
9. Removing the current space clears selection or selects the next available space.

## Error and Empty States

Required product states:

- No sync spaces: invite the user to add the first space.
- Empty space: file tree shows an empty state.
- No selected node: center pane shows a welcome state.
- Tree loading failure: sidebar shows retry affordance.
- Details loading failure: details pane shows an error state.
- Text preview failure: center pane shows a read failure card.
- Image preview failure: fallback to file card.
- Unsupported preview: fallback file card with system-open action.

## Testing Strategy

Backend unit tests should cover:

- Sync space insert/list/delete behavior.
- Relative path resolution.
- Root escape prevention.
- Tree node construction and sorting.
- Details for files and directories.
- Preview byte limit behavior for text files.

Verification commands:

```bash
cargo test --workspace
cargo fmt --all
cargo clippy --workspace
```

Functional checks:

- Login opens the three-column workbench.
- Add-space opens folder picker and persists the new space.
- Spaces survive app restart.
- Expanding folders loads children lazily.
- Text file selection shows read-only preview.
- Image file selection shows image preview when possible.
- Unsupported file selection shows fallback card and details.
- Removing a space does not delete files on disk.
- Device status remains visible but does not interrupt browsing.

## Implementation Order

1. Define frontend types and `tauriClient.ts`.
2. Add `synced_spaces` model and storage queries.
3. Update Tauri commands to use persistent spaces.
4. Add safe path resolution helpers and tests.
5. Add lazy tree, details, preview, and open-file commands.
6. Refactor `App.tsx` into login shell plus `Workbench`.
7. Build sidebar space list and file tree.
8. Build center preview and right details panes.
9. Apply light theme and loading/error/empty states.
10. Run tests, formatting, clippy, and manual functional checks.
