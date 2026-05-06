import type { ImagePreviewResult } from "../../types/workbench";

interface ImagePreviewProps {
  result: ImagePreviewResult;
}

export function ImagePreview({ result }: ImagePreviewProps) {
  return (
    <div className="image-preview-wrap">
      <div className="preview-meta">
        <span>{result.mimeType}</span>
        <span>{result.size} bytes</span>
      </div>
      <img className="image-preview" src={result.dataUrl} alt="预览图片" />
    </div>
  );
}
