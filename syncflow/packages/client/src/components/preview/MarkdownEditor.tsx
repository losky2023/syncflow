import { markdown } from "@codemirror/lang-markdown";
import { Compartment, EditorState, RangeSetBuilder } from "@codemirror/state";
import {
  Decoration,
  type DecorationSet,
  EditorView,
  keymap,
  placeholder,
  ViewPlugin,
  type ViewUpdate,
  WidgetType,
} from "@codemirror/view";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { useEffect, useMemo, useRef, useState } from "react";
import type { TextPreviewResult, TreeNode } from "../../types/workbench";

interface MarkdownEditorProps {
  node: TreeNode;
  result: TextPreviewResult;
  isSaving: boolean;
  saveError: string | null;
  onSave: (content: string) => Promise<void>;
  onStateChange?: (state: { content: string; isDirty: boolean; wordCount: number }) => void;
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

type MarkdownDecoration = {
  from: number;
  to: number;
  decoration: Decoration;
};

class MarkdownMarkerWidget extends WidgetType {
  constructor(
    private readonly text: string,
    private readonly className: string,
  ) {
    super();
  }

  eq(other: MarkdownMarkerWidget) {
    return this.text === other.text && this.className === other.className;
  }

  toDOM() {
    const element = document.createElement("span");
    element.className = this.className;
    element.textContent = this.text;
    return element;
  }
}

function activeLineNumber(view: EditorView) {
  if (!view.hasFocus) return null;
  return view.state.doc.lineAt(view.state.selection.main.head).number;
}

function pushHiddenMarker(
  decorations: MarkdownDecoration[],
  lineFrom: number,
  start: number,
  end: number,
  widget?: WidgetType,
) {
  if (end <= start) return;
  decorations.push({
    from: lineFrom + start,
    to: lineFrom + end,
    decoration: Decoration.replace({
      widget,
      inclusive: false,
    }),
  });
}

function pushInlineMark(
  decorations: MarkdownDecoration[],
  lineFrom: number,
  start: number,
  end: number,
  className: string,
) {
  if (end <= start) return;
  decorations.push({
    from: lineFrom + start,
    to: lineFrom + end,
    decoration: Decoration.mark({ class: className }),
  });
}

function decorateInlineMarkdown(
  decorations: MarkdownDecoration[],
  lineText: string,
  lineFrom: number,
) {
  for (const match of lineText.matchAll(/\[([^\]\n]+)\]\(([^)\n]+)\)/g)) {
    if (match.index === undefined || !match[1]) continue;
    const start = match.index;
    const labelStart = start + 1;
    const labelEnd = labelStart + match[1].length;
    const end = start + match[0].length;
    pushHiddenMarker(decorations, lineFrom, start, labelStart);
    pushHiddenMarker(decorations, lineFrom, labelEnd, end);
    pushInlineMark(decorations, lineFrom, labelStart, labelEnd, "cm-md-link");
  }

  for (const match of lineText.matchAll(/`([^`\n]+)`/g)) {
    if (match.index === undefined || !match[1]) continue;
    const start = match.index;
    const contentStart = start + 1;
    const contentEnd = contentStart + match[1].length;
    pushHiddenMarker(decorations, lineFrom, start, contentStart);
    pushHiddenMarker(decorations, lineFrom, contentEnd, contentEnd + 1);
    pushInlineMark(decorations, lineFrom, contentStart, contentEnd, "cm-md-inline-code");
  }

  for (const match of lineText.matchAll(/(\*\*|__)([^\n]+?)\1/g)) {
    if (match.index === undefined || !match[1] || !match[2]) continue;
    const start = match.index;
    const markerLength = match[1].length;
    const contentStart = start + markerLength;
    const contentEnd = contentStart + match[2].length;
    pushHiddenMarker(decorations, lineFrom, start, contentStart);
    pushHiddenMarker(decorations, lineFrom, contentEnd, contentEnd + markerLength);
    pushInlineMark(decorations, lineFrom, contentStart, contentEnd, "cm-md-bold");
  }

  for (const match of lineText.matchAll(/(^|[^\*_])(\*|_)([^\s*_][^\n]*?[^\s*_])\2/g)) {
    if (match.index === undefined || !match[2] || !match[3]) continue;
    const prefixLength = match[1].length;
    const markerStart = match.index + prefixLength;
    const contentStart = markerStart + 1;
    const contentEnd = contentStart + match[3].length;
    pushHiddenMarker(decorations, lineFrom, markerStart, contentStart);
    pushHiddenMarker(decorations, lineFrom, contentEnd, contentEnd + 1);
    pushInlineMark(decorations, lineFrom, contentStart, contentEnd, "cm-md-italic");
  }
}

function decorateMarkdownLine(
  decorations: MarkdownDecoration[],
  lineText: string,
  lineFrom: number,
  lineNumber: number,
  activeLine: number | null,
) {
  const isActiveLine = lineNumber === activeLine;
  const heading = lineText.match(/^(#{1,6})(\s+)/);
  if (heading) {
    const level = heading[1].length;
    decorations.push({
      from: lineFrom,
      to: lineFrom,
      decoration: Decoration.line({
        class: `cm-md-heading-line cm-md-heading-line-${level}`,
      }),
    });
    if (!isActiveLine) {
      pushHiddenMarker(decorations, lineFrom, 0, heading[1].length + heading[2].length);
    }
  }

  const task = lineText.match(/^(\s*)([-*+])\s+\[([ xX])\]\s+/);
  if (task && !isActiveLine) {
    const checked = task[3].toLowerCase() === "x";
    pushHiddenMarker(
      decorations,
      lineFrom,
      task[1].length,
      task[0].length,
      new MarkdownMarkerWidget(checked ? "☑ " : "☐ ", "cm-md-task-marker"),
    );
  } else {
    const list = lineText.match(/^(\s*)((?:[-*+])|(?:\d+[.)]))(\s+)/);
    if (list && !isActiveLine) {
      const marker = /^\d/.test(list[2]) ? `${list[2]} ` : "• ";
      pushHiddenMarker(
        decorations,
        lineFrom,
        list[1].length,
        list[0].length,
        new MarkdownMarkerWidget(marker, "cm-md-list-marker"),
      );
    }
  }

  const quote = lineText.match(/^(\s*)>\s?/);
  if (quote) {
    decorations.push({
      from: lineFrom,
      to: lineFrom,
      decoration: Decoration.line({ class: "cm-md-quote-line" }),
    });
    if (!isActiveLine) {
      pushHiddenMarker(decorations, lineFrom, quote[1].length, quote[0].length);
    }
  }

  if (!isActiveLine) {
    decorateInlineMarkdown(decorations, lineText, lineFrom);
  }
}

function buildMarkdownLivePreviewDecorations(view: EditorView) {
  const decorations: MarkdownDecoration[] = [];
  const activeLine = activeLineNumber(view);

  for (const range of view.visibleRanges) {
    let position = range.from;
    while (position <= range.to) {
      const line = view.state.doc.lineAt(position);
      decorateMarkdownLine(decorations, line.text, line.from, line.number, activeLine);
      if (line.to >= range.to) break;
      position = line.to + 1;
    }
  }

  decorations.sort((a, b) => a.from - b.from || a.to - b.to);
  const builder = new RangeSetBuilder<Decoration>();
  for (const item of decorations) {
    builder.add(item.from, item.to, item.decoration);
  }
  return builder.finish();
}

const markdownLivePreview = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    private focused: boolean;

    constructor(view: EditorView) {
      this.focused = view.hasFocus;
      this.decorations = buildMarkdownLivePreviewDecorations(view);
    }

    update(update: ViewUpdate) {
      const focusChanged = update.view.hasFocus !== this.focused;
      this.focused = update.view.hasFocus;
      if (update.docChanged || update.viewportChanged || update.selectionSet || focusChanged) {
        this.decorations = buildMarkdownLivePreviewDecorations(update.view);
      }
    }
  },
  {
    decorations: (plugin) => plugin.decorations,
  },
);

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
  const contentRef = useRef(result.content);
  const savedContentRef = useRef(result.content);
  const activePathRef = useRef(node.relativePath);
  const containerRef = useRef<HTMLDivElement | null>(null);
  const editorRef = useRef<EditorView | null>(null);
  const saveTimerRef = useRef<number | null>(null);
  const saveInFlightRef = useRef(false);
  const pendingSaveRef = useRef<string | null>(null);
  const onSaveRef = useRef(onSave);
  const isSavingRef = useRef(isSaving);
  const editableCompartmentRef = useRef(new Compartment());
  const isDirty = content !== lastSavedContent;
  const wordCount = useMemo(() => countWords(content), [content]);

  useEffect(() => {
    onSaveRef.current = onSave;
  }, [onSave]);

  useEffect(() => {
    isSavingRef.current = isSaving;
  }, [isSaving]);

  useEffect(() => {
    contentRef.current = content;
  }, [content]);

  useEffect(() => {
    savedContentRef.current = lastSavedContent;
  }, [lastSavedContent]);

  useEffect(() => {
    onStateChange?.({ content, isDirty, wordCount });
  }, [content, isDirty, onStateChange, wordCount]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const view = new EditorView({
      parent: container,
      state: EditorState.create({
        doc: result.content,
        extensions: [
          history(),
          markdown(),
          markdownLivePreview,
          placeholder("开始编辑 Markdown..."),
          keymap.of([
            {
              key: "Mod-s",
              run: () => {
                void saveContent(contentRef.current).catch(() => undefined);
                return true;
              },
            },
            indentWithTab,
            ...defaultKeymap,
            ...historyKeymap,
          ]),
          EditorView.lineWrapping,
          editableCompartmentRef.current.of(EditorView.editable.of(!result.truncated)),
          EditorView.updateListener.of((update) => {
            if (!update.docChanged) return;
            const nextContent = update.state.doc.toString();
            contentRef.current = nextContent;
            setContent(nextContent);
          }),
        ],
      }),
    });

    editorRef.current = view;
    return () => {
      view.destroy();
      if (editorRef.current === view) {
        editorRef.current = null;
      }
    };
  }, []);

  useEffect(() => {
    const isSamePath = activePathRef.current === node.relativePath;
    activePathRef.current = node.relativePath;

    savedContentRef.current = result.content;
    setLastSavedContent(result.content);

    if (isSamePath && result.content === contentRef.current) {
      return;
    }

    contentRef.current = result.content;
    setContent(result.content);

    const view = editorRef.current;
    if (!view) return;

    const currentDoc = view.state.doc.toString();
    if (currentDoc === result.content) return;
    view.dispatch({
      changes: { from: 0, to: view.state.doc.length, insert: result.content },
    });
  }, [node.relativePath, result.content]);

  useEffect(() => {
    const view = editorRef.current;
    if (!view) return;
    view.dispatch({
      effects: editableCompartmentRef.current.reconfigure(EditorView.editable.of(!result.truncated)),
    });
  }, [result.truncated]);

  useEffect(() => {
    return () => {
      if (saveTimerRef.current !== null) {
        window.clearTimeout(saveTimerRef.current);
      }
    };
  }, []);

  useEffect(() => {
    if (result.truncated || !isDirty) return;
    if (saveTimerRef.current !== null) {
      window.clearTimeout(saveTimerRef.current);
    }
    saveTimerRef.current = window.setTimeout(() => {
      saveTimerRef.current = null;
      void saveContent(contentRef.current).catch(() => undefined);
    }, 1500);
  }, [content, isDirty, result.truncated]);

  async function saveContent(nextContent = contentRef.current) {
    if (result.truncated || nextContent === savedContentRef.current) return;
    if (saveInFlightRef.current || isSavingRef.current) {
      pendingSaveRef.current = nextContent;
      return;
    }

    saveInFlightRef.current = true;
    try {
      await onSaveRef.current(nextContent);
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

  return (
    <div className="markdown-editor">
      {saveError ? <div className="error-banner error-banner-compact">{saveError}</div> : null}
      {result.truncated ? (
        <div className="error-banner error-banner-compact">
          文件过大，当前只读预览，暂不支持保存。
        </div>
      ) : null}
      <div ref={containerRef} className="markdown-codemirror-editor" />
    </div>
  );
}
