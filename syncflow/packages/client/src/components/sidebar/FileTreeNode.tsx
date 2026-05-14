import type { TreeNode } from "../../types/workbench";
import {
  ChevronDownIcon,
  ChevronRightIcon,
  FileIcon,
  FileTextIcon,
  FolderIcon,
  FolderOpenIcon,
  ImageIcon,
  MoreHorizontalIcon,
} from "../ui/Icons";
import { CreateInput, type TreeCreateDraft } from "./FileTree";

interface FileTreeNodeProps {
  node: TreeNode;
  depth: number;
  selectedPath: string | null;
  createDraft: TreeCreateDraft | null;
  createName: string;
  createError: string | null;
  creating: boolean;
  expandedPaths: Set<string>;
  childrenByPath: Record<string, TreeNode[]>;
  treeLoadingByPath: Record<string, boolean>;
  treeErrorByPath: Record<string, string | null>;
  actionMenuPath: string | null;
  renameDraft: TreeNode | null;
  renameName: string;
  renameError: string | null;
  mutationLoading: boolean;
  onToggle: (node: TreeNode) => void;
  onSelect: (node: TreeNode) => void;
  onStartCreate: (parentRelativePath: string | null | undefined, kind: "file" | "folder") => void;
  onCreateNameChange: (value: string) => void;
  onCommitCreate: () => void;
  onCancelCreate: () => void;
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
}

function getExtension(name: string) {
  const lastDotIndex = name.lastIndexOf(".");
  return lastDotIndex >= 0 ? name.slice(lastDotIndex + 1).toLowerCase() : "";
}

function TreeNodeIcon({ node, expanded }: { node: TreeNode; expanded: boolean }) {
  if (node.nodeType === "directory") {
    return expanded ? (
      <FolderOpenIcon className="tree-icon directory" />
    ) : (
      <FolderIcon className="tree-icon directory" />
    );
  }

  const extension = getExtension(node.name);
  if (["md", "markdown", "txt"].includes(extension)) {
    return <FileTextIcon className="tree-icon file" />;
  }
  if (["png", "jpg", "jpeg", "gif", "webp", "svg"].includes(extension)) {
    return <ImageIcon className="tree-icon file" />;
  }
  return <FileIcon className="tree-icon file" />;
}

export function FileTreeNode({
  node,
  depth,
  selectedPath,
  createDraft,
  createName,
  createError,
  creating,
  expandedPaths,
  childrenByPath,
  treeLoadingByPath,
  treeErrorByPath,
  actionMenuPath,
  renameDraft,
  renameName,
  renameError,
  mutationLoading,
  onToggle,
  onSelect,
  onStartCreate,
  onCreateNameChange,
  onCommitCreate,
  onCancelCreate,
  onActionMenuChange,
  onStartRename,
  onRenameNameChange,
  onCommitRename,
  onCancelRename,
  onRequestDelete,
  onStartMove,
  onCopyRelativePath,
  onReveal,
  onRefreshPath,
}: FileTreeNodeProps) {
  const isDirectory = node.nodeType === "directory";
  const isExpanded = expandedPaths.has(node.relativePath);
  const isSelected = selectedPath === node.relativePath;
  const children = childrenByPath[node.relativePath] ?? [];
  const loading = treeLoadingByPath[node.relativePath] ?? false;
  const error = treeErrorByPath[node.relativePath] ?? null;
  const childDraft = createDraft?.parentRelativePath === node.relativePath ? createDraft : null;
  const isRenaming = renameDraft?.relativePath === node.relativePath;

  function handleRowClick() {
    onSelect(node);
    if (isDirectory && node.hasChildren && !isExpanded) {
      onToggle(node);
    }
  }

  function startCreate(kind: "file" | "folder") {
    if (!isDirectory) return;
    if (!isExpanded) {
      onToggle(node);
    }
    onStartCreate(node.relativePath, kind);
  }

  return (
    <div className="tree-node">
      {isRenaming ? (
        <div className="tree-rename-row" style={{ paddingLeft: `${10 + depth * 12}px` }}>
          <TreeNodeIcon node={node} expanded={isExpanded} />
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
      ) : (
        <div
          role="button"
          tabIndex={0}
          className={isSelected ? "tree-row selected" : "tree-row"}
          style={{ paddingLeft: `${10 + depth * 12}px` }}
          onClick={handleRowClick}
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
          title={node.relativePath || node.name}
        >
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
              isExpanded ? (
                <ChevronDownIcon className="tree-action-icon" />
              ) : (
                <ChevronRightIcon className="tree-action-icon" />
              )
            ) : null}
          </span>
          <TreeNodeIcon node={node} expanded={isExpanded} />
          <span className="tree-name">{node.name}</span>
          <span className="tree-row-actions">
            {isDirectory ? (
              <>
                <span
                  role="button"
                  tabIndex={0}
                  className="tree-row-action tree-row-action-secondary"
                  title="新建文件"
                  onClick={(event) => {
                    event.stopPropagation();
                    startCreate("file");
                  }}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") {
                      event.stopPropagation();
                      startCreate("file");
                    }
                  }}
                >
                  <FileIcon className="tree-action-icon" />
                </span>
                <span
                  role="button"
                  tabIndex={0}
                  className="tree-row-action tree-row-action-secondary"
                  title="新建文件夹"
                  onClick={(event) => {
                    event.stopPropagation();
                    startCreate("folder");
                  }}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") {
                      event.stopPropagation();
                      startCreate("folder");
                    }
                  }}
                >
                  <FolderIcon className="tree-action-icon" />
                </span>
              </>
            ) : null}
            <span
              role="button"
              tabIndex={0}
              className="tree-row-action tree-row-action-primary"
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
              <MoreHorizontalIcon className="tree-action-icon" />
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
          </span>
        </div>
      )}

      {isDirectory && isExpanded ? (
        <div className="tree-children">
          {childDraft ? (
            <CreateInput
              depth={depth + 1}
              kind={childDraft.kind}
              value={createName}
              error={createError}
              creating={creating}
              onChange={onCreateNameChange}
              onCommit={onCommitCreate}
              onCancel={onCancelCreate}
            />
          ) : null}
          {loading ? <div className="tree-meta">加载中...</div> : null}
          {error ? <div className="tree-error">{error}</div> : null}
          {!loading && !error && children.length === 0 && !childDraft ? <div className="tree-meta">空文件夹</div> : null}
          {children.map((child) => (
            <FileTreeNode
              key={child.relativePath}
              node={child}
              depth={depth + 1}
              selectedPath={selectedPath}
              createDraft={createDraft}
              createName={createName}
              createError={createError}
              creating={creating}
              expandedPaths={expandedPaths}
              childrenByPath={childrenByPath}
              treeLoadingByPath={treeLoadingByPath}
              treeErrorByPath={treeErrorByPath}
              actionMenuPath={actionMenuPath}
              renameDraft={renameDraft}
              renameName={renameName}
              renameError={renameError}
              mutationLoading={mutationLoading}
              onToggle={onToggle}
              onSelect={onSelect}
              onStartCreate={onStartCreate}
              onCreateNameChange={onCreateNameChange}
              onCommitCreate={onCommitCreate}
              onCancelCreate={onCancelCreate}
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
            />
          ))}
        </div>
      ) : null}
    </div>
  );
}
