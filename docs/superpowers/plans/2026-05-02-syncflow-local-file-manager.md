# SyncFlow Local File Manager Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add safe local file manager actions to the SyncFlow workbench file tree: rename, delete, move, refresh, copy relative path, and reveal in the system file manager.

**Architecture:** Keep the existing `spaceId + relativePath` command boundary. Add small Tauri filesystem mutation commands in `commands.rs`, typed wrappers in `tauriClient.ts`, and menu-driven single-item flows in the existing `Workbench`, `FileTree`, and `FileTreeNode` components.

**Tech Stack:** Rust 2021, Tauri commands, React, TypeScript, existing workbench CSS, existing SyncFlow storage/runtime services.

---

## File Structure

- Modify `syncflow/packages/client/src-tauri/src/commands.rs`
  - Add request structs for rename/delete/move.
  - Add path helper functions for existing item mutation and move-target validation.
  - Add Tauri commands: `rename_tree_item`, `delete_tree_item`, `move_tree_item`, `reveal_tree_item`.
  - Add unit tests for helper-level safety behavior.
- Modify `syncflow/packages/client/src-tauri/src/main.rs`
  - Register the new Tauri commands.
- Modify `syncflow/packages/client/src/types/workbench.ts`
  - Add frontend draft/action state types if needed by component props.
- Modify `syncflow/packages/client/src/lib/tauriClient.ts`
  - Add typed wrappers for the new commands.
- Modify `syncflow/packages/client/src/app/Workbench.tsx`
  - Own rename/delete/move/action-menu state.
  - Implement mutation handlers and refresh rules.
  - Pass new props into `FileTree`.
- Modify `syncflow/packages/client/src/components/sidebar/FileTree.tsx`
  - Add refresh root and mutation props.
  - Render move-target selector when the move picker needs root/directory options.
- Modify `syncflow/packages/client/src/components/sidebar/FileTreeNode.tsx`
  - Add row action menu, inline rename input, keyboard handlers, refresh directory, and move target actions.
- Modify `syncflow/packages/client/src/styles/workbench.css`
  - Add compact styles for action menus, rename input, confirmation dialog, and move picker.

## Current Context

The existing implementation already has:

- `get_tree_children`, `create_tree_file`, and `create_tree_folder` Tauri commands.
- `CreateTreeItemRequest`, `TreeNode`, `tree_node_from_path`, `validate_new_child_name`, and `validate_relative_path` in `commands.rs`.
- `getTreeChildren`, `createTreeFile`, and `createTreeFolder` wrappers in `tauriClient.ts`.
- `Workbench.tsx` state for `rootNodes`, `selectedNode`, `expandedPaths`, `childrenByPath`, per-path loading/errors, and create drafts.
- `refreshTreeParent(spaceId, parentRelativePath)` in `Workbench.tsx`.
- `FileTree` and `FileTreeNode` recursive rendering.

Prefer extending these structures instead of creating a new file-manager store.

---

### Task 1: Backend Path Helpers and Tests

**Files:**
- Modify: `syncflow/packages/client/src-tauri/src/commands.rs`

- [ ] **Step 1: Add helper tests first**

Append this test module near the bottom of `commands.rs`, after helper functions. If a test module already exists in that file by the time this task runs, merge these tests into it instead of creating a second `mod tests`.

```rust
#[cfg(test)]
mod local_file_manager_tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};

    struct TestSpace {
        path: PathBuf,
    }

    impl TestSpace {
        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestSpace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn make_space() -> TestSpace {
        let path = std::env::temp_dir().join(format!(
            "syncflow-local-file-manager-test-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        TestSpace { path }
    }

    #[test]
    fn mutation_source_rejects_empty_root_path() {
        let dir = make_space();
        let root = fs::canonicalize(dir.path()).expect("canonical root");

        let result = resolve_existing_tree_item_path(&root, "");

        assert_eq!(
            result.expect_err("root mutation should fail"),
            "不能直接操作同步空间根目录"
        );
    }

    #[test]
    fn mutation_source_rejects_path_traversal() {
        let dir = make_space();
        let root = fs::canonicalize(dir.path()).expect("canonical root");

        let result = resolve_existing_tree_item_path(&root, "../outside.txt");

        assert_eq!(
            result.expect_err("path traversal should fail"),
            "Relative path must not contain '..'"
        );
    }

    #[test]
    fn mutation_source_resolves_existing_file_inside_root() {
        let dir = make_space();
        fs::write(dir.path().join("note.md"), "hello").expect("write note");
        let root = fs::canonicalize(dir.path()).expect("canonical root");

        let result = resolve_existing_tree_item_path(&root, "note.md").expect("resolved file");

        assert_eq!(result.file_name().and_then(|value| value.to_str()), Some("note.md"));
        assert!(result.starts_with(&root));
    }

    #[test]
    fn rename_target_rejects_existing_sibling() {
        let dir = make_space();
        fs::write(dir.path().join("old.md"), "old").expect("write old");
        fs::write(dir.path().join("new.md"), "new").expect("write new");
        let root = fs::canonicalize(dir.path()).expect("canonical root");
        let source = resolve_existing_tree_item_path(&root, "old.md").expect("source");

        let result = resolve_rename_target_path(&root, &source, "new.md");

        assert_eq!(
            result.expect_err("existing target should fail"),
            "同名文件或文件夹已存在"
        );
    }

    #[test]
    fn move_target_rejects_directory_descendant() {
        let dir = make_space();
        fs::create_dir_all(dir.path().join("a/b")).expect("create dirs");
        let root = fs::canonicalize(dir.path()).expect("canonical root");
        let source = resolve_existing_tree_item_path(&root, "a").expect("source");

        let result = resolve_move_target_path(&root, &source, Some("a/b"));

        assert_eq!(
            result.expect_err("descendant move should fail"),
            "不能移动到自身或子文件夹"
        );
    }

    #[test]
    fn move_target_resolves_root_parent() {
        let dir = make_space();
        fs::create_dir_all(dir.path().join("folder")).expect("create folder");
        fs::write(dir.path().join("folder/note.md"), "hello").expect("write note");
        let root = fs::canonicalize(dir.path()).expect("canonical root");
        let source = resolve_existing_tree_item_path(&root, "folder/note.md").expect("source");

        let target = resolve_move_target_path(&root, &source, None).expect("target");

        assert_eq!(target, root.join("note.md"));
    }
}
```

- [ ] **Step 2: Run tests and verify they fail because helpers are missing**

Run:

```bash
cargo test --manifest-path syncflow/Cargo.toml --package syncflow-client local_file_manager_tests
```

Expected: FAIL with unresolved function errors for `resolve_existing_tree_item_path`, `resolve_rename_target_path`, and `resolve_move_target_path`.

- [ ] **Step 3: Add helper implementations**

Add these helper functions near existing helpers `resolve_new_child_path`, `validate_new_child_name`, and `tree_node_from_path` in `commands.rs`:

```rust
fn resolve_existing_tree_item_path(root: &Path, relative_path: &str) -> Result<PathBuf, String> {
    if relative_path.trim().is_empty() {
        return Err("不能直接操作同步空间根目录".to_string());
    }
    validate_relative_path(relative_path)?;
    let target = root.join(relative_path);
    let target = std::fs::canonicalize(&target).map_err(|e| format!("文件已不存在: {e}"))?;
    if !target.starts_with(root) {
        return Err("路径不在当前同步空间内".to_string());
    }
    Ok(target)
}

fn resolve_rename_target_path(root: &Path, source: &Path, new_name: &str) -> Result<PathBuf, String> {
    let name = validate_new_child_name(new_name)?;
    let parent = source
        .parent()
        .ok_or_else(|| "无法读取父目录".to_string())?;
    let parent = std::fs::canonicalize(parent).map_err(|e| format!("父目录不可访问: {e}"))?;
    if !parent.starts_with(root) {
        return Err("路径不在当前同步空间内".to_string());
    }
    let target = parent.join(name);
    if target.exists() {
        return Err("同名文件或文件夹已存在".to_string());
    }
    if !target.starts_with(root) {
        return Err("路径不在当前同步空间内".to_string());
    }
    Ok(target)
}

fn resolve_move_target_path(
    root: &Path,
    source: &Path,
    target_parent_relative_path: Option<&str>,
) -> Result<PathBuf, String> {
    let target_parent_relative_path = target_parent_relative_path.unwrap_or("").trim();
    validate_relative_path(target_parent_relative_path)?;
    let parent = if target_parent_relative_path.is_empty() {
        root.to_path_buf()
    } else {
        root.join(target_parent_relative_path)
    };
    let parent = std::fs::canonicalize(&parent).map_err(|e| format!("目标文件夹不可访问: {e}"))?;
    if !parent.starts_with(root) {
        return Err("路径不在当前同步空间内".to_string());
    }
    let parent_metadata = std::fs::metadata(&parent).map_err(|e| format!("读取目标文件夹失败: {e}"))?;
    if !parent_metadata.is_dir() {
        return Err("只能移动到文件夹中".to_string());
    }
    let source_metadata = std::fs::metadata(source).map_err(|e| format!("读取源文件失败: {e}"))?;
    if source_metadata.is_dir() && (parent == source || parent.starts_with(source)) {
        return Err("不能移动到自身或子文件夹".to_string());
    }
    let name = source
        .file_name()
        .ok_or_else(|| "无法读取文件名".to_string())?;
    let target = parent.join(name);
    if target.exists() {
        return Err("同名文件或文件夹已存在".to_string());
    }
    if !target.starts_with(root) {
        return Err("路径不在当前同步空间内".to_string());
    }
    Ok(target)
}
```

- [ ] **Step 4: Run helper tests and verify they pass**

Run:

```bash
cargo test --manifest-path syncflow/Cargo.toml --package syncflow-client local_file_manager_tests
```

Expected: PASS for all `local_file_manager_tests`.

- [ ] **Step 5: Commit backend helper work**

```bash
git add syncflow/packages/client/src-tauri/src/commands.rs
git commit -m "feat: add local file manager path helpers"
```

---

### Task 2: Backend Mutation Commands

**Files:**
- Modify: `syncflow/packages/client/src-tauri/src/commands.rs`
- Modify: `syncflow/packages/client/src-tauri/src/main.rs`

- [ ] **Step 1: Add request structs**

Add these structs after `CreateTreeItemRequest` in `commands.rs`:

```rust
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameTreeItemRequest {
    pub space_id: String,
    pub relative_path: String,
    pub new_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteTreeItemRequest {
    pub space_id: String,
    pub relative_path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveTreeItemRequest {
    pub space_id: String,
    pub relative_path: String,
    pub target_parent_relative_path: Option<String>,
}
```

- [ ] **Step 2: Add Tauri command functions**

Add these commands after `create_tree_folder` and before preview commands in `commands.rs`:

```rust
#[tauri::command]
pub async fn rename_tree_item(
    request: RenameTreeItemRequest,
    state: State<'_, TauriState>,
) -> Result<TreeNode, String> {
    let (space, root) = resolve_space_path(&state, &request.space_id, None).await?;
    let source = resolve_existing_tree_item_path(&root, &request.relative_path)?;
    let target = resolve_rename_target_path(&root, &source, &request.new_name)?;
    tokio::fs::rename(&source, &target)
        .await
        .map_err(|e| format!("重命名失败: {e}"))?;
    state.runtime_manager.refresh_space_counts(space.id).await;
    tree_node_from_path(&root, &target)
}

#[tauri::command]
pub async fn delete_tree_item(
    request: DeleteTreeItemRequest,
    state: State<'_, TauriState>,
) -> Result<bool, String> {
    let (space, root) = resolve_space_path(&state, &request.space_id, None).await?;
    let source = resolve_existing_tree_item_path(&root, &request.relative_path)?;
    let metadata = tokio::fs::metadata(&source)
        .await
        .map_err(|e| format!("读取条目信息失败: {e}"))?;
    if metadata.is_dir() {
        tokio::fs::remove_dir_all(&source)
            .await
            .map_err(|e| format!("删除文件夹失败: {e}"))?;
    } else {
        tokio::fs::remove_file(&source)
            .await
            .map_err(|e| format!("删除文件失败: {e}"))?;
    }
    state.runtime_manager.refresh_space_counts(space.id).await;
    Ok(true)
}

#[tauri::command]
pub async fn move_tree_item(
    request: MoveTreeItemRequest,
    state: State<'_, TauriState>,
) -> Result<TreeNode, String> {
    let (space, root) = resolve_space_path(&state, &request.space_id, None).await?;
    let source = resolve_existing_tree_item_path(&root, &request.relative_path)?;
    let target = resolve_move_target_path(
        &root,
        &source,
        request.target_parent_relative_path.as_deref(),
    )?;
    tokio::fs::rename(&source, &target)
        .await
        .map_err(|e| format!("移动失败: {e}"))?;
    state.runtime_manager.refresh_space_counts(space.id).await;
    tree_node_from_path(&root, &target)
}
```

- [ ] **Step 3: Add reveal command**

Add this command after `open_file` in `commands.rs`:

```rust
#[tauri::command]
pub async fn reveal_tree_item(
    space_id: String,
    relative_path: String,
    state: State<'_, TauriState>,
) -> Result<bool, String> {
    let (_, resolved_path) = resolve_space_path(&state, &space_id, Some(&relative_path)).await?;
    let file_path = resolved_path.to_string_lossy().to_string();

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(format!("/select,{file_path}"))
            .spawn()
            .map_err(|e| format!("无法在系统文件管理器中显示: {e}"))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-R", &file_path])
            .spawn()
            .map_err(|e| format!("无法在系统文件管理器中显示: {e}"))?;
    }
    #[cfg(target_os = "linux")]
    {
        let reveal_path = if resolved_path.is_dir() {
            resolved_path
        } else {
            resolved_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or(resolved_path)
        };
        std::process::Command::new("xdg-open")
            .arg(reveal_path)
            .spawn()
            .map_err(|e| format!("无法在系统文件管理器中显示: {e}"))?;
    }
    Ok(true)
}
```

- [ ] **Step 4: Register commands**

Add these entries in the `tauri::generate_handler!` list in `syncflow/packages/client/src-tauri/src/main.rs`, directly after `commands::create_tree_folder`:

```rust
            commands::rename_tree_item,
            commands::delete_tree_item,
            commands::move_tree_item,
            commands::reveal_tree_item,
```

- [ ] **Step 5: Run Rust check**

Run:

```bash
cargo check --workspace --manifest-path syncflow/Cargo.toml
```

Expected: PASS. If Linux ownership of `resolved_path` in `reveal_tree_item` causes a type mismatch, change the Linux block to compute a `PathBuf`:

```rust
let reveal_path = if resolved_path.is_dir() {
    resolved_path.clone()
} else {
    resolved_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| resolved_path.clone())
};
```

- [ ] **Step 6: Commit backend commands**

```bash
git add syncflow/packages/client/src-tauri/src/commands.rs syncflow/packages/client/src-tauri/src/main.rs
git commit -m "feat: add local file manager commands"
```

---

### Task 3: Frontend Command Wrappers

**Files:**
- Modify: `syncflow/packages/client/src/lib/tauriClient.ts`

- [ ] **Step 1: Add wrapper functions**

Add these functions after `createTreeFolder` in `tauriClient.ts`:

```ts
export async function renameTreeItem(
  spaceId: string,
  relativePath: string,
  newName: string,
): Promise<TreeNode> {
  try {
    const invoke = getInvoke();
    const result = await invoke<any>("rename_tree_item", {
      request: { spaceId, relativePath, newName },
    });
    return mapTreeNode(result);
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function deleteTreeItem(
  spaceId: string,
  relativePath: string,
): Promise<boolean> {
  try {
    const invoke = getInvoke();
    return await invoke<boolean>("delete_tree_item", {
      request: { spaceId, relativePath },
    });
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function moveTreeItem(
  spaceId: string,
  relativePath: string,
  targetParentRelativePath: string | null,
): Promise<TreeNode> {
  try {
    const invoke = getInvoke();
    const result = await invoke<any>("move_tree_item", {
      request: { spaceId, relativePath, targetParentRelativePath },
    });
    return mapTreeNode(result);
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function revealTreeItem(
  spaceId: string,
  relativePath: string,
): Promise<boolean> {
  try {
    const invoke = getInvoke();
    return await invoke<boolean>("reveal_tree_item", { spaceId, relativePath });
  } catch (error) {
    throw normalizeError(error);
  }
}
```

- [ ] **Step 2: Run TypeScript build to catch wrapper errors**

Run:

```bash
npm --prefix syncflow/packages/client run build
```

Expected: PASS or fail only on later missing UI usage if another task has already started. If this isolated task is run alone, it should pass.

- [ ] **Step 3: Commit wrappers**

```bash
git add syncflow/packages/client/src/lib/tauriClient.ts
git commit -m "feat: add file manager tauri wrappers"
```

---

### Task 4: Workbench Mutation State and Handlers

**Files:**
- Modify: `syncflow/packages/client/src/app/Workbench.tsx`

- [ ] **Step 1: Update imports**

Add the new wrapper imports from `../lib/tauriClient` in `Workbench.tsx`:

```ts
  deleteTreeItem,
  moveTreeItem,
  renameTreeItem,
  revealTreeItem,
```

- [ ] **Step 2: Add helper functions and state**

Add these helpers near existing tree helper functions or near `previewTabId`:

```ts
function parentRelativePath(relativePath: string): string | null {
  const parts = relativePath.split("/").filter(Boolean);
  parts.pop();
  return parts.length > 0 ? parts.join("/") : null;
}

function pathIsSameOrChild(path: string, possibleParent: string) {
  return path === possibleParent || path.startsWith(`${possibleParent}/`);
}

function basename(relativePath: string) {
  const parts = relativePath.split("/").filter(Boolean);
  return parts[parts.length - 1] ?? relativePath;
}
```

Add this state near existing tree create state:

```ts
  const [treeActionMenuPath, setTreeActionMenuPath] = useState<string | null>(null);
  const [treeRenameDraft, setTreeRenameDraft] = useState<TreeNode | null>(null);
  const [treeRenameName, setTreeRenameName] = useState("");
  const [treeRenameError, setTreeRenameError] = useState<string | null>(null);
  const [treeDeleteTarget, setTreeDeleteTarget] = useState<TreeNode | null>(null);
  const [treeMoveDraft, setTreeMoveDraft] = useState<TreeNode | null>(null);
  const [treeMoveTargetPath, setTreeMoveTargetPath] = useState<string | null>(null);
  const [treeMutationLoading, setTreeMutationLoading] = useState(false);
  const [treeMutationError, setTreeMutationError] = useState<string | null>(null);
```

- [ ] **Step 3: Add mutation handlers**

Add these handlers after `handleCancelTreeCreate`:

```ts
  function handleStartRename(node: TreeNode) {
    setTreeActionMenuPath(null);
    setTreeRenameDraft(node);
    setTreeRenameName(node.name);
    setTreeRenameError(null);
    setTreeMutationError(null);
  }

  function handleCancelRename() {
    if (treeMutationLoading) return;
    setTreeRenameDraft(null);
    setTreeRenameName("");
    setTreeRenameError(null);
  }

  async function handleCommitRename() {
    if (!selectedSpaceId || !treeRenameDraft || treeMutationLoading) return;
    const newName = treeRenameName.trim();
    if (!newName) {
      setTreeRenameError("请输入名称");
      return;
    }
    setTreeMutationLoading(true);
    setTreeRenameError(null);
    try {
      const renamed = await renameTreeItem(selectedSpaceId, treeRenameDraft.relativePath, newName);
      await refreshTreeParent(selectedSpaceId, parentRelativePath(treeRenameDraft.relativePath));
      setTreeRenameDraft(null);
      setTreeRenameName("");
      setSelectedNode(renamed);
      if (renamed.nodeType === "file") {
        await handleSelectNode(renamed);
      } else {
        setPreview({ type: "directory", node: renamed });
      }
      await loadRuntimeStatuses();
    } catch (error) {
      setTreeRenameError(error instanceof Error ? error.message : String(error));
    } finally {
      setTreeMutationLoading(false);
    }
  }

  function handleRequestDelete(node: TreeNode) {
    setTreeActionMenuPath(null);
    setTreeDeleteTarget(node);
    setTreeMutationError(null);
  }

  async function handleConfirmDelete() {
    if (!selectedSpaceId || !treeDeleteTarget || treeMutationLoading) return;
    const target = treeDeleteTarget;
    setTreeMutationLoading(true);
    setTreeMutationError(null);
    try {
      await deleteTreeItem(selectedSpaceId, target.relativePath);
      await refreshTreeParent(selectedSpaceId, parentRelativePath(target.relativePath));
      if (selectedNode && pathIsSameOrChild(selectedNode.relativePath, target.relativePath)) {
        setSelectedNode(null);
        setDetails(null);
        setDetailsError(null);
        setPreview({ type: "welcome" });
        setActivePreviewTabId(null);
      }
      setTreeDeleteTarget(null);
      await loadRuntimeStatuses();
    } catch (error) {
      setTreeMutationError(error instanceof Error ? error.message : String(error));
    } finally {
      setTreeMutationLoading(false);
    }
  }

  function handleCancelDelete() {
    if (treeMutationLoading) return;
    setTreeDeleteTarget(null);
    setTreeMutationError(null);
  }

  function handleStartMove(node: TreeNode) {
    setTreeActionMenuPath(null);
    setTreeMoveDraft(node);
    setTreeMoveTargetPath(parentRelativePath(node.relativePath));
    setTreeMutationError(null);
  }

  async function handleCommitMove() {
    if (!selectedSpaceId || !treeMoveDraft || treeMutationLoading) return;
    const source = treeMoveDraft;
    setTreeMutationLoading(true);
    setTreeMutationError(null);
    try {
      const moved = await moveTreeItem(selectedSpaceId, source.relativePath, treeMoveTargetPath);
      await refreshTreeParent(selectedSpaceId, parentRelativePath(source.relativePath));
      await refreshTreeParent(selectedSpaceId, treeMoveTargetPath);
      if (treeMoveTargetPath) {
        setExpandedPaths((current) => new Set(current).add(treeMoveTargetPath));
      }
      setTreeMoveDraft(null);
      setTreeMoveTargetPath(null);
      setSelectedNode(moved);
      if (moved.nodeType === "file") {
        await handleSelectNode(moved);
      } else {
        setPreview({ type: "directory", node: moved });
      }
      await loadRuntimeStatuses();
    } catch (error) {
      setTreeMutationError(error instanceof Error ? error.message : String(error));
    } finally {
      setTreeMutationLoading(false);
    }
  }

  function handleCancelMove() {
    if (treeMutationLoading) return;
    setTreeMoveDraft(null);
    setTreeMoveTargetPath(null);
    setTreeMutationError(null);
  }

  async function handleRevealTreeItem(node: TreeNode) {
    if (!selectedSpaceId) return;
    setTreeActionMenuPath(null);
    setTreeMutationError(null);
    try {
      await revealTreeItem(selectedSpaceId, node.relativePath);
    } catch (error) {
      setTreeMutationError(error instanceof Error ? error.message : String(error));
    }
  }

  async function handleCopyRelativePath(node: TreeNode) {
    setTreeActionMenuPath(null);
    setTreeMutationError(null);
    try {
      await navigator.clipboard.writeText(node.relativePath);
    } catch {
      setTreeMutationError("复制路径失败");
    }
  }

  async function handleRefreshTreePath(parentRelativePathValue: string | null) {
    if (!selectedSpaceId) return;
    setTreeMutationError(null);
    await refreshTreeParent(selectedSpaceId, parentRelativePathValue);
  }
```

- [ ] **Step 4: Run TypeScript build and expect prop errors**

Run:

```bash
npm --prefix syncflow/packages/client run build
```

Expected: FAIL if new props are not wired into `FileTree` yet, or PASS if no props are referenced yet. Continue to Task 5.

- [ ] **Step 5: Commit handler state if build passes**

Only commit in this step if Task 4 build passes without UI prop changes. If it fails because `FileTree` props are not yet wired, skip this commit and include Task 4 changes in the Task 5 commit.

```bash
git add syncflow/packages/client/src/app/Workbench.tsx
git commit -m "feat: add workbench file mutation handlers"
```

---

### Task 5: File Tree Row Actions and Inline Rename

**Files:**
- Modify: `syncflow/packages/client/src/components/sidebar/FileTree.tsx`
- Modify: `syncflow/packages/client/src/components/sidebar/FileTreeNode.tsx`
- Modify: `syncflow/packages/client/src/app/Workbench.tsx`

- [ ] **Step 1: Extend `FileTreeProps`**

In `FileTree.tsx`, add these props to `FileTreeProps`:

```ts
  actionMenuPath: string | null;
  renameDraft: TreeNode | null;
  renameName: string;
  renameError: string | null;
  mutationLoading: boolean;
  onActionMenuChange: (relativePath: string | null) => void;
  onStartRename: (node: TreeNode) => void;
  onRenameNameChange: (value: string) => void;
  onCommitRename: () => void;
  onCancelRename: () => void;
  onRequestDelete: (node: TreeNode) => void;
  onStartMove: (node: TreeNode) => void;
  onCopyRelativePath: (node: TreeNode) => void;
  onReveal: (node: TreeNode) => void;
  onRefreshPath: (relativePath: string | null) => void;
```

Add the same names to the function destructuring.

- [ ] **Step 2: Add root refresh button**

In the tree header actions in `FileTree.tsx`, add this button before the create buttons:

```tsx
          <button
            type="button"
            className="tree-action-button"
            onClick={() => onRefreshPath(null)}
            title="刷新"
          >
            ↻
          </button>
```

- [ ] **Step 3: Forward props into `FileTreeNode`**

Pass the new props to each `FileTreeNode` in `FileTree.tsx`:

```tsx
              actionMenuPath={actionMenuPath}
              renameDraft={renameDraft}
              renameName={renameName}
              renameError={renameError}
              mutationLoading={mutationLoading}
              onActionMenuChange={onActionMenuChange}
              onStartRename={onStartRename}
              onRenameNameChange={onRenameNameChange}
              onCommitRename={onCommitRename}
              onCancelRename={onCancelRename}
              onRequestDelete={onRequestDelete}
              onStartMove={onStartMove}
              onCopyRelativePath={onCopyRelativePath}
              onReveal={onReveal}
              onRefreshPath={onRefreshPath}
```

- [ ] **Step 4: Extend `FileTreeNodeProps`**

In `FileTreeNode.tsx`, add the same props to `FileTreeNodeProps` and the function destructuring.

- [ ] **Step 5: Add inline rename render path**

In `FileTreeNode.tsx`, compute:

```ts
  const isRenaming = renameDraft?.relativePath === node.relativePath;
```

Before the current row `<button>`, add this conditional render:

```tsx
      {isRenaming ? (
        <div className="tree-rename-row" style={{ paddingLeft: `${10 + depth * 12}px` }}>
          <span className={isDirectory ? "tree-icon directory" : "tree-icon file"} />
          <input
            autoFocus
            value={renameName}
            disabled={mutationLoading}
            onChange={(event) => onRenameNameChange(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") onCommitRename();
              if (event.key === "Escape") onCancelRename();
            }}
          />
          {renameError ? <span className="tree-create-error">{renameError}</span> : null}
        </div>
      ) : null}
```

Then wrap the existing row button with `{!isRenaming ? (...) : null}` so the normal row is hidden during rename.

- [ ] **Step 6: Add keyboard handlers to the row button**

On the existing row button in `FileTreeNode.tsx`, add:

```tsx
        onKeyDown={(event) => {
          if (event.key === "F2") {
            event.preventDefault();
            onStartRename(node);
          }
          if (event.key === "Delete" || event.key === "Backspace") {
            event.preventDefault();
            onRequestDelete(node);
          }
          if (event.key === "Enter") {
            event.preventDefault();
            handleRowClick();
          }
          if (event.key === "Escape") {
            onActionMenuChange(null);
          }
        }}
```

- [ ] **Step 7: Add action menu UI**

Inside the existing `tree-row-actions` span in `FileTreeNode.tsx`, after the create buttons for directories, add this menu button and panel. For files, render a `tree-row-actions` span too, not only directories.

```tsx
            <span
              role="button"
              tabIndex={0}
              className="tree-row-action"
              title="更多"
              onClick={(event) => {
                event.stopPropagation();
                onActionMenuChange(actionMenuPath === node.relativePath ? null : node.relativePath);
              }}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.stopPropagation();
                  onActionMenuChange(actionMenuPath === node.relativePath ? null : node.relativePath);
                }
              }}
            >
              ⋯
            </span>
            {actionMenuPath === node.relativePath ? (
              <span className="tree-action-menu" onClick={(event) => event.stopPropagation()}>
                <button type="button" onClick={() => onStartRename(node)}>重命名</button>
                <button type="button" onClick={() => onStartMove(node)}>移动到...</button>
                {isDirectory ? (
                  <button type="button" onClick={() => onRefreshPath(node.relativePath)}>刷新</button>
                ) : null}
                <button type="button" onClick={() => onCopyRelativePath(node)}>复制路径</button>
                <button type="button" onClick={() => onReveal(node)}>在系统中显示</button>
                <button type="button" className="danger" onClick={() => onRequestDelete(node)}>删除</button>
              </span>
            ) : null}
```

If the current `tree-row-actions` is only rendered for directories, change it to always render. Keep the create-file and create-folder buttons inside `{isDirectory ? (...) : null}`.

- [ ] **Step 8: Forward props recursively**

In the recursive `<FileTreeNode />` call, pass all new props exactly as in Step 3.

- [ ] **Step 9: Wire `FileTree` props from `Workbench`**

In the `<FileTree />` call in `Workbench.tsx`, add:

```tsx
            actionMenuPath={treeActionMenuPath}
            renameDraft={treeRenameDraft}
            renameName={treeRenameName}
            renameError={treeRenameError}
            mutationLoading={treeMutationLoading}
            onActionMenuChange={setTreeActionMenuPath}
            onStartRename={handleStartRename}
            onRenameNameChange={setTreeRenameName}
            onCommitRename={() => void handleCommitRename()}
            onCancelRename={handleCancelRename}
            onRequestDelete={handleRequestDelete}
            onStartMove={handleStartMove}
            onCopyRelativePath={(node) => void handleCopyRelativePath(node)}
            onReveal={(node) => void handleRevealTreeItem(node)}
            onRefreshPath={(relativePath) => void handleRefreshTreePath(relativePath)}
```

- [ ] **Step 10: Run TypeScript build**

Run:

```bash
npm --prefix syncflow/packages/client run build
```

Expected: PASS.

- [ ] **Step 11: Commit row actions**

```bash
git add syncflow/packages/client/src/app/Workbench.tsx syncflow/packages/client/src/components/sidebar/FileTree.tsx syncflow/packages/client/src/components/sidebar/FileTreeNode.tsx
git commit -m "feat: add file tree row actions"
```

---

### Task 6: Delete Confirmation and Move Picker

**Files:**
- Modify: `syncflow/packages/client/src/app/Workbench.tsx`

- [ ] **Step 1: Add directory option collector**

Add this helper inside `Workbench.tsx`, near other tree helpers:

```ts
  function directoryOptions() {
    const options: TreeNode[] = [];
    const visit = (nodes: TreeNode[]) => {
      nodes.forEach((node) => {
        if (node.nodeType !== "directory") return;
        options.push(node);
        visit(childrenByPath[node.relativePath] ?? []);
      });
    };
    visit(rootNodes);
    return options;
  }
```

Inside the component render, before `return`, add:

```ts
  const moveDirectoryOptions = directoryOptions().filter((node) => {
    if (!treeMoveDraft || treeMoveDraft.nodeType !== "directory") return true;
    return !pathIsSameOrChild(node.relativePath, treeMoveDraft.relativePath);
  });
```

- [ ] **Step 2: Render mutation error**

Near the top of the workbench shell content, or directly above `<main className=...>`, render:

```tsx
      {treeMutationError ? (
        <div className="error-banner error-banner-compact">{treeMutationError}</div>
      ) : null}
```

- [ ] **Step 3: Render delete confirmation**

Inside the workbench shell but outside the main grid, add:

```tsx
      {treeDeleteTarget ? (
        <div className="modal-backdrop">
          <section className="file-manager-dialog" role="dialog" aria-modal="true" aria-label="删除确认">
            <h2>删除 {treeDeleteTarget.name}</h2>
            <p>
              {treeDeleteTarget.nodeType === "directory"
                ? "该文件夹及其中内容会被删除。"
                : "该文件会被删除。"}
            </p>
            <code>{treeDeleteTarget.relativePath}</code>
            {treeMutationError ? <div className="tree-create-error">{treeMutationError}</div> : null}
            <div className="dialog-actions">
              <button type="button" onClick={handleCancelDelete} disabled={treeMutationLoading}>
                取消
              </button>
              <button
                type="button"
                className="danger-button"
                onClick={() => void handleConfirmDelete()}
                disabled={treeMutationLoading}
              >
                {treeMutationLoading ? "删除中..." : "删除"}
              </button>
            </div>
          </section>
        </div>
      ) : null}
```

- [ ] **Step 4: Render move picker**

Add this next to the delete dialog:

```tsx
      {treeMoveDraft ? (
        <div className="modal-backdrop">
          <section className="file-manager-dialog" role="dialog" aria-modal="true" aria-label="移动到">
            <h2>移动 {treeMoveDraft.name}</h2>
            <p>选择当前同步空间内的目标文件夹。</p>
            <label className="move-target-field">
              目标文件夹
              <select
                value={treeMoveTargetPath ?? ""}
                disabled={treeMutationLoading}
                onChange={(event) => setTreeMoveTargetPath(event.target.value || null)}
              >
                <option value="">仓库根目录</option>
                {moveDirectoryOptions.map((node) => (
                  <option key={node.relativePath} value={node.relativePath}>
                    {node.relativePath}
                  </option>
                ))}
              </select>
            </label>
            {treeMutationError ? <div className="tree-create-error">{treeMutationError}</div> : null}
            <div className="dialog-actions">
              <button type="button" onClick={handleCancelMove} disabled={treeMutationLoading}>
                取消
              </button>
              <button type="button" onClick={() => void handleCommitMove()} disabled={treeMutationLoading}>
                {treeMutationLoading ? "移动中..." : "移动"}
              </button>
            </div>
          </section>
        </div>
      ) : null}
```

- [ ] **Step 5: Run TypeScript build**

Run:

```bash
npm --prefix syncflow/packages/client run build
```

Expected: PASS.

- [ ] **Step 6: Commit dialogs**

```bash
git add syncflow/packages/client/src/app/Workbench.tsx
git commit -m "feat: add file delete and move dialogs"
```

---

### Task 7: Styling and Interaction Polish

**Files:**
- Modify: `syncflow/packages/client/src/styles/workbench.css`

- [ ] **Step 1: Add styles**

Append these styles near existing tree styles:

```css
.tree-rename-row {
  display: grid;
  grid-template-columns: 18px minmax(0, 1fr);
  gap: 6px;
  align-items: center;
  min-height: 30px;
}

.tree-rename-row input {
  min-width: 0;
  border: 1px solid rgba(80, 92, 120, 0.35);
  border-radius: 6px;
  padding: 4px 6px;
  font: inherit;
}

.tree-action-menu {
  position: absolute;
  right: 6px;
  top: 28px;
  z-index: 20;
  display: grid;
  min-width: 132px;
  padding: 4px;
  border: 1px solid rgba(80, 92, 120, 0.18);
  border-radius: 8px;
  background: #fff;
  box-shadow: 0 12px 30px rgba(20, 30, 55, 0.16);
}

.tree-action-menu button {
  border: 0;
  background: transparent;
  border-radius: 6px;
  padding: 7px 8px;
  text-align: left;
  font: inherit;
  cursor: pointer;
}

.tree-action-menu button:hover {
  background: rgba(40, 95, 160, 0.08);
}

.tree-action-menu button.danger,
.danger-button {
  color: #b42318;
}

.modal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 50;
  display: grid;
  place-items: center;
  padding: 20px;
  background: rgba(15, 23, 42, 0.28);
}

.file-manager-dialog {
  width: min(420px, 100%);
  border-radius: 10px;
  background: #fff;
  padding: 18px;
  box-shadow: 0 22px 60px rgba(15, 23, 42, 0.24);
}

.file-manager-dialog h2 {
  margin: 0 0 8px;
  font-size: 18px;
}

.file-manager-dialog p {
  margin: 0 0 12px;
  color: #475467;
}

.file-manager-dialog code {
  display: block;
  max-width: 100%;
  overflow-wrap: anywhere;
  border-radius: 6px;
  background: rgba(15, 23, 42, 0.06);
  padding: 8px;
}

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 16px;
}

.dialog-actions button {
  border: 1px solid rgba(80, 92, 120, 0.24);
  border-radius: 7px;
  background: #fff;
  padding: 7px 12px;
  font: inherit;
  cursor: pointer;
}

.dialog-actions button:hover {
  background: rgba(40, 95, 160, 0.08);
}

.move-target-field {
  display: grid;
  gap: 6px;
  margin-top: 12px;
  color: #344054;
  font-size: 13px;
}

.move-target-field select {
  min-width: 0;
  border: 1px solid rgba(80, 92, 120, 0.28);
  border-radius: 7px;
  padding: 7px 8px;
  font: inherit;
}
```

If duplicate selectors already exist, merge this CSS into the existing blocks instead of appending duplicate definitions.

- [ ] **Step 2: Ensure tree rows can anchor menus**

Find `.tree-row` in `workbench.css` and ensure it includes:

```css
  position: relative;
```

- [ ] **Step 3: Run frontend build**

Run:

```bash
npm --prefix syncflow/packages/client run build
```

Expected: PASS.

- [ ] **Step 4: Commit styling**

```bash
git add syncflow/packages/client/src/styles/workbench.css
git commit -m "feat: style file manager controls"
```

---

### Task 8: Full Verification

**Files:**
- No planned source edits unless verification reveals issues.

- [ ] **Step 1: Run Rust tests**

Run:

```bash
cargo test --workspace --manifest-path syncflow/Cargo.toml
```

Expected: PASS.

- [ ] **Step 2: Run frontend build**

Run:

```bash
npm --prefix syncflow/packages/client run build
```

Expected: PASS.

- [ ] **Step 3: Run Rust formatting**

Run:

```bash
cargo fmt --all --manifest-path syncflow/Cargo.toml
```

Expected: command completes with no errors. If it changes files, inspect and commit formatting with the relevant source changes.

- [ ] **Step 4: Optional manual app smoke test**

Run:

```bash
cd syncflow/packages/client && npx tauri dev
```

Manual checks:

- Create a file, then rename it.
- Delete a selected file and confirm preview clears.
- Create two folders and move a file between them.
- Refresh root and a directory after changing files outside the app.
- Use copy relative path.
- Use reveal in system file manager.

- [ ] **Step 5: Commit verification fixes if needed**

If verification required fixes:

```bash
git add <changed-files>
git commit -m "fix: stabilize local file manager"
```

If no fixes were needed, do not create an empty commit.

---

## Self-Review Checklist

- Spec coverage: covered rename, delete, move, refresh, copy path, reveal, backend safety, frontend state, refresh rules, error handling, and verification.
- Deferred scope remains deferred: drag-and-drop, multi-select, recycle bin, search, sync badges, and cloud differences are not included.
- Type consistency: backend request fields use camelCase over Tauri serde, frontend wrappers pass camelCase fields, and UI handlers operate on existing `TreeNode`.
- Red-flag scan: all implementation steps include exact files, commands, and code snippets.
