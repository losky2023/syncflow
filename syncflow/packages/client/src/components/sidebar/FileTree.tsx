import type { TreeNode } from "../../types/workbench";
import { FileTreeNode } from "./FileTreeNode";

interface FileTreeProps {
  roots: TreeNode[];
  selectedPath: string | null;
  expandedPaths: Set<string>;
  childrenByPath: Record<string, TreeNode[]>;
  treeLoadingByPath: Record<string, boolean>;
  treeErrorByPath: Record<string, string | null>;
  rootLoading: boolean;
  rootError: string | null;
  onToggle: (node: TreeNode) => void;
  onSelect: (node: TreeNode) => void;
}

export function FileTree({
  roots,
  selectedPath,
  expandedPaths,
  childrenByPath,
  treeLoadingByPath,
  treeErrorByPath,
  rootLoading,
  rootError,
  onToggle,
  onSelect,
}: FileTreeProps) {
  return (
    <section className="panel tree-section">
      <div className="section-header">
        <div>
          <h2>文件树</h2>
          <p>按需展开目录，不会一次性预取整棵树。</p>
        </div>
      </div>

      {rootLoading ? <div className="empty-card">正在加载根目录...</div> : null}
      {rootError ? <div className="error-banner">{rootError}</div> : null}
      {!rootLoading && !rootError && roots.length === 0 ? (
        <div className="empty-card">当前空间为空。</div>
      ) : null}

      <div className="tree-list">
        {roots.map((node) => (
          <FileTreeNode
            key={node.relativePath}
            node={node}
            depth={0}
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
    </section>
  );
}
