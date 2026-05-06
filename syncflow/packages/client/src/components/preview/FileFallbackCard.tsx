import type { TreeNode } from "../../types/workbench";

interface FileFallbackCardProps {
  node: TreeNode;
  reason?: string;
  onOpen: () => void;
}

export function FileFallbackCard({ node, reason, onOpen }: FileFallbackCardProps) {
  return (
    <div className="fallback-card">
      <h3>{node.name}</h3>
      <p>{reason ?? "当前文件暂不支持内置预览。"}</p>
      <button className="primary-button" onClick={onOpen}>
        用系统打开
      </button>
    </div>
  );
}
