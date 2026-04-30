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
  onToggle: (node: TreeNode) => void;
  onSelect: (node: TreeNode) => void;
  onStartCreate: (parentRelativePath: string | null, kind: "file" | "folder") => void;
  onCreateNameChange: (value: string) => void;
  onCommitCreate: () => void;
  onCancelCreate: () => void;
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
  onToggle,
  onSelect,
  onStartCreate,
  onCreateNameChange,
  onCommitCreate,
  onCancelCreate,
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
          <button type="button" className="tree-action-button" onClick={() => onStartCreate(null, "file")} title="新建文件">
            +
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
              onToggle={onToggle}
              onSelect={onSelect}
              onStartCreate={onStartCreate}
              onCreateNameChange={onCreateNameChange}
              onCommitCreate={onCommitCreate}
              onCancelCreate={onCancelCreate}
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
