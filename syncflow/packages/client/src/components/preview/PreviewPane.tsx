import type { PreviewState } from "../../types/workbench";
import { FileFallbackCard } from "./FileFallbackCard";
import { ImagePreview } from "./ImagePreview";
import { MarkdownEditor } from "./MarkdownEditor";
import { TextPreview } from "./TextPreview";

interface PreviewPaneProps {
  preview: PreviewState;
  onOpenFallback: (relativePath: string) => void;
  onSaveMarkdown: (relativePath: string, content: string) => Promise<void>;
  markdownSaveState: { relativePath: string; isSaving: boolean; error: string | null } | null;
  onMarkdownStateChange?: (state: { content: string; isDirty: boolean; wordCount: number } | null) => void;
}

export function PreviewPane({
  preview,
  onOpenFallback,
  onSaveMarkdown,
  markdownSaveState,
  onMarkdownStateChange,
}: PreviewPaneProps) {
  if (preview.type === "welcome") {
    return (
      <div className="preview-empty-state preview-empty-state-welcome">
        <div className="preview-empty-mark" />
        <strong>选择一个文件开始预览</strong>
        <span>从左侧文件树选择文件或文件夹，内容会显示在这里。</span>
      </div>
    );
  }

  if (preview.type === "loading") {
    return (
      <div className="preview-empty-state">
        <div className="preview-empty-mark loading" />
        <strong>正在加载预览</strong>
        <span>{preview.node.name}</span>
      </div>
    );
  }

  if (preview.type === "directory") {
    return (
      <div className="preview-empty-state preview-empty-state-directory">
        <div className="preview-empty-mark" />
        <strong>已选中文件夹</strong>
        <span>“{preview.node.name}” 的元数据可在详情栏查看。</span>
      </div>
    );
  }

  if (preview.type === "error") {
    return <div className="error-banner large">{preview.message}</div>;
  }

  if (preview.type === "text") {
    return <TextPreview result={preview.result} />;
  }

  if (preview.type === "markdown") {
    return (
      <MarkdownEditor
        node={preview.node}
        result={preview.result}
        isSaving={
          markdownSaveState?.relativePath === preview.node.relativePath
            ? markdownSaveState.isSaving
            : false
        }
        saveError={
          markdownSaveState?.relativePath === preview.node.relativePath
            ? markdownSaveState.error
            : null
        }
        onSave={(content) => onSaveMarkdown(preview.node.relativePath, content)}
        onStateChange={onMarkdownStateChange}
      />
    );
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
