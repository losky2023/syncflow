import { useRef } from "react";
import type { TextPreviewResult } from "../../types/workbench";

interface TextPreviewProps {
  result: TextPreviewResult;
}

export function TextPreview({ result }: TextPreviewProps) {
  const contentRef = useRef<HTMLPreElement | null>(null);

  function selectPreviewContent() {
    const content = contentRef.current;
    if (!content) return;
    const range = document.createRange();
    range.selectNodeContents(content);
    const selection = window.getSelection();
    selection?.removeAllRanges();
    selection?.addRange(range);
  }

  return (
    <div
      className="preview-scroll"
      tabIndex={0}
      onKeyDown={(event) => {
        if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "a") {
          event.preventDefault();
          selectPreviewContent();
        }
      }}
    >
      <div className="preview-meta">
        <span>{result.size} bytes</span>
        {result.truncated ? <span>已截断到 {result.maxBytes} bytes</span> : null}
      </div>
      <pre ref={contentRef} className="text-preview">{result.content}</pre>
    </div>
  );
}
