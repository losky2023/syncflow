import type { TreeNode } from "../../types/workbench";
import { FileTreeNode } from "./FileTreeNode";

export type TreeCreateDraft = {
  parentRelativePath: string | null;
  kind: "file" | "folder";
};

interface FileTreeProps {
  roots: TreeNode[];
  selectedPath: string | null;
  createDraft: TreeCreateDraft | null;
  createName: string;
  createError: string | null;
  creating: boolean;
  expandedPaths: Set<string>;
  childrenByPath: Record<string, TreeNode[]>;
  treeLoadingByPath: Record<string, boolean>;
  treeErrorByPath: Record<string, string | null>;
  rootLoading: boolean;
  rootError: string | null;
  actionMenuPath: string | null;
  renameDraft: TreeNode | null;
  renameName: string;
  renameError: string | null;
  mutationLoading: boolean;
  onToggle: (node: TreeNode) => void;
  onSelect: (node: TreeNode) => void;
  onStartCreate: (parentRelativePath: string | null | undefined, kind: "file" | "folder") => void;
  onCreateNameChange: (value: string) => void;
  onCommitCreate: () => void;
  onCancelCreate: () => void;
  onActionMenuChange: (relativePath: string | null) => void;
  onStartRename: (node: TreeNode) => void;
  onRenameNameChange: (value: string) => void;
  onCommitRename: () => void;
  onCancelRename: () => void;
  onRequestDelete: (node: TreeNode) => void;
  onStartMove: (node: TreeNode) => void;
  onCopyRelativePath: (node: TreeNode) => void;
  onReveal: (node: TreeNode) => void;
  onRefreshPath: (relativePath: string | null) => void;
  onImportDocument: () => void;
  onImportWeChatArticle: () => void;
}

export function FileTree({
  roots,
  selectedPath,
  createDraft,
  createName,
  createError,
  creating,
  expandedPaths,
  childrenByPath,
  treeLoadingByPath,
  treeErrorByPath,
  rootLoading,
  rootError,
  actionMenuPath,
  renameDraft,
  renameName,
  renameError,
  mutationLoading,
  onToggle,
  onSelect,
  onStartCreate,
  onCreateNameChange,
  onCommitCreate,
  onCancelCreate,
  onActionMenuChange,
  onStartRename,
  onRenameNameChange,
  onCommitRename,
  onCancelRename,
  onRequestDelete,
  onStartMove,
  onCopyRelativePath,
  onReveal,
  onRefreshPath,
  onImportDocument,
  onImportWeChatArticle,
}: FileTreeProps) {
  const rootDraft = createDraft?.parentRelativePath === null ? createDraft : null;

  return (
    <section className="panel tree-section tree-section-compact">
      <div className="section-header compact-header tree-header-compact">
        <div>
          <h2>文件</h2>
          <p>管理当前仓库内容。</p>
        </div>
        <div className="tree-header-actions">
          <button
            type="button"
            className="tree-action-button"
            onClick={() => onRefreshPath(null)}
            title="刷新"
          >
            ↻
          </button>
          <button type="button" className="tree-action-button" onClick={() => onStartCreate(null, "file")} title="新建文件">
            +
          </button>
          <button
            type="button"
            className="tree-action-button"
            onClick={onImportDocument}
            title="导入文档为 Markdown"
            aria-label="导入文档为 Markdown"
          >
            <svg className="tree-action-icon" viewBox="0 0 20 20" aria-hidden="true">
              <path d="M5 2.75h6.25L15 6.5v10.75H5z" />
              <path d="M11.25 2.75V6.5H15" />
              <path d="M10 8.5v5" />
              <path d="m7.75 11.75 2.25 2.25 2.25-2.25" />
            </svg>
          </button>
          <button
            type="button"
            className="tree-action-button tree-action-button-wechat"
            onClick={onImportWeChatArticle}
            title="Import WeChat article from clipboard"
            aria-label="Import WeChat article from clipboard"
          >
            Wx
          </button>
          <button
            type="button"
            className="tree-action-button"
            onClick={() => onStartCreate(null, "folder")}
            title="新建文件夹"
          >
            ▣
          </button>
        </div>
      </div>

      {rootDraft ? (
        <CreateInput
          depth={0}
          kind={rootDraft.kind}
          value={createName}
          error={createError}
          creating={creating}
          onChange={onCreateNameChange}
          onCommit={onCommitCreate}
          onCancel={onCancelCreate}
        />
      ) : null}

      {rootLoading ? <div className="empty-card empty-card-compact">正在加载文件...</div> : null}
      {rootError ? <div className="error-banner error-banner-compact">{rootError}</div> : null}
      {!rootLoading && !rootError && roots.length === 0 && !rootDraft ? (
        <div className="empty-card empty-card-compact">当前仓库为空，可以新建文件或文件夹。</div>
      ) : null}

      {!rootLoading && !rootError && roots.length > 0 ? (
        <div className="tree-list">
          {roots.map((node) => (
            <FileTreeNode
              key={node.relativePath}
              node={node}
              depth={0}
              selectedPath={selectedPath}
              createDraft={createDraft}
              createName={createName}
              createError={createError}
              creating={creating}
              expandedPaths={expandedPaths}
              childrenByPath={childrenByPath}
              treeLoadingByPath={treeLoadingByPath}
              treeErrorByPath={treeErrorByPath}
              actionMenuPath={actionMenuPath}
              renameDraft={renameDraft}
              renameName={renameName}
              renameError={renameError}
              mutationLoading={mutationLoading}
              onToggle={onToggle}
              onSelect={onSelect}
              onStartCreate={onStartCreate}
              onCreateNameChange={onCreateNameChange}
              onCommitCreate={onCommitCreate}
              onCancelCreate={onCancelCreate}
              onActionMenuChange={onActionMenuChange}
              onStartRename={onStartRename}
              onRenameNameChange={onRenameNameChange}
              onCommitRename={onCommitRename}
              onCancelRename={onCancelRename}
              onRequestDelete={onRequestDelete}
              onStartMove={onStartMove}
              onCopyRelativePath={onCopyRelativePath}
              onReveal={onReveal}
              onRefreshPath={onRefreshPath}
            />
          ))}
        </div>
      ) : null}
    </section>
  );
}

interface CreateInputProps {
  depth: number;
  kind: "file" | "folder";
  value: string;
  error: string | null;
  creating: boolean;
  onChange: (value: string) => void;
  onCommit: () => void;
  onCancel: () => void;
}

export function CreateInput({
  depth,
  kind,
  value,
  error,
  creating,
  onChange,
  onCommit,
  onCancel,
}: CreateInputProps) {
  return (
    <div className="tree-create-row" style={{ paddingLeft: `${10 + depth * 12}px` }}>
      <span className={kind === "folder" ? "tree-icon directory" : "tree-icon file"} />
      <input
        autoFocus
        value={value}
        disabled={creating}
        placeholder={kind === "folder" ? "文件夹名称" : "文件名，默认 .md"}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter") onCommit();
          if (event.key === "Escape") onCancel();
        }}
        onBlur={() => {
          if (!creating && !value.trim()) onCancel();
        }}
      />
      {error ? <span className="tree-create-error">{error}</span> : null}
    </div>
  );
}
