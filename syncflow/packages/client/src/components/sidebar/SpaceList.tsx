import type { SyncedSpace } from "../../types/workbench";

interface SpaceListProps {
  spaces: SyncedSpace[];
  selectedSpaceId: string | null;
  addPath: string;
  isPicking: boolean;
  error: string | null;
  onAddPathChange: (value: string) => void;
  onBrowse: () => void;
  onAdd: () => void;
  onSelect: (spaceId: string) => void;
  onRemove: (spaceId: string) => void;
}

export function SpaceList({
  spaces,
  selectedSpaceId,
  addPath,
  isPicking,
  error,
  onAddPathChange,
  onBrowse,
  onAdd,
  onSelect,
  onRemove,
}: SpaceListProps) {
  return (
    <section className="panel sidebar-section">
      <div className="section-header">
        <div>
          <h2>同步空间</h2>
          <p>选择一个空间后，在左侧树中浏览其内容。</p>
        </div>
      </div>

      <div className="space-add-row">
        <button className="primary-button" onClick={onBrowse} disabled={isPicking}>
          {isPicking ? "选择中..." : "浏览"}
        </button>
        <input
          value={addPath}
          onChange={(event) => onAddPathChange(event.target.value)}
          placeholder="输入本地文件夹路径"
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              onAdd();
            }
          }}
        />
        <button className="secondary-button" onClick={onAdd}>
          添加
        </button>
      </div>

      {error ? <div className="error-banner">{error}</div> : null}

      {spaces.length === 0 ? (
        <div className="empty-card">
          <strong>还没有同步空间</strong>
          <p>添加一个本地文件夹后，它会显示在这里，并在下次启动时保留。</p>
        </div>
      ) : (
        <div className="space-list">
          {spaces.map((space) => {
            const isSelected = selectedSpaceId === space.id;
            return (
              <button
                key={space.id}
                className={isSelected ? "space-card selected" : "space-card"}
                onClick={() => onSelect(space.id)}
              >
                <div className="space-card-main">
                  <strong>{space.name}</strong>
                  <span>{space.status}</span>
                  <small>{space.rootPath}</small>
                </div>
                <span
                  className="space-remove"
                  onClick={(event) => {
                    event.stopPropagation();
                    onRemove(space.id);
                  }}
                >
                  移除
                </span>
              </button>
            );
          })}
        </div>
      )}
    </section>
  );
}
