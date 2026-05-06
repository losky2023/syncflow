import type { FileDetails } from "../../types/workbench";

interface DetailsPaneProps {
  details: FileDetails | null;
  error: string | null;
}

export function DetailsPane({ details, error }: DetailsPaneProps) {
  if (error) {
    return <div className="error-banner">{error}</div>;
  }

  if (!details) {
    return <div className="empty-card">未选中节点。</div>;
  }

  return (
    <div className="panel details-panel">
      <div className="section-header">
        <div>
          <h2>详情</h2>
          <p>{details.spaceName}</p>
        </div>
      </div>
      <dl className="details-grid">
        <dt>名称</dt>
        <dd>{details.name}</dd>
        <dt>类型</dt>
        <dd>{details.nodeType === "directory" ? "文件夹" : "文件"}</dd>
        <dt>扩展名</dt>
        <dd>{details.extension ?? "-"}</dd>
        <dt>大小</dt>
        <dd>{details.size} bytes</dd>
        <dt>修改时间</dt>
        <dd>{details.modifiedAt ?? "-"}</dd>
        <dt>相对路径</dt>
        <dd className="path-value">{details.relativePath}</dd>
      </dl>
    </div>
  );
}
