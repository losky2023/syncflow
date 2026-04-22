import type { TextPreviewResult } from "../../types/workbench";

interface TextPreviewProps {
  result: TextPreviewResult;
}

export function TextPreview({ result }: TextPreviewProps) {
  return (
    <div className="preview-scroll">
      <div className="preview-meta">
        <span>{result.size} bytes</span>
        {result.truncated ? <span>已截断到 {result.maxBytes} bytes</span> : null}
      </div>
      <pre className="text-preview">{result.content}</pre>
    </div>
  );
}
