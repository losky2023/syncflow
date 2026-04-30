import type { TreeNode } from "../../types/workbench";
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
  onToggle: (node: TreeNode) => void;
  onSelect: (node: TreeNode) => void;
  onStartCreate: (parentRelativePath: string | null, kind: "file" | "folder") => void;
  onCreateNameChange: (value: string) => void;
  onCommitCreate: () => void;
  onCancelCreate: () => void;
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
  onToggle,
  onSelect,
  onStartCreate,
  onCreateNameChange,
  onCommitCreate,
  onCancelCreate,
}: FileTreeNodeProps) {
  const isDirectory = node.nodeType === "directory";
  const isExpanded = expandedPaths.has(node.relativePath);
  const isSelected = selectedPath === node.relativePath;
  const children = childrenByPath[node.relativePath] ?? [];
  const loading = treeLoadingByPath[node.relativePath] ?? false;
  const error = treeErrorByPath[node.relativePath] ?? null;
  const childDraft = createDraft?.parentRelativePath === node.relativePath ? createDraft : null;

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
      <button
        type="button"
        className={isSelected ? "tree-row selected" : "tree-row"}
        style={{ paddingLeft: `${10 + depth * 12}px` }}
        onClick={handleRowClick}
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
          {isDirectory ? (node.hasChildren ? (isExpanded ? "▾" : "▸") : "•") : ""}
        </span>
        <span className={isDirectory ? "tree-icon directory" : "tree-icon file"} />
        <span className="tree-name">{node.name}</span>
        {isDirectory ? (
          <span className="tree-row-actions">
            <span
              role="button"
              tabIndex={0}
              className="tree-row-action"
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
              +
            </span>
            <span
              role="button"
              tabIndex={0}
              className="tree-row-action"
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
              ▣
            </span>
          </span>
        ) : null}
      </button>

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
              onToggle={onToggle}
              onSelect={onSelect}
              onStartCreate={onStartCreate}
              onCreateNameChange={onCreateNameChange}
              onCommitCreate={onCommitCreate}
              onCancelCreate={onCancelCreate}
            />
          ))}
        </div>
      ) : null}
    </div>
  );
}
