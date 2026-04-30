import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { TextPreviewResult, TreeNode } from "../../types/workbench";

interface MarkdownEditorProps {
  node: TreeNode;
  result: TextPreviewResult;
  isSaving: boolean;
  saveError: string | null;
  onSave: (content: string) => Promise<void>;
  onStateChange?: (state: { content: string; isDirty: boolean; wordCount: number }) => void;
}

type InlineSegment = {
  kind: "text" | "bold" | "italic" | "code" | "link";
  raw: string;
  display: string;
  start: number;
  end: number;
  href?: string;
};

type ActiveEdit = {
  line: number;
  start: number;
  end: number;
};

type LineModel =
  | { kind: "blank"; contentStart: 0; content: "" }
  | { kind: "paragraph"; contentStart: 0; content: string }
  | { kind: "indented"; depth: number; contentStart: number; content: string }
  | { kind: "heading"; level: number; contentStart: number; content: string }
  | { kind: "unordered"; marker: string; contentStart: number; content: string }
  | { kind: "ordered"; marker: string; contentStart: number; content: string }
  | { kind: "code"; contentStart: 0; content: string };

function liveLineClassName(line: string, editing: boolean) {
  const classes = ["markdown-live-line"];
  if (editing) classes.push("active");
  const heading = line.match(/^(#{1,6})\s+/);
  if (heading) {
    classes.push("markdown-live-line-heading", `markdown-live-line-h${heading[1].length}`);
  } else if (/^\s*(?:[-*]|\d+\.)\s+/.test(line)) {
    classes.push("markdown-live-line-list");
  } else if (line.trim().startsWith("```")) {
    classes.push("markdown-live-line-code");
  } else if (/^\s{2,}/.test(line)) {
    classes.push("markdown-live-line-indented");
  } else if (!line.trim()) {
    classes.push("markdown-live-line-blank");
  } else {
    classes.push("markdown-live-line-paragraph");
  }
  return classes.join(" ");
}

function countWords(content: string) {
  const cjk = content.match(/[\u4e00-\u9fff]/g)?.length ?? 0;
  const words = content
    .replace(/[\u4e00-\u9fff]/g, " ")
    .trim()
    .split(/\s+/)
    .filter(Boolean).length;
  return cjk + words;
}

export function countMarkdownWords(content: string) {
  return countWords(content);
}

function parseLineModel(line: string): LineModel {
  if (!line.trim()) {
    return { kind: "blank", contentStart: 0, content: "" };
  }

  const heading = line.match(/^(#{1,6})(\s+)(.*)$/);
  if (heading) {
    return {
      kind: "heading",
      level: heading[1].length,
      contentStart: heading[1].length + heading[2].length,
      content: heading[3],
    };
  }

  const listItem = line.match(/^(\s*)((?:[-*])|(?:\d+\.))(\s+)(.*)$/);
  if (listItem) {
    const marker = listItem[2];
    return {
      kind: /^\d+\.$/.test(marker) ? "ordered" : "unordered",
      marker: marker === "*" || marker === "-" ? "•" : marker,
      contentStart: listItem[1].length + listItem[2].length + listItem[3].length,
      content: listItem[4],
    };
  }

  if (line.trim().startsWith("```")) {
    return { kind: "code", contentStart: 0, content: line };
  }

  const indented = line.match(/^(\s{2,})(.*)$/);
  if (indented) {
    return {
      kind: "indented",
      depth: Math.max(1, Math.floor(indented[1].length / 2)),
      contentStart: indented[1].length,
      content: indented[2],
    };
  }

  return { kind: "paragraph", contentStart: 0, content: line };
}

function nextMarkdownTokenStart(value: string, from: number) {
  const candidates = ["**", "*", "`", "["]
    .map((token) => value.indexOf(token, from))
    .filter((index) => index >= 0);
  return candidates.length > 0 ? Math.min(...candidates) : value.length;
}

function parseInlineSegments(value: string, baseStart: number): InlineSegment[] {
  if (!value) return [];

  const segments: InlineSegment[] = [];
  let index = 0;

  const pushText = (end: number) => {
    if (end <= index) return;
    const raw = value.slice(index, end);
    segments.push({
      kind: "text",
      raw,
      display: raw,
      start: baseStart + index,
      end: baseStart + end,
    });
    index = end;
  };

  while (index < value.length) {
    if (value.startsWith("**", index)) {
      const close = value.indexOf("**", index + 2);
      if (close > index + 2) {
        const raw = value.slice(index, close + 2);
        segments.push({
          kind: "bold",
          raw,
          display: value.slice(index + 2, close),
          start: baseStart + index,
          end: baseStart + close + 2,
        });
        index = close + 2;
        continue;
      }
    }

    if (value[index] === "`") {
      const close = value.indexOf("`", index + 1);
      if (close > index + 1) {
        const raw = value.slice(index, close + 1);
        segments.push({
          kind: "code",
          raw,
          display: value.slice(index + 1, close),
          start: baseStart + index,
          end: baseStart + close + 1,
        });
        index = close + 1;
        continue;
      }
    }

    if (value[index] === "[") {
      const labelEnd = value.indexOf("](", index + 1);
      const urlEnd = labelEnd >= 0 ? value.indexOf(")", labelEnd + 2) : -1;
      if (labelEnd > index + 1 && urlEnd > labelEnd + 2) {
        const raw = value.slice(index, urlEnd + 1);
        segments.push({
          kind: "link",
          raw,
          display: value.slice(index + 1, labelEnd),
          href: value.slice(labelEnd + 2, urlEnd),
          start: baseStart + index,
          end: baseStart + urlEnd + 1,
        });
        index = urlEnd + 1;
        continue;
      }
    }

    if (value[index] === "*" && !value.startsWith("**", index)) {
      const close = value.indexOf("*", index + 1);
      if (close > index + 1 && value[close + 1] !== "*") {
        const raw = value.slice(index, close + 1);
        segments.push({
          kind: "italic",
          raw,
          display: value.slice(index + 1, close),
          start: baseStart + index,
          end: baseStart + close + 1,
        });
        index = close + 1;
        continue;
      }
    }

    pushText(nextMarkdownTokenStart(value, index + 1));
  }

  return segments;
}

function defaultEditForLine(line: string, lineIndex: number): ActiveEdit {
  const model = parseLineModel(line);
  if (model.kind === "code") {
    return { line: lineIndex, start: 0, end: line.length };
  }
  if (model.kind === "blank") {
    return { line: lineIndex, start: 0, end: 0 };
  }
  return { line: lineIndex, start: 0, end: line.length };
}

export function MarkdownEditor({
  node,
  result,
  isSaving,
  saveError,
  onSave,
  onStateChange,
}: MarkdownEditorProps) {
  const [content, setContent] = useState(result.content);
  const [lastSavedContent, setLastSavedContent] = useState(result.content);
  const [activeEdit, setActiveEdit] = useState<ActiveEdit | null>(null);
  const isDirty = content !== lastSavedContent;
  const lines = useMemo(() => content.replace(/\r\n/g, "\n").split("\n"), [content]);
  const wordCount = useMemo(() => countWords(content), [content]);
  const editorRef = useRef<HTMLDivElement | null>(null);
  const activeEditableRef = useRef<HTMLElement | null>(null);
  const activeFocusKey = activeEdit ? `${activeEdit.line}:${activeEdit.start}` : null;
  const pendingCaretOffsetRef = useRef<number | null>(null);
  const saveTimerRef = useRef<number | null>(null);
  const editingSaveTimerRef = useRef<number | null>(null);
  const editingRef = useRef(false);
  const latestContentRef = useRef(content);
  const savedContentRef = useRef(lastSavedContent);
  const activePathRef = useRef(node.relativePath);
  const saveInFlightRef = useRef(false);
  const pendingSaveRef = useRef<string | null>(null);

  useEffect(() => {
    if (activePathRef.current === node.relativePath) {
      if (result.content !== latestContentRef.current) {
        latestContentRef.current = result.content;
        setContent(result.content);
      }
      savedContentRef.current = result.content;
      setLastSavedContent(result.content);
      return;
    }

    activePathRef.current = node.relativePath;
    latestContentRef.current = result.content;
    savedContentRef.current = result.content;
    setContent(result.content);
    setLastSavedContent(result.content);
    editingRef.current = false;
    setActiveEdit(null);
  }, [node.relativePath, result.content]);

  useEffect(() => {
    latestContentRef.current = content;
  }, [content]);

  useEffect(() => {
    savedContentRef.current = lastSavedContent;
  }, [lastSavedContent]);

  useEffect(() => {
    onStateChange?.({ content, isDirty, wordCount });
  }, [content, isDirty, onStateChange, wordCount]);

  useEffect(() => {
    return () => {
      if (saveTimerRef.current !== null) {
        window.clearTimeout(saveTimerRef.current);
      }
      if (editingSaveTimerRef.current !== null) {
        window.clearTimeout(editingSaveTimerRef.current);
      }
    };
  }, []);

  useEffect(() => {
    if (result.truncated || !isDirty) return;
    if (editingRef.current) return;
    if (saveTimerRef.current !== null) {
      window.clearTimeout(saveTimerRef.current);
    }
    saveTimerRef.current = window.setTimeout(() => {
      saveTimerRef.current = null;
      void saveContent(latestContentRef.current).catch(() => undefined);
    }, 1500);
  }, [content, isDirty, result.truncated]);

  useEffect(() => {
    if (activeFocusKey === null) return;
    window.requestAnimationFrame(() => {
      const editable = activeEditableRef.current;
      if (!editable) return;
      editable.focus();
      placeCaretAtEnd(editable);
    });
  }, [activeFocusKey]);

  useLayoutEffect(() => {
    const offset = pendingCaretOffsetRef.current;
    if (offset === null) return;
    pendingCaretOffsetRef.current = null;
    const editable = activeEditableRef.current;
    if (!editable) return;
    restoreCaretOffset(editable, offset);
  }, [content]);

  async function saveContent(nextContent = latestContentRef.current) {
    if (result.truncated || nextContent === savedContentRef.current) return;
    if (saveInFlightRef.current || isSaving) {
      pendingSaveRef.current = nextContent;
      return;
    }

    saveInFlightRef.current = true;
    try {
      await onSave(nextContent);
      savedContentRef.current = nextContent;
      setLastSavedContent(nextContent);
    } finally {
      saveInFlightRef.current = false;
    }

    const pending = pendingSaveRef.current;
    pendingSaveRef.current = null;
    if (pending && pending !== savedContentRef.current) {
      await saveContent(pending);
    }
  }

  function updateContent(nextContent: string) {
    latestContentRef.current = nextContent;
    setContent(nextContent);
  }

  function scheduleEditingSave() {
    if (result.truncated) return;
    if (editingSaveTimerRef.current !== null) {
      window.clearTimeout(editingSaveTimerRef.current);
    }
    editingSaveTimerRef.current = window.setTimeout(() => {
      editingSaveTimerRef.current = null;
      if (editingRef.current) {
        void saveContent(latestContentRef.current).catch(() => undefined);
      }
    }, 5000);
  }

  function placeCaretAtEnd(element: HTMLElement) {
    const range = document.createRange();
    range.selectNodeContents(element);
    range.collapse(false);
    const selection = window.getSelection();
    selection?.removeAllRanges();
    selection?.addRange(range);
  }

  function getCaretOffset(element: HTMLElement) {
    const selection = window.getSelection();
    if (!selection || selection.rangeCount === 0) return element.textContent?.length ?? 0;
    const range = selection.getRangeAt(0);
    if (!element.contains(range.endContainer)) return element.textContent?.length ?? 0;
    const beforeCaret = range.cloneRange();
    beforeCaret.selectNodeContents(element);
    beforeCaret.setEnd(range.endContainer, range.endOffset);
    return beforeCaret.toString().length;
  }

  function restoreCaretOffset(element: HTMLElement, offset: number) {
    const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT);
    let remaining = offset;
    let node = walker.nextNode();
    while (node) {
      const length = node.textContent?.length ?? 0;
      if (remaining <= length) {
        const range = document.createRange();
        range.setStart(node, remaining);
        range.collapse(true);
        const selection = window.getSelection();
        selection?.removeAllRanges();
        selection?.addRange(range);
        return;
      }
      remaining -= length;
      node = walker.nextNode();
    }
    placeCaretAtEnd(element);
  }

  function stripEditorPlaceholders(value: string) {
    return value.replace(/\u00a0/g, "").replace(/\u200b/g, "");
  }

  function replaceLineRange(lineIndex: number, start: number, end: number, value: string) {
    const nextLines = [...lines];
    const currentLine = nextLines[lineIndex] ?? "";
    const nextLine = `${currentLine.slice(0, start)}${value}${currentLine.slice(end)}`;
    nextLines.splice(lineIndex, 1, ...nextLine.replace(/\r\n/g, "\n").split("\n"));
    updateContent(nextLines.join("\n"));
    setActiveEdit((current) =>
      current && current.line === lineIndex && current.start === start
        ? { ...current, end: start + value.length }
        : current,
    );
  }

  function updateActiveEditText(edit: ActiveEdit, displayValue: string) {
    const value = stripEditorPlaceholders(displayValue);
    replaceLineRange(edit.line, edit.start, edit.end, value);
  }

  function splitActiveLine(event: React.KeyboardEvent<HTMLElement>, edit: ActiveEdit) {
    if (activeEdit === null) return;
    event.preventDefault();
    const text = stripEditorPlaceholders(event.currentTarget.textContent ?? "");
    const line = lines[edit.line] ?? "";
    const nextLines = [...lines];
    nextLines.splice(edit.line, 1, `${line.slice(0, edit.start)}${text}`, line.slice(edit.end));
    updateContent(nextLines.join("\n"));
    setActiveEdit({ line: edit.line + 1, start: 0, end: 0 });
  }

  function handleEditableKeyDown(event: React.KeyboardEvent<HTMLElement>, edit: ActiveEdit) {
    const displayValue = stripEditorPlaceholders(event.currentTarget.textContent ?? "");
    if ((event.key === "Backspace" || event.key === "Delete") && displayValue.length === 0) {
      event.preventDefault();
      deleteLine(edit.line);
      return;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      setActiveEdit(null);
      return;
    }
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s") {
      event.preventDefault();
      void saveContent(latestContentRef.current).catch(() => undefined);
      return;
    }
    if (event.key === "Enter") {
      splitActiveLine(event, edit);
      return;
    }
    if (event.key === "Tab") {
      event.preventDefault();
      document.execCommand("insertText", false, "  ");
    }
  }

  function deleteLine(lineIndex: number) {
    const nextLines = [...lines];
    if (nextLines.length <= 1) {
      updateContent("");
      setActiveEdit({ line: 0, start: 0, end: 0 });
      pendingCaretOffsetRef.current = 0;
      return;
    }

    nextLines.splice(lineIndex, 1);
    updateContent(nextLines.join("\n"));
    const nextLineIndex = Math.max(0, Math.min(lineIndex - 1, nextLines.length - 1));
    setActiveEdit(defaultEditForLine(nextLines[nextLineIndex] ?? "", nextLineIndex));
    pendingCaretOffsetRef.current = 0;
  }

  function editableProps(edit: ActiveEdit) {
    return {
      ref: (element: HTMLElement | null) => {
        activeEditableRef.current = element;
      },
      className: "markdown-live-fragment markdown-live-editing-fragment",
      contentEditable: !result.truncated,
      suppressContentEditableWarning: true,
      onInput: (event: React.FormEvent<HTMLElement>) => {
        editingRef.current = true;
        pendingCaretOffsetRef.current = getCaretOffset(event.currentTarget);
        updateActiveEditText(edit, event.currentTarget.textContent ?? "");
        scheduleEditingSave();
      },
      onBlur: () => {
        editingRef.current = false;
        if (editingSaveTimerRef.current !== null) {
          window.clearTimeout(editingSaveTimerRef.current);
          editingSaveTimerRef.current = null;
        }
        if (saveTimerRef.current !== null) {
          window.clearTimeout(saveTimerRef.current);
          saveTimerRef.current = null;
        }
        void saveContent(latestContentRef.current).catch(() => undefined);
      },
      onKeyDown: (event: React.KeyboardEvent<HTMLElement>) => handleEditableKeyDown(event, edit),
    };
  }

  function renderActiveFragment(line: string, edit: ActiveEdit) {
    const value = line.slice(edit.start, edit.end);
    return <span {...editableProps(edit)}>{value || "\u200b"}</span>;
  }

  function renderSegment(lineIndex: number, segment: InlineSegment, active: ActiveEdit | null) {
    const isActive =
      active?.line === lineIndex && active.start === segment.start && active.end === segment.end;
    if (isActive) {
      return <span key={`active-${segment.start}`}>{renderActiveFragment(lines[lineIndex] ?? "", active)}</span>;
    }

    const activate = (event: React.MouseEvent) => {
      if (result.truncated) return;
      event.preventDefault();
      event.stopPropagation();
      setActiveEdit({
        line: lineIndex,
        start: segment.start,
        end: segment.end,
      });
    };

    const props = {
      className: "markdown-live-fragment",
      onMouseDown: activate,
    };

    if (segment.kind === "bold") {
      return (
        <strong key={`${segment.start}-bold`} {...props}>
          {segment.display}
        </strong>
      );
    }
    if (segment.kind === "italic") {
      return (
        <em key={`${segment.start}-italic`} {...props}>
          {segment.display}
        </em>
      );
    }
    if (segment.kind === "code") {
      return (
        <code key={`${segment.start}-code`} {...props}>
          {segment.display}
        </code>
      );
    }
    if (segment.kind === "link") {
      return (
        <a key={`${segment.start}-link`} href={segment.href} {...props}>
          {segment.display}
        </a>
      );
    }
    return (
      <span key={`${segment.start}-text`} {...props}>
        {segment.display}
      </span>
    );
  }

  function renderSegments(lineIndex: number, line: string, content: string, contentStart: number) {
    const active = activeEdit?.line === lineIndex ? activeEdit : null;
    if (active && active.start >= contentStart && active.end <= line.length) {
      const before = line.slice(contentStart, active.start);
      const after = line.slice(active.end);
      return (
        <>
          {parseInlineSegments(before, contentStart).map((segment) => renderSegment(lineIndex, segment, null))}
          {renderActiveFragment(line, active)}
          {parseInlineSegments(after, active.end).map((segment) => renderSegment(lineIndex, segment, null))}
        </>
      );
    }

    const segments = parseInlineSegments(content, contentStart);
    if (segments.length === 0) {
      const edit = defaultEditForLine(line, lineIndex);
      return (
        <span
          className="markdown-live-fragment markdown-live-empty-fragment"
          onMouseDown={(event) => {
            if (result.truncated) return;
            event.preventDefault();
            setActiveEdit(edit);
          }}
        >
          &nbsp;
        </span>
      );
    }
    return segments.map((segment) => renderSegment(lineIndex, segment, active));
  }

  function renderLine(lineIndex: number, line: string) {
    const model = parseLineModel(line);
    const active = activeEdit?.line === lineIndex ? activeEdit : null;
    const wholeLineEdit = Boolean(active && active.start === 0 && active.end === line.length);

    if (model.kind === "blank") {
      return active ? (
        renderActiveFragment(line, active)
      ) : (
        <span
          className="markdown-live-blank"
          onMouseDown={(event) => {
            if (result.truncated) return;
            event.preventDefault();
            setActiveEdit({ line: lineIndex, start: 0, end: 0 });
          }}
        >
          &nbsp;
        </span>
      );
    }

    if (model.kind === "code") {
      return active ? (
        renderActiveFragment(line, active)
      ) : (
        <pre
          onMouseDown={(event) => {
            if (result.truncated) return;
            event.preventDefault();
            setActiveEdit({ line: lineIndex, start: 0, end: line.length });
          }}
        >
          <code>{line.trim().startsWith("```") ? line : line.trimStart()}</code>
        </pre>
      );
    }

    if (model.kind === "heading") {
      if (active && wholeLineEdit) {
        return renderActiveFragment(line, active);
      }
      const content = renderSegments(lineIndex, line, model.content, model.contentStart);
      const activateLine = (event: React.MouseEvent) => {
        if (result.truncated) return;
        event.preventDefault();
        setActiveEdit({ line: lineIndex, start: 0, end: line.length });
      };
      if (model.level === 1) return <h1 onMouseDown={activateLine}>{content}</h1>;
      if (model.level === 2) return <h2 onMouseDown={activateLine}>{content}</h2>;
      if (model.level === 3) return <h3 onMouseDown={activateLine}>{content}</h3>;
      if (model.level === 4) return <h4 onMouseDown={activateLine}>{content}</h4>;
      if (model.level === 5) return <h5 onMouseDown={activateLine}>{content}</h5>;
      return <h6 onMouseDown={activateLine}>{content}</h6>;
    }

    if (model.kind === "unordered" || model.kind === "ordered") {
      if (active && wholeLineEdit) {
        return renderActiveFragment(line, active);
      }
      return (
        <div
          className="markdown-live-list-line"
          onMouseDown={(event) => {
            if (result.truncated) return;
            event.preventDefault();
            setActiveEdit({ line: lineIndex, start: 0, end: line.length });
          }}
        >
          <span>{model.marker}</span>
          <p>{renderSegments(lineIndex, line, model.content, model.contentStart)}</p>
        </div>
      );
    }

    if (model.kind === "indented") {
      if (active && wholeLineEdit) {
        return renderActiveFragment(line, active);
      }
      return (
        <p
          className="markdown-live-indented-paragraph"
          style={{ "--markdown-indent-depth": model.depth } as React.CSSProperties}
          onMouseDown={(event) => {
            if (result.truncated) return;
            event.preventDefault();
            setActiveEdit({ line: lineIndex, start: 0, end: line.length });
          }}
        >
          {renderSegments(lineIndex, line, model.content, model.contentStart)}
        </p>
      );
    }

    return <p>{renderSegments(lineIndex, line, model.content, model.contentStart)}</p>;
  }

  return (
    <div className="markdown-editor">
      {saveError ? <div className="error-banner error-banner-compact">{saveError}</div> : null}
      {result.truncated ? (
        <div className="error-banner error-banner-compact">
          文件过大，当前只读预览，暂不支持保存。
        </div>
      ) : null}

      <div ref={editorRef} className="markdown-live-editor" role="textbox" aria-label="Markdown 编辑器">
        {lines.map((line, index) => {
          const editing = activeEdit?.line === index && !result.truncated;
          return (
            <div key={index} className={liveLineClassName(line, editing)}>
              <div className="markdown-live-rendered">{renderLine(index, line)}</div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
