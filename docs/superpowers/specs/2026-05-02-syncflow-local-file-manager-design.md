# SyncFlow Local File Manager Design

Date: 2026-05-02

## Summary

SyncFlow's workbench file tree will evolve from a lazy file browser into a lightweight local file manager. The first version focuses on safe single-item management inside a selected sync space: rename, delete, move, refresh, copy relative path, and reveal in the system file manager.

This design intentionally keeps sync-state visualization, cloud differences, drag-and-drop, multi-select, and recycle-bin semantics out of scope. The goal is to complete the local file management loop without weakening the existing `spaceId + relativePath` safety boundary.

## Approved Direction

Use a menu-driven single-item file manager.

- Each file tree row exposes a compact actions menu.
- Directory rows keep the existing inline create-file and create-folder actions.
- File and directory operations are scoped to one selected item at a time.
- The backend continues to receive only `spaceId + relativePath` style inputs.
- The frontend refreshes affected tree parents after every mutation.

This approach fits the current React component structure and avoids the complexity of drag-and-drop, multi-selection, and cross-space operations.

## Goals

- Add rename, delete, move, refresh, reveal, and copy-path actions to the file tree.
- Keep all file mutations constrained to a registered sync space.
- Preserve lazy tree loading for large folders.
- Refresh the tree, preview, details, and runtime counts after mutations.
- Return user-readable errors for common filesystem failures.
- Keep the implementation compatible with future drag-and-drop and search features.

## Non-goals

- Drag-and-drop moving.
- Multi-select and batch operations.
- Recycle bin or undo support.
- File search or filtering.
- Sync status badges in the tree.
- Cloud-vs-local difference views.
- Cross-space file moves.
- Rich permission management UI.

## User Experience

The file tree remains in the left workbench sidebar. Users manage local files through row actions and keyboard shortcuts.

### Row Actions

Every file and directory row should support:

- Rename.
- Delete.
- Move to another folder in the same sync space.
- Copy relative path.
- Reveal in system file manager.

Directory rows should continue to support:

- New file.
- New folder.
- Expand/collapse.
- Refresh directory.

The tree header should support:

- New file at root.
- New folder at root.
- Refresh root.

### Keyboard Behavior

When a tree row is focused:

- `Enter` selects and previews the item.
- `F2` starts rename.
- `Delete` opens delete confirmation.
- `Escape` cancels rename or creation drafts.

Keyboard shortcuts should not run when focus is inside a text input.

### Rename Flow

Rename uses an inline input in the tree row.

1. User starts rename from the row menu or `F2`.
2. The current basename is selected in an inline input.
3. `Enter` commits.
4. `Escape` cancels.
5. Empty names, `.` / `..`, and names with path separators are rejected before calling the backend.
6. On success, the old parent is refreshed and the renamed node is selected.

### Delete Flow

Delete uses a confirmation dialog or confirmation panel.

1. User chooses delete from the row menu or presses `Delete`.
2. UI shows the name and relative path being deleted.
3. Directories should be described clearly, including that their contents will be deleted.
4. On confirm, the backend deletes the item.
5. On success, the parent directory is refreshed.
6. If the deleted item was selected, preview and details return to an empty or parent-directory state.

The first version may permanently delete files. Recycle-bin behavior is deferred because it varies by platform and requires a separate design.

### Move Flow

Move uses a simple target-folder picker inside the app.

1. User chooses move from the row menu.
2. UI opens a target-folder selector for the current sync space.
3. User picks root or an existing directory.
4. Backend rejects moving an item into itself or one of its descendants.
5. Backend rejects overwriting an existing target.
6. On success, both the old parent and target parent are refreshed.
7. The moved node is selected at its new path.

The first version should use a conservative picker rather than drag-and-drop. The picker can reuse lazy tree loading, but it should only show directories as valid targets.

### Refresh Flow

Refresh should be available at:

- Root tree header.
- Directory row actions.

Refreshing a directory reloads only that directory's children. It should preserve expanded state where possible. If a selected item disappears during refresh, the UI should clear the selection and show a small "file was moved or deleted" style state.

## Tauri Command Boundary

Add typed wrappers in `src/lib/tauriClient.ts` and Tauri commands in `src-tauri/src/commands.rs`.

```text
rename_tree_item(request) -> TreeNode
delete_tree_item(request) -> bool
move_tree_item(request) -> TreeNode
reveal_tree_item(space_id, relative_path) -> bool
```

Suggested request shapes:

```text
RenameTreeItemRequest {
  space_id: String,
  relative_path: String,
  new_name: String,
}

DeleteTreeItemRequest {
  space_id: String,
  relative_path: String,
}

MoveTreeItemRequest {
  space_id: String,
  relative_path: String,
  target_parent_relative_path: Option<String>,
}
```

The frontend wrapper should expose camelCase functions:

```ts
renameTreeItem(spaceId, relativePath, newName)
deleteTreeItem(spaceId, relativePath)
moveTreeItem(spaceId, relativePath, targetParentRelativePath)
revealTreeItem(spaceId, relativePath)
```

`copy relative path` does not need a backend command unless platform clipboard access must stay inside Tauri.

## Filesystem Safety

All backend mutations must follow the existing sync-space safety model.

For every command:

1. Load the sync space by `space_id`.
2. Resolve the source path from `space_id + relative_path`.
3. Reject empty relative paths for item-level mutation commands. The sync-space root itself cannot be renamed, deleted, or moved through the tree.
4. Reject absolute paths, parent traversal, and invalid components before joining.
5. Canonicalize existing source targets and verify they remain inside the canonical sync-space root.
6. For new names, reject empty names, `.`, `..`, and path separators.
7. For move and rename targets, verify the resolved parent is inside the sync-space root.
8. Reject overwriting an existing file or directory.

Move-specific rules:

- Moving a directory into itself is invalid.
- Moving a directory into one of its descendants is invalid.
- Moving to the same parent with the same name is a no-op or user-facing error. Prefer an error to avoid ambiguous UI feedback.
- Cross-space moves are not supported.

Symlink behavior should be conservative. If a source or target parent cannot be canonicalized inside the sync-space root, the command should fail with a user-readable error.

## Backend Behavior

### Rename

Use `std::fs::rename` or `tokio::fs::rename` after validating the source and target.

After rename:

- Return a fresh `TreeNode` for the new path.
- Refresh runtime counts for the space.
- Update or re-index persisted file metadata if existing storage helpers require it. If the watcher/indexer will handle the change, document that assumption in code comments or tests.

### Delete

Delete files with file removal and directories with recursive directory removal.

After delete:

- Return `true` on success.
- Refresh runtime counts for the space.
- Ensure the selected deleted path is not used for subsequent preview/details reads.

### Move

Move preserves the original basename and changes only the parent directory.

After move:

- Return a fresh `TreeNode` for the new path.
- Refresh runtime counts for the space.
- Let the frontend refresh old and new parents.

### Reveal

Open the platform file manager focused on the target where possible. If focusing the exact item is not portable, reveal the parent directory.

The command should still validate `space_id + relative_path` before opening anything.

## Frontend Architecture

Keep the current component split:

- `Workbench.tsx` owns tree state, selection, preview, details, and mutation flows.
- `FileTree.tsx` renders root controls and root-level creation.
- `FileTreeNode.tsx` renders row actions, inline rename state, and recursive children.
- `tauriClient.ts` owns typed command wrappers and result mapping.
- `workbench.ts` owns request/result types.

Add focused state to `Workbench.tsx`:

- `treeActionMenuPath`.
- `treeRenameDraft`.
- `treeRenameName`.
- `treeRenameError`.
- `treeDeleteTarget`.
- `treeMoveDraft`.
- `treeMutationLoading`.
- `treeMutationError`.

Avoid adding a global file-manager store until there is a second surface that needs the same state.

## State Refresh Rules

Mutation flows should refresh only affected parents.

- Create at root: refresh root, select new node.
- Create in directory: refresh parent, expand parent, select new node.
- Rename: refresh old parent, select new node.
- Delete: refresh old parent, clear selection if deleted path was selected or contained the selected path.
- Move: refresh old parent and target parent, expand target parent, select new node.
- Refresh root: reload root children.
- Refresh directory: reload that directory's children.

When a refresh finds that the currently selected item no longer exists, clear preview/details and show a concise missing-file state.

## Error Handling

Map common backend failures to clear user messages.

- Existing target: "同名文件或文件夹已存在".
- Invalid name: "名称不能包含路径分隔符、`.` 或 `..`".
- Missing source: "文件已不存在，已刷新目录".
- Permission denied: "没有权限执行该操作".
- Busy file: "文件正在被其他程序占用".
- Invalid move target: "不能移动到自身或子文件夹".
- Unsafe path: "路径不在当前同步空间内".

The frontend should keep the failed draft open for rename and move validation errors so the user can correct the input.

## Testing

### Rust Tests

Add tests around the command-level path helpers or lower-level filesystem helpers where practical:

- Rename file.
- Rename directory.
- Delete file.
- Delete non-empty directory.
- Move file to another directory.
- Move directory to another directory.
- Reject path traversal.
- Reject root rename/delete/move.
- Reject overwriting an existing target.
- Reject moving a directory into itself.
- Reject moving a directory into its own descendant.

### Frontend Verification

Run:

```bash
npm --prefix syncflow/packages/client run build
```

Manual checks:

- Rename selected file and confirm preview/details follow the new path.
- Delete selected file and confirm preview/details clear.
- Move selected file into another directory and confirm tree refreshes both parents.
- Refresh a directory after external filesystem changes.
- Reveal a file from the row menu.

### Workspace Verification

Run backend tests before submitting implementation:

```bash
cargo test --workspace --manifest-path syncflow/Cargo.toml
```

For UI-only changes, still run the frontend build. For command or path-safety changes, run the Rust tests.

## Future Extensions

After the single-item local manager is stable, consider:

- Drag-and-drop move.
- Multi-select and batch actions.
- Search/filter within the tree.
- Recent files.
- Favorite directories.
- Sort modes.
- Recycle-bin support.
- Sync state badges.
- Local/cloud difference view.
