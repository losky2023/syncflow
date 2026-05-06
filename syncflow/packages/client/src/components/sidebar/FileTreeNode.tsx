import type { TreeNode } from "../../types/workbench";

interface FileTreeNodeProps {
  node: TreeNode;
  depth: number;
  selectedPath: string | null;
  expandedPaths: Set<string>;
  childrenByPath: Record<string, TreeNode[]>;
  treeLoadingByPath: Record<string, boolean>;
  treeErrorByPath: Record<string, string | null>;
  onToggle: (node: TreeNode) => void;
  onSelect: (node: TreeNode) => void;
}

export function FileTreeNode({
  node,
  depth,
  selectedPath,
  expandedPaths,
  childrenByPath,
  treeLoadingByPath,
  treeErrorByPath,
  onToggle,
  onSelect,
}: FileTreeNodeProps) {
  const isDirectory = node.nodeType === "directory";
  const isExpanded = expandedPaths.has(node.relativePath);
  const isSelected = selectedPath === node.relativePath;
  const children = childrenByPath[node.relativePath] ?? [];
  const loading = treeLoadingByPath[node.relativePath] ?? false;
  const error = treeErrorByPath[node.relativePath] ?? null;

  return (
    <div>
      <button
        className={isSelected ? "tree-row selected" : "tree-row"}
        style={{ paddingLeft: `${12 + depth * 16}px` }}
        onClick={() => onSelect(node)}
      >
        <span className="tree-toggle" onClick={(event) => {
          event.stopPropagation();
          if (isDirectory && node.hasChildren) {
            onToggle(node);
          }
        }}>
          {isDirectory ? (node.hasChildren ? (isExpanded ? "▾" : "▸") : "·") : ""}
        </span>
        <span className="tree-icon">{isDirectory ? "📁" : "📄"}</span>
        <span className="tree-name">{node.name}</span>
      </button>

      {isDirectory && isExpanded ? (
        <div>
          {loading ? <div className="tree-meta">加载中...</div> : null}
          {error ? <div className="tree-error">{error}</div> : null}
          {!loading && !error && children.length === 0 ? (
            <div className="tree-meta">空文件夹</div>
          ) : null}
          {children.map((child) => (
            <FileTreeNode
              key={child.relativePath}
              node={child}
              depth={depth + 1}
              selectedPath={selectedPath}
              expandedPaths={expandedPaths}
              childrenByPath={childrenByPath}
              treeLoadingByPath={treeLoadingByPath}
              treeErrorByPath={treeErrorByPath}
              onToggle={onToggle}
              onSelect={onSelect}
            />
          ))}
        </div>
      ) : null}
    </div>
  );
}
