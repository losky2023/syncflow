# SyncFlow Codex-Style UI Refresh Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the approved Codex-style workbench refresh from `docs/superpowers/specs/2026-05-13-syncflow-codex-style-ui-design.md`.

**Architecture:** Keep existing React state and Tauri command behavior intact. Refactor only the visual/component shell: local icon components, quieter file tree actions, bottom repository status, two-zone workbench layout, and details as an inspector overlay.

**Tech Stack:** React 18, TypeScript, CSS, existing Tauri invoke wrappers. Use a local `Icons.tsx` module instead of adding a dependency.

---

## File Structure

- Create `syncflow/packages/client/src/components/ui/Icons.tsx`
  - Owns small `16px` stroke icons used by the refreshed UI.
  - Keeps visual consistency without adding `lucide-react`.

- Modify `syncflow/packages/client/src/components/sidebar/FileTree.tsx`
  - Replace text/symbol toolbar actions with icon buttons.
  - Keep all current callbacks and tree behavior.

- Modify `syncflow/packages/client/src/components/sidebar/FileTreeNode.tsx`
  - Replace square/text node symbols with folder/file icons.
  - Keep hover/selected row actions and more menu behavior.
  - Keep keyboard behavior for Enter, Escape, F2, Delete/Backspace.

- Modify `syncflow/packages/client/src/components/sidebar/SpaceList.tsx`
  - Keep the repository manager popover behavior.
  - Restyle trigger as the compact bottom sidebar status control.
  - Add concise synced/pending/conflict summary text.

- Modify `syncflow/packages/client/src/components/details/DetailsPane.tsx`
  - Add an optional close callback and inspector-friendly wrapper class.
  - Keep current file, location, conflict, and resolution content.

- Modify `syncflow/packages/client/src/app/Workbench.tsx`
  - Remove the heavy topbar from the visual flow.
  - Add compact brand/sidebar structure and floating path row.
  - Render details as an overlay inspector when `detailsOpen` is true.

- Modify `syncflow/packages/client/src/styles/workbench.css`
  - Add final Codex-style token overrides at the bottom of the file.
  - Override earlier theme layers without deleting unrelated existing CSS.
  - Reduce gradients, heavy shadows, large radii, and nested-card feel.

---

## Task 1: Local Icon Set

**Files:**
- Create: `syncflow/packages/client/src/components/ui/Icons.tsx`
- Verify: `npm --prefix syncflow/packages/client run build`

- [ ] **Step 1: Create the local icon module**

Create `syncflow/packages/client/src/components/ui/Icons.tsx` with this content:

```tsx
import type { SVGProps } from "react";

type IconProps = SVGProps<SVGSVGElement> & {
  size?: number;
};

function IconBase({ size = 16, children, ...props }: IconProps) {
  return (
    <svg
      aria-hidden="true"
      focusable="false"
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.8}
      strokeLinecap="round"
      strokeLinejoin="round"
      {...props}
    >
      {children}
    </svg>
  );
}

export function ChevronRightIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="m9 18 6-6-6-6" />
    </IconBase>
  );
}

export function ChevronDownIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="m6 9 6 6 6-6" />
    </IconBase>
  );
}

export function FolderIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M3 7.5A2.5 2.5 0 0 1 5.5 5H9l2 2h7.5A2.5 2.5 0 0 1 21 9.5v7A2.5 2.5 0 0 1 18.5 19h-13A2.5 2.5 0 0 1 3 16.5z" />
    </IconBase>
  );
}

export function FolderOpenIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M3 8.5A2.5 2.5 0 0 1 5.5 6H9l2 2h7.5A2.5 2.5 0 0 1 21 10.5" />
      <path d="m3.5 10.5 2 7A2 2 0 0 0 7.4 19h10.4a2 2 0 0 0 1.9-1.4l1.8-6.1A1.2 1.2 0 0 0 20.4 10H4.6a1.2 1.2 0 0 0-1.1 1.5Z" />
    </IconBase>
  );
}

export function FileIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M6 3.5h7l5 5V19a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 6 19z" />
      <path d="M13 3.5V9h5" />
    </IconBase>
  );
}

export function FileTextIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M6 3.5h7l5 5V19a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 6 19z" />
      <path d="M13 3.5V9h5" />
      <path d="M9 13h6" />
      <path d="M9 16h4" />
    </IconBase>
  );
}

export function ImageIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <rect x="4" y="5" width="16" height="14" rx="2" />
      <path d="m7 16 3.5-3.5 2.5 2.5 2-2 2 3" />
      <circle cx="9" cy="9" r="1" />
    </IconBase>
  );
}

export function MoreHorizontalIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <circle cx="6" cy="12" r="1" />
      <circle cx="12" cy="12" r="1" />
      <circle cx="18" cy="12" r="1" />
    </IconBase>
  );
}

export function PlusIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M12 5v14" />
      <path d="M5 12h14" />
    </IconBase>
  );
}

export function RefreshIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M20 12a8 8 0 0 1-13.7 5.7" />
      <path d="M4 12A8 8 0 0 1 17.7 6.3" />
      <path d="M17.7 3.5v2.8H15" />
      <path d="M6.3 20.5v-2.8H9" />
    </IconBase>
  );
}

export function ExternalLinkIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M14 4h6v6" />
      <path d="m10 14 10-10" />
      <path d="M20 14v4a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h4" />
    </IconBase>
  );
}

export function InfoIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <circle cx="12" cy="12" r="9" />
      <path d="M12 10v6" />
      <path d="M12 7.5h.01" />
    </IconBase>
  );
}

export function SettingsIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M12 8.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7Z" />
      <path d="M19.4 15a1.8 1.8 0 0 0 .36 2l.05.05a2 2 0 0 1-2.83 2.83l-.05-.05a1.8 1.8 0 0 0-2-.36 1.8 1.8 0 0 0-1.1 1.66V21a2 2 0 0 1-4 0v-.07a1.8 1.8 0 0 0-1.1-1.66 1.8 1.8 0 0 0-2 .36l-.05.05a2 2 0 1 1-2.83-2.83l.05-.05a1.8 1.8 0 0 0 .36-2 1.8 1.8 0 0 0-1.66-1.1H3a2 2 0 0 1 0-4h.07a1.8 1.8 0 0 0 1.66-1.1 1.8 1.8 0 0 0-.36-2l-.05-.05a2 2 0 0 1 2.83-2.83l.05.05a1.8 1.8 0 0 0 2 .36A1.8 1.8 0 0 0 10.3 3V3a2 2 0 0 1 4 0v.07a1.8 1.8 0 0 0 1.1 1.66 1.8 1.8 0 0 0 2-.36l.05-.05a2 2 0 0 1 2.83 2.83l-.05.05a1.8 1.8 0 0 0-.36 2 1.8 1.8 0 0 0 1.66 1.1H21a2 2 0 0 1 0 4h-.07A1.8 1.8 0 0 0 19.4 15Z" />
    </IconBase>
  );
}

export function CloseIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M18 6 6 18" />
      <path d="m6 6 12 12" />
    </IconBase>
  );
}
```

- [ ] **Step 2: Run the TypeScript build**

Run:

```bash
npm --prefix syncflow/packages/client run build
```

Expected: build succeeds, proving the new icon module compiles.

- [ ] **Step 3: Commit**

Run:

```bash
git add syncflow/packages/client/src/components/ui/Icons.tsx
git commit -m "feat: add workbench icon set"
```

Expected: commit succeeds if Git user identity is configured.

---

## Task 2: File Tree Header and Node Presentation

**Files:**
- Modify: `syncflow/packages/client/src/components/sidebar/FileTree.tsx`
- Modify: `syncflow/packages/client/src/components/sidebar/FileTreeNode.tsx`
- Verify: `npm --prefix syncflow/packages/client run build`

- [ ] **Step 1: Update `FileTree.tsx` imports**

Add icon imports after the existing imports:

```tsx
import {
  FileIcon,
  FolderIcon,
  MoreHorizontalIcon,
  PlusIcon,
  RefreshIcon,
} from "../ui/Icons";
```

- [ ] **Step 2: Replace the tree section header markup**

Replace the current `<div className="section-header compact-header tree-header-compact">...</div>` in `FileTree.tsx` with:

```tsx
<div className="files-head">
  <strong>Files</strong>
  <div className="files-actions">
    <button
      type="button"
      className="icon-button codex-icon-button"
      onClick={() => onRefreshPath(null)}
      title="刷新"
      aria-label="刷新文件树"
    >
      <RefreshIcon />
    </button>
    <button
      type="button"
      className="icon-button codex-icon-button"
      onClick={() => onStartCreate(null, "file")}
      title="新建文件"
      aria-label="新建文件"
    >
      <FileIcon />
    </button>
    <button
      type="button"
      className="icon-button codex-icon-button"
      onClick={() => onStartCreate(null, "folder")}
      title="新建文件夹"
      aria-label="新建文件夹"
    >
      <FolderIcon />
    </button>
    <button
      type="button"
      className="icon-button codex-icon-button"
      onClick={onImportDocument}
      title="导入文档为 Markdown"
      aria-label="导入文档为 Markdown"
    >
      <PlusIcon />
    </button>
    <button
      type="button"
      className="icon-button codex-icon-button codex-text-icon-button"
      onClick={onImportWeChatArticle}
      title="从剪贴板导入微信文章"
      aria-label="从剪贴板导入微信文章"
    >
      Wx
    </button>
  </div>
</div>
```

- [ ] **Step 3: Update `FileTreeNode.tsx` imports**

Add icon imports after the existing imports:

```tsx
import {
  ChevronDownIcon,
  ChevronRightIcon,
  ExternalLinkIcon,
  FileIcon,
  FileTextIcon,
  FolderIcon,
  FolderOpenIcon,
  ImageIcon,
  MoreHorizontalIcon,
  PlusIcon,
} from "../ui/Icons";
```

- [ ] **Step 4: Add a local node icon helper**

Add this helper inside `FileTreeNode.tsx`, above `export function FileTreeNode`:

```tsx
function TreeNodeIcon({
  node,
  isExpanded,
}: {
  node: TreeNode;
  isExpanded: boolean;
}) {
  if (node.nodeType === "directory") {
    return isExpanded ? <FolderOpenIcon /> : <FolderIcon />;
  }

  const extension = node.extension?.toLowerCase();
  if (extension === "md" || extension === "markdown" || extension === "txt") {
    return <FileTextIcon />;
  }
  if (extension && ["png", "jpg", "jpeg", "gif", "webp", "svg"].includes(extension)) {
    return <ImageIcon />;
  }
  return <FileIcon />;
}
```

- [ ] **Step 5: Replace tree toggle and node icon spans**

In `FileTreeNode.tsx`, replace the `tree-toggle` content block with:

```tsx
<span
  className={node.hasChildren ? "tree-toggle expandable" : "tree-toggle"}
  onClick={(event) => {
    event.stopPropagation();
    if (isDirectory && node.hasChildren) {
      onToggle(node);
    }
  }}
>
  {isDirectory && node.hasChildren ? (
    isExpanded ? <ChevronDownIcon /> : <ChevronRightIcon />
  ) : null}
</span>
<span className={isDirectory ? "tree-icon directory" : "tree-icon file"}>
  <TreeNodeIcon node={node} isExpanded={isExpanded} />
</span>
```

Also replace rename/create icon spans:

```tsx
<span className={isDirectory ? "tree-icon directory" : "tree-icon file"}>
  {isDirectory ? <FolderIcon /> : <FileIcon />}
</span>
```

and in `CreateInput`:

```tsx
<span className={kind === "folder" ? "tree-icon directory" : "tree-icon file"}>
  {kind === "folder" ? <FolderIcon /> : <FileIcon />}
</span>
```

- [ ] **Step 6: Replace row action symbols**

Replace row action contents with icons:

```tsx
<PlusIcon />
```

for create file/folder quick actions, and:

```tsx
<MoreHorizontalIcon />
```

for the more action. For reveal/open quick action, use:

```tsx
<ExternalLinkIcon />
```

Only keep quick create actions for directories; keep delete inside the menu.

- [ ] **Step 7: Run the build**

Run:

```bash
npm --prefix syncflow/packages/client run build
```

Expected: TypeScript build succeeds.

- [ ] **Step 8: Commit**

Run:

```bash
git add syncflow/packages/client/src/components/sidebar/FileTree.tsx syncflow/packages/client/src/components/sidebar/FileTreeNode.tsx
git commit -m "feat: refresh workbench file tree"
```

Expected: commit succeeds if Git user identity is configured.

---

## Task 3: Bottom Repository Status Control

**Files:**
- Modify: `syncflow/packages/client/src/components/sidebar/SpaceList.tsx`
- Verify: `npm --prefix syncflow/packages/client run build`

- [ ] **Step 1: Import icons**

Add this import:

```tsx
import { ChevronDownIcon, FolderIcon } from "../ui/Icons";
```

- [ ] **Step 2: Add compact summary helpers**

Add these helpers below `syncHealthLabel`:

```tsx
function repositoryProviderLabel(isCloudSpace: boolean) {
  return isCloudSpace ? "百度网盘" : "本地";
}

function repositorySummary(status?: SyncRuntimeStatus) {
  const fileCount = status?.fileCount ?? 0;
  const pendingCount = status?.pendingCount ?? 0;
  const conflictCount = status?.cloudConflictCount ?? 0;
  return `文件 ${fileCount} · 队列 ${pendingCount} · 冲突 ${conflictCount}`;
}
```

- [ ] **Step 3: Replace the trigger button content**

Replace the current closed trigger button content in `SpaceList.tsx` with:

```tsx
<button
  type="button"
  className="vault-trigger panel codex-vault-trigger"
  onClick={toggleMenu}
  aria-expanded={isOpen}
>
  <span className="vault-trigger-icon" aria-hidden="true">
    <FolderIcon />
  </span>
  <span className="vault-trigger-main">
    <strong>{selectedSpace?.name ?? "打开仓库"}</strong>
    <span>
      {selectedSpace
        ? `${repositoryProviderLabel(selectedIsCloudSpace)} · ${runtimeStatusLabel(selectedStatus)} · ${repositorySummary(selectedStatus)}`
        : "选择或添加同步文件夹"}
    </span>
  </span>
  <span className="vault-trigger-chevron" aria-hidden="true">
    <ChevronDownIcon />
  </span>
</button>
```

- [ ] **Step 4: Keep popover behavior unchanged**

Do not remove `vault-menu`, adding/importing repositories, or current repository actions. The visual refresh can restyle those existing classes in CSS.

- [ ] **Step 5: Run the build**

Run:

```bash
npm --prefix syncflow/packages/client run build
```

Expected: build succeeds.

- [ ] **Step 6: Commit**

Run:

```bash
git add syncflow/packages/client/src/components/sidebar/SpaceList.tsx
git commit -m "feat: compact repository status control"
```

Expected: commit succeeds if Git user identity is configured.

---

## Task 4: Workbench Two-Zone Layout and Details Inspector

**Files:**
- Modify: `syncflow/packages/client/src/app/Workbench.tsx`
- Modify: `syncflow/packages/client/src/components/details/DetailsPane.tsx`
- Verify: `npm --prefix syncflow/packages/client run build`

- [ ] **Step 1: Import shell icons in `Workbench.tsx`**

Add:

```tsx
import {
  CloseIcon,
  ExternalLinkIcon,
  InfoIcon,
  RefreshIcon,
  SettingsIcon,
} from "../components/ui/Icons";
```

- [ ] **Step 2: Add an optional close prop to `DetailsPane`**

In `DetailsPane.tsx`, update the props interface:

```tsx
  onClose?: () => void;
```

Destructure it:

```tsx
  onClose,
```

Replace the details header with:

```tsx
<div className="section-header details-header-compact inspector-header">
  <div>
    <h2>详情</h2>
    <p>{details?.spaceName ?? "选中文件或冲突后，在这里查看详细信息。"}</p>
  </div>
  {onClose ? (
    <button
      type="button"
      className="icon-button codex-icon-button"
      onClick={onClose}
      aria-label="关闭详情"
      title="关闭详情"
    >
      <CloseIcon />
    </button>
  ) : null}
</div>
```

Add this import:

```tsx
import { CloseIcon } from "../ui/Icons";
```

- [ ] **Step 3: Replace the outer shell in `Workbench.tsx`**

Keep all state and handler code. Replace the JSX starting at:

```tsx
return (
  <div className="workbench-shell">
```

through the opening of `<main className=...>` with:

```tsx
return (
  <div className="workbench-shell codex-workbench-shell">
    <main className={detailsOpen ? "workbench-grid details-open codex-workbench-grid" : "workbench-grid codex-workbench-grid"}>
      <aside className="left-column codex-sidebar">
        <div className="sidebar-brand">
          <span className="sidebar-brand-mark" aria-hidden="true" />
          <strong>SyncFlow</strong>
        </div>
```

Remove the old `<header className="panel workspace-topbar">...</header>` from the visible layout. Preserve the existing cloud settings and sync diagnostics drawers; move their trigger buttons into the floating path row in Step 4.

- [ ] **Step 4: Add the floating path row before the preview panel**

Immediately before:

```tsx
<section className="panel preview-panel">
```

insert:

```tsx
<div className="codex-main-zone">
  <div className="codex-path-row">
    <div className="codex-path-pill" title={activePreviewTab?.node.relativePath || selectedNode?.relativePath || selectedSpace?.rootPath || undefined}>
      {selectedSpace?.name ?? "未选择仓库"}
      {activePreviewTab?.node.relativePath || selectedNode?.relativePath
        ? ` / ${activePreviewTab?.node.relativePath || selectedNode?.relativePath}`
        : selectedSpace?.rootPath
          ? ` / ${selectedSpace.rootPath}`
          : ""}
    </div>
    <div className="codex-path-actions">
      <button type="button" className="icon-button codex-icon-button" onClick={() => selectedSpaceId && void refreshVisibleTree(selectedSpaceId)} aria-label="刷新文件树" title="刷新文件树">
        <RefreshIcon />
      </button>
      <button type="button" className="icon-button codex-icon-button" onClick={() => setSyncDiagnosticsOpen(true)} aria-label="同步诊断" title="同步诊断">
        <InfoIcon />
      </button>
      <button type="button" className="icon-button codex-icon-button" onClick={() => setBaiduConfigOpen(true)} aria-label="云同步设置" title="云同步设置">
        <SettingsIcon />
      </button>
    </div>
  </div>
```

Then close `codex-main-zone` immediately after the preview panel:

```tsx
</section>
</div>
```

- [ ] **Step 5: Replace the details toggle icon**

Inside the preview header, replace the existing SVG inside the details toggle button with:

```tsx
<InfoIcon />
```

Keep `aria-label`, `title`, `aria-pressed`, and `onClick`.

- [ ] **Step 6: Render details as an inspector overlay**

Replace the current details render:

```tsx
{detailsOpen ? (
  <DetailsPane ... />
) : null}
```

with:

```tsx
{detailsOpen ? (
  <div className="details-inspector-layer">
    <DetailsPane
      details={details}
      error={detailsError}
      conflicts={selectedSpaceConflicts}
      conflictError={conflictError}
      selectedConflictId={selectedConflictId}
      conflictDetail={conflictDetail}
      conflictDetailError={conflictDetailError}
      conflictActionError={conflictActionError}
      conflictActionLoading={conflictActionLoading}
      onSelectConflict={setSelectedConflictId}
      onResolveKeepLocal={(conflictId) => void handleResolveConflict("keep-local", conflictId)}
      onResolveKeepRemote={(conflictId) => void handleResolveConflict("keep-remote", conflictId)}
      onDismissConflict={(conflictId) => void handleResolveConflict("dismiss", conflictId)}
      onClose={() => setDetailsOpen(false)}
    />
  </div>
) : null}
```

- [ ] **Step 7: Run the build**

Run:

```bash
npm --prefix syncflow/packages/client run build
```

Expected: build succeeds.

- [ ] **Step 8: Commit**

Run:

```bash
git add syncflow/packages/client/src/app/Workbench.tsx syncflow/packages/client/src/components/details/DetailsPane.tsx
git commit -m "feat: add codex-style workbench shell"
```

Expected: commit succeeds if Git user identity is configured.

---

## Task 5: Codex-Style CSS Override Layer

**Files:**
- Modify: `syncflow/packages/client/src/styles/workbench.css`
- Verify: `npm --prefix syncflow/packages/client run build`

- [ ] **Step 1: Append the final token and layout override layer**

Append this block to the end of `workbench.css`:

```css
/* Codex-style workbench refresh */
:root {
  --cx-app: #f5f7fa;
  --cx-sidebar: #eaf0f7;
  --cx-sidebar-selected: #dfe6ee;
  --cx-panel: #ffffff;
  --cx-line: #e5e7eb;
  --cx-line-strong: #d5dbe3;
  --cx-text: #24292f;
  --cx-muted: #7b8490;
  --cx-hover: #e4ebf2;
  --cx-success: #16a34a;
  --cx-warning: #f97316;
}

body {
  background: var(--cx-app);
}

body::before {
  display: none;
}

.codex-workbench-shell {
  padding: 0;
  gap: 0;
  background: var(--cx-sidebar);
  grid-template-rows: minmax(0, 1fr);
  color: var(--cx-text);
}

.workspace-topbar {
  display: none;
}

.codex-workbench-grid,
.codex-workbench-grid.details-open {
  grid-template-columns: 292px minmax(0, 1fr);
  gap: 0;
  position: relative;
  background: var(--cx-sidebar);
  overflow: hidden;
}

.codex-sidebar {
  grid-template-rows: auto minmax(0, 1fr) auto;
  gap: 0;
  padding: 12px 0 12px 8px;
  background: var(--cx-sidebar);
}

.sidebar-brand {
  height: 30px;
  margin-right: 10px;
  padding: 0 8px;
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  font-weight: 700;
}

.sidebar-brand-mark {
  width: 20px;
  height: 20px;
  border-radius: 6px;
  background: var(--cx-text);
  position: relative;
}

.sidebar-brand-mark::after {
  content: "";
  position: absolute;
  inset: 5px;
  border: 1.5px solid #fff;
  border-radius: 4px;
}

.codex-main-zone {
  min-width: 0;
  min-height: 0;
  padding: 38px 8px 8px 0;
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
  position: relative;
}

.codex-path-row {
  position: absolute;
  inset: 8px 8px auto 0;
  height: 28px;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 10px;
  align-items: center;
}

.codex-path-pill {
  height: 28px;
  min-width: 0;
  border: 1px solid rgba(214, 219, 227, 0.78);
  border-radius: 9px;
  background: rgba(255, 255, 255, 0.62);
  color: var(--cx-muted);
  display: flex;
  align-items: center;
  padding: 0 10px;
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.codex-path-actions,
.files-actions {
  display: inline-flex;
  align-items: center;
  gap: 3px;
}

.codex-icon-button,
.tree-action-button,
.tree-row-action,
.details-toggle,
.icon-button {
  border: 1px solid transparent;
  border-radius: 7px;
  background: transparent;
  color: var(--cx-muted);
  box-shadow: none;
  transform: none;
}

.codex-icon-button:hover,
.tree-action-button:hover,
.tree-row-action:hover,
.details-toggle:hover,
.icon-button:hover {
  border-color: transparent;
  background: var(--cx-hover);
  color: var(--cx-text);
  transform: none;
}

.codex-text-icon-button {
  font-size: 11px;
  font-weight: 700;
}

.panel {
  border-color: var(--cx-line);
  border-radius: 14px;
  background: var(--cx-panel);
  box-shadow: none;
  backdrop-filter: none;
}

.tree-section {
  border: 0;
  border-radius: 0;
  background: transparent;
  box-shadow: none;
  padding: 0;
}

.files-head {
  height: 36px;
  margin-right: 10px;
  padding: 0 8px;
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.files-head strong {
  color: #5f6875;
  font-size: 13px;
  font-weight: 600;
}

.tree-list {
  padding: 2px 8px 8px 0;
  gap: 0;
}

.tree-row {
  min-height: 28px;
  border: 0;
  border-radius: 8px;
  color: #3d4652;
  font-size: 12px;
  padding-right: 8px;
}

.tree-row:hover {
  background: var(--cx-hover);
}

.tree-row.selected {
  background: var(--cx-sidebar-selected);
  border-color: transparent;
  color: var(--cx-text);
  box-shadow: none;
}

.tree-toggle {
  width: 14px;
  flex: 0 0 14px;
  display: inline-grid;
  place-items: center;
  color: var(--cx-muted);
}

.tree-icon {
  width: 16px;
  height: 16px;
  display: inline-grid;
  place-items: center;
  color: #738197;
}

.tree-name {
  font-size: 12px;
}

.tree-row-actions {
  opacity: 0;
  pointer-events: none;
}

.tree-row:hover .tree-row-actions,
.tree-row.selected .tree-row-actions,
.tree-row:focus-within .tree-row-actions {
  opacity: 1;
  pointer-events: auto;
}

.tree-row-action {
  width: 20px;
  height: 20px;
}

.tree-action-menu {
  border-color: var(--cx-line-strong);
  border-radius: 10px;
  box-shadow: 0 8px 28px rgba(15, 23, 42, 0.12);
}

.tree-action-menu button {
  min-height: 26px;
}

.tree-create-row,
.tree-rename-row {
  min-height: 28px;
}

.tree-create-row input,
.tree-rename-row input {
  height: 24px;
  border-color: #9aa7b8;
  border-radius: 6px;
}

.codex-vault-trigger {
  min-height: 54px;
  margin-right: 10px;
  border: 1px solid rgba(214, 219, 227, 0.78);
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.64);
  box-shadow: none;
}

.codex-vault-trigger:hover {
  border-color: rgba(214, 219, 227, 0.96);
  background: rgba(255, 255, 255, 0.78);
  box-shadow: none;
}

.vault-trigger-icon {
  border: 0;
  background: transparent;
  color: var(--cx-muted);
}

.vault-trigger-main strong {
  color: var(--cx-text);
  font-size: 12px;
}

.vault-trigger-main span {
  color: var(--cx-muted);
  font-size: 11px;
}

.preview-panel {
  min-height: 0;
  border-radius: 14px;
  overflow: hidden;
  background: var(--cx-panel);
  display: flex;
  flex-direction: column;
  padding: 0;
}

.preview-header {
  min-height: 48px;
  margin: 0;
  padding: 0 12px 0 16px;
  border-bottom: 1px solid var(--cx-line);
}

.preview-header h2 {
  font-size: 13px;
}

.preview-path {
  color: var(--cx-muted);
}

.preview-tabs {
  padding: 0 10px;
  min-height: 34px;
  align-items: end;
  background: #f8f9fb;
  border-bottom: 1px solid var(--cx-line);
}

.preview-tab {
  min-height: 29px;
  border-radius: 8px 8px 0 0;
  background: transparent;
}

.preview-tab.active {
  border-color: var(--cx-line);
  background: var(--cx-panel);
  box-shadow: none;
}

.preview-content {
  padding: 0;
}

.markdown-codemirror-editor,
.text-preview,
.image-preview {
  border: 0;
  border-radius: 0;
  background: var(--cx-panel);
}

.markdown-codemirror-editor .cm-content {
  padding: 42px clamp(40px, 7vw, 92px) 56px;
}

.bottom-bar,
.preview-statusbar {
  border-top: 1px solid var(--cx-line);
  background: #fbfcfe;
}

.details-inspector-layer {
  position: absolute;
  top: 38px;
  right: 8px;
  bottom: 8px;
  z-index: 30;
  width: min(320px, calc(100% - 320px));
  min-width: 280px;
  pointer-events: none;
}

.details-inspector-layer .details-panel {
  height: 100%;
  pointer-events: auto;
  border-radius: 12px 0 0 12px;
  border-color: var(--cx-line);
  border-right: 0;
  box-shadow: -8px 0 28px rgba(15, 23, 42, 0.08);
}

.inspector-header {
  align-items: flex-start;
}

.details-section-card,
.conflict-card,
.sync-task-row,
.sync-overview-card,
.cloud-settings-card {
  border-color: var(--cx-line);
  border-radius: 10px;
  background: var(--cx-panel);
}

@media (max-width: 1100px) {
  .codex-workbench-grid,
  .codex-workbench-grid.details-open {
    grid-template-columns: minmax(240px, 280px) minmax(0, 1fr);
  }
}

@media (max-width: 820px) {
  .codex-workbench-grid,
  .codex-workbench-grid.details-open {
    grid-template-columns: 1fr;
  }

  .codex-sidebar {
    min-height: 260px;
    padding-right: 8px;
  }

  .codex-main-zone {
    padding: 38px 8px 8px;
  }

  .details-inspector-layer {
    left: 16px;
    right: 16px;
    width: auto;
    min-width: 0;
  }
}
```

- [ ] **Step 2: Run the build**

Run:

```bash
npm --prefix syncflow/packages/client run build
```

Expected: build succeeds.

- [ ] **Step 3: Commit**

Run:

```bash
git add syncflow/packages/client/src/styles/workbench.css
git commit -m "feat: apply codex-style workbench theme"
```

Expected: commit succeeds if Git user identity is configured.

---

## Task 6: Manual Visual Verification and Final Polish

**Files:**
- Modify as needed:
  - `syncflow/packages/client/src/styles/workbench.css`
  - `syncflow/packages/client/src/app/Workbench.tsx`
  - `syncflow/packages/client/src/components/sidebar/FileTree.tsx`
  - `syncflow/packages/client/src/components/sidebar/FileTreeNode.tsx`
  - `syncflow/packages/client/src/components/sidebar/SpaceList.tsx`
  - `syncflow/packages/client/src/components/details/DetailsPane.tsx`
- Verify: `npm --prefix syncflow/packages/client run build`

- [ ] **Step 1: Start the frontend dev server**

Run:

```bash
npm --prefix syncflow/packages/client run dev -- --host 127.0.0.1 --port 1420
```

Expected: Vite serves the app at `http://127.0.0.1:1420/`.

- [ ] **Step 2: Open the app in the browser**

Open:

```text
http://127.0.0.1:1420/
```

Expected: the workbench visually follows the v7 mockup after any required app login/session state.

- [ ] **Step 3: Check default workbench**

Verify:

- Left sidebar is gray-blue and visually continuous with the shell.
- Main preview area is one white rounded container.
- Top path row is light and compact.
- Details inspector is hidden by default.
- Repository/sync status is at the bottom of the sidebar.
- File tree rows do not show action buttons until hover, focus, or selected state.

- [ ] **Step 4: Check file tree interactions**

Verify:

- Folder rows use folder icons.
- File rows use file/text/image icons.
- Hovering a row reveals quick actions.
- More menu contains rename, move, copy path, reveal, delete.
- Delete is styled as dangerous and is not visible by default.
- Rename/new item uses inline input.

- [ ] **Step 5: Check details inspector**

Click the details/info icon.

Verify:

- Inspector opens on the right.
- Main layout does not permanently switch to a third grid column.
- Close button hides the inspector.
- Conflict content remains accessible in the inspector.

- [ ] **Step 6: Check responsive widths**

Check desktop widths around:

```text
1280x800
1024x768
820x720
```

Expected:

- Text does not overlap.
- Buttons stay inside headers.
- File tree remains readable.
- Details inspector does not cover the entire app on desktop widths.

- [ ] **Step 7: Run final build**

Run:

```bash
npm --prefix syncflow/packages/client run build
```

Expected: build succeeds.

- [ ] **Step 8: Commit final polish**

If Step 3-6 required polish changes, commit them:

```bash
git add syncflow/packages/client/src
git commit -m "fix: polish codex-style workbench layout"
```

Expected: commit succeeds if Git user identity is configured.

---

## Self-Review Notes

- Spec coverage:
  - Two-zone layout: Tasks 4 and 5.
  - Codex-style tokens, radius, borders, shadows: Task 5.
  - File tree icons and actions: Task 2.
  - Bottom repository status: Task 3.
  - Details hidden by default and inspector: Task 4.
  - Accessibility labels and tooltips: Tasks 2, 3, 4.
  - Verification: Task 6.

- Placeholder scan:
  - No `TODO` or `TBD` steps are intentionally left.
  - Manual polish is constrained to the exact files and visual checks in Task 6.

- Type consistency:
  - New icon imports come from `../ui/Icons` for sidebar/details components and `../components/ui/Icons` for `Workbench.tsx`.
  - `DetailsPane` receives only one new optional prop: `onClose?: () => void`.
  - Existing tree and repository callbacks remain unchanged.
