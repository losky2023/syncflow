import type { PreviewState } from "../../types/workbench";
import { FileFallbackCard } from "./FileFallbackCard";
import { ImagePreview } from "./ImagePreview";
import { TextPreview } from "./TextPreview";

interface PreviewPaneProps {
  preview: PreviewState;
  onOpenFallback: (relativePath: string) => void;
}

export function PreviewPane({ preview, onOpenFallback }: PreviewPaneProps) {
  if (preview.type === "welcome") {
    return <div className="empty-card large">选择左侧同步空间中的文件或文件夹开始浏览。</div>;
  }

  if (preview.type === "loading") {
    return <div className="empty-card large">正在加载 {preview.node.name} 的预览...</div>;
  }

  if (preview.type === "directory") {
    return <div className="empty-card large">已选中文件夹“{preview.node.name}”，请在右侧查看详情。</div>;
  }

  if (preview.type === "error") {
    return <div className="error-banner large">{preview.message}</div>;
  }

  if (preview.type === "text") {
    return <TextPreview result={preview.result} />;
  }

  if (preview.type === "image") {
    return <ImagePreview result={preview.result} />;
  }

  return (
    <FileFallbackCard
      node={preview.node}
      reason={preview.reason}
      onOpen={() => onOpenFallback(preview.node.relativePath)}
    />
  );
}
