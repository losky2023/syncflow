import { useMemo, useState } from "react";
import type { SyncRuntimeStatus, SyncedSpace } from "../../types/workbench";

interface SpaceListProps {
  spaces: SyncedSpace[];
  statusesBySpaceId: Record<string, SyncRuntimeStatus>;
  selectedSpaceId: string | null;
  addPath: string;
  isPicking: boolean;
  error: string | null;
  onAddPathChange: (value: string) => void;
  onBrowse: () => void;
  onAdd: () => void;
  onSelect: (spaceId: string) => void;
  onRemove: (spaceId: string) => void;
  onStartSync: (spaceId: string) => void;
  onStopSync: (spaceId: string) => void;
  onBindBaiduSpace: (spaceId: string) => void;
  canBindBaidu: boolean;
  syncActionBySpaceId: Record<string, "start" | "stop" | undefined>;
}

function runtimeStatusLabel(status?: SyncRuntimeStatus) {
  switch (status?.status) {
    case "watching":
    case "syncing":
      return "运行中";
    case "indexing":
    case "starting":
      return "启动中";
    case "error":
      return "异常";
    default:
      return "未启动";
  }
}

function runtimeStatusClass(status?: SyncRuntimeStatus) {
  return `vault-status-badge status-${status?.status ?? "stopped"}`;
}

function syncHealthLabel(status?: SyncRuntimeStatus) {
  if (status?.lastError) return "有错误";
  if ((status?.cloudConflictCount ?? 0) > 0) return `云冲突 ${status?.cloudConflictCount}`;
  if ((status?.pendingCount ?? 0) > 0) return `待同步 ${status?.pendingCount}`;
  if (status?.status === "watching" || status?.status === "syncing") return "已就绪";
  if (status?.status === "starting" || status?.status === "indexing") return "正在准备";
  return "等待启动";
}

function isStarting(status?: SyncRuntimeStatus) {
  return status?.status === "starting" || status?.status === "indexing";
}

function isRunning(status?: SyncRuntimeStatus) {
  return status?.status === "watching" || status?.status === "syncing";
}

export function SpaceList({
  spaces,
  statusesBySpaceId,
  selectedSpaceId,
  addPath,
  isPicking,
  error,
  onAddPathChange,
  onBrowse,
  onAdd,
  onSelect,
  onRemove,
  onStartSync,
  onStopSync,
  onBindBaiduSpace,
  canBindBaidu,
  syncActionBySpaceId,
}: SpaceListProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [isAdding, setIsAdding] = useState(false);
  const selectedSpace = useMemo(
    () => spaces.find((space) => space.id === selectedSpaceId) ?? null,
    [spaces, selectedSpaceId],
  );
  const selectedStatus = selectedSpaceId ? statusesBySpaceId[selectedSpaceId] : undefined;
  const selectedCloudBinding = selectedSpace?.cloudBinding;
  const selectedIsCloudSpace = Boolean(selectedStatus?.cloudProvider ?? selectedCloudBinding);
  const selectedRemotePath = selectedStatus?.cloudRemotePath ?? selectedCloudBinding?.remoteRootPath;
  const selectedAction = selectedSpaceId ? syncActionBySpaceId[selectedSpaceId] : undefined;
  const selectedStarting = isStarting(selectedStatus);
  const selectedRunning = isRunning(selectedStatus);
  const selectedHasError = selectedStatus?.status === "error";
  const selectedPendingCount = selectedStatus?.pendingCount ?? 0;
  const selectedPrimarySyncLabel =
    selectedAction === "start"
      ? "启动中"
      : selectedStarting
        ? "启动中"
        : selectedHasError
          ? "重新启动"
          : selectedRunning
            ? selectedPendingCount > 0
              ? "同步中..."
              : "已同步"
            : "启动同步";
  const selectedPrimarySyncDisabled =
    !selectedSpaceId || Boolean(selectedAction) || selectedStarting || (selectedRunning && !selectedHasError);
  const selectedShowPauseSync = selectedRunning || selectedAction === "stop";

  return (
    <section className="vault-switcher">
      {isOpen ? (
        <div className="vault-menu panel">
          <div className="vault-menu-header">
            <div className="vault-menu-title">
              <strong>管理仓库</strong>
              <span>已配置 {spaces.length} 个同步文件夹</span>
            </div>
            <div className="vault-menu-header-actions">
              <button
                type="button"
                className="secondary-button secondary-button-compact"
                onClick={() => setIsAdding((current) => !current)}
              >
                {isAdding ? "收起添加" : "添加仓库"}
              </button>
              <button
                type="button"
                className="icon-button"
                onClick={() => setIsOpen(false)}
                aria-label="关闭仓库管理"
                title="关闭"
              >
                ×
              </button>
            </div>
          </div>

          {isAdding ? (
            <div className="vault-add-panel">
              <div className="vault-section-title">
                <strong>添加仓库</strong>
                <span>选择一个本地文件夹作为同步仓库</span>
              </div>
              <div className="vault-add-row">
                <button
                  type="button"
                  className="secondary-button secondary-button-compact"
                  onClick={onBrowse}
                  disabled={isPicking}
                >
                  {isPicking ? "选择中..." : "浏览"}
                </button>
                <input
                  value={addPath}
                  onChange={(event) => onAddPathChange(event.target.value)}
                  placeholder="本地同步文件夹路径"
                  onKeyDown={(event) => {
                    if (event.key === "Enter") {
                      onAdd();
                    }
                  }}
                />
                <button type="button" className="primary-button primary-button-compact" onClick={onAdd}>
                  添加
                </button>
              </div>
            </div>
          ) : null}

          {error ? <div className="error-banner error-banner-compact">{error}</div> : null}

          <div className="vault-manager-body">
            <div className="vault-list-section">
              <div className="vault-section-title">
                <strong>仓库列表</strong>
                <span>点击切换当前仓库</span>
              </div>
              <div className="vault-list" role="listbox" aria-label="已配置的同步文件夹">
                {spaces.length === 0 ? (
                  <div className="empty-card empty-card-compact">
                    <strong>还没有仓库</strong>
                    <p>添加一个本地文件夹后，可以在这里切换和绑定百度网盘。</p>
                  </div>
                ) : (
                  spaces.map((space) => {
                    const status = statusesBySpaceId[space.id];
                    const cloudBinding = space.cloudBinding;
                    const isCloudSpace = Boolean(status?.cloudProvider ?? cloudBinding);
                    const isSelected = selectedSpaceId === space.id;
                    return (
                      <button
                        key={space.id}
                        type="button"
                        className={isSelected ? "vault-row selected" : "vault-row"}
                        role="option"
                        aria-selected={isSelected}
                        onClick={() => onSelect(space.id)}
                      >
                        <span className="vault-name-row">
                          <span className="vault-name">{space.name}</span>
                          {isSelected ? <span className="vault-selected-pill">当前</span> : null}
                        </span>
                        <span className="vault-path" title={space.rootPath}>
                          {space.rootPath}
                        </span>
                        <span className="vault-runtime-line">
                          <span className={runtimeStatusClass(status)}>{runtimeStatusLabel(status)}</span>
                          <span className="vault-health-chip">{syncHealthLabel(status)}</span>
                          <span className="vault-meta">
                            {isCloudSpace ? "百度网盘" : "仅本地"} · 文件 {status?.fileCount ?? 0} · 队列{" "}
                            {status?.pendingCount ?? 0}
                          </span>
                        </span>
                        {status?.lastError ? <span className="vault-error">{status.lastError}</span> : null}
                      </button>
                    );
                  })
                )}
              </div>
            </div>

            <div className="vault-current-card" aria-label="当前仓库详情">
              <div className="vault-current-head">
                <span className="vault-card-label">当前仓库</span>
                <span className={runtimeStatusClass(selectedStatus)}>{runtimeStatusLabel(selectedStatus)}</span>
                <span className="vault-health-chip">{syncHealthLabel(selectedStatus)}</span>
              </div>
              <div className="vault-current-main">
                <strong>{selectedSpace?.name ?? "未选择仓库"}</strong>
                <span title={selectedSpace?.rootPath}>
                  {selectedSpace?.rootPath ?? "添加或选择一个本地同步文件夹"}
                </span>
                {selectedRemotePath ? (
                  <small className="vault-remote-path" title={selectedRemotePath}>
                    {selectedRemotePath}
                  </small>
                ) : null}
              </div>
              <div className="vault-current-stats">
                <span>
                  <strong>{selectedStatus?.fileCount ?? 0}</strong>
                  <small>文件</small>
                </span>
                <span>
                  <strong>{selectedStatus?.pendingCount ?? 0}</strong>
                  <small>队列</small>
                </span>
                <span>
                  <strong>{selectedStatus?.cloudConflictCount ?? 0}</strong>
                  <small>云冲突</small>
                </span>
                <span>
                  <strong>{selectedIsCloudSpace ? "已绑定" : "未绑定"}</strong>
                  <small>网盘</small>
                </span>
              </div>
              {selectedStatus?.lastError ? <div className="vault-error vault-detail-error">{selectedStatus.lastError}</div> : null}
              <div className="vault-actions vault-detail-actions">
                {selectedSpace && !selectedIsCloudSpace ? (
                  <button
                    type="button"
                    className="secondary-button secondary-button-compact"
                    disabled={!canBindBaidu}
                    title={canBindBaidu ? "绑定到 /apps/SyncFlow 下的百度网盘目录" : "请先连接百度网盘账号"}
                    onClick={() => onBindBaiduSpace(selectedSpace.id)}
                  >
                    绑定网盘
                  </button>
                ) : (
                  <button
                    type="button"
                    className={
                      selectedHasError || !selectedRunning
                        ? "primary-button primary-button-compact vault-action-start"
                        : selectedPendingCount > 0
                          ? "secondary-button secondary-button-compact vault-action-state vault-action-syncing"
                          : "secondary-button secondary-button-compact vault-action-state vault-action-synced"
                    }
                    onClick={() => {
                      if (selectedSpaceId && !selectedRunning && !selectedStarting) {
                        onStartSync(selectedSpaceId);
                      }
                    }}
                    disabled={selectedPrimarySyncDisabled}
                  >
                    {selectedPrimarySyncLabel}
                  </button>
                )}
                {selectedShowPauseSync && selectedSpaceId ? (
                  <button
                    type="button"
                    className="secondary-button secondary-button-compact vault-action-pause"
                    onClick={() => onStopSync(selectedSpaceId)}
                    disabled={Boolean(selectedAction) || !selectedRunning}
                  >
                    {selectedAction === "stop" ? "暂停中" : "暂停"}
                  </button>
                ) : null}
                {selectedSpaceId ? (
                  <button
                    type="button"
                    className="ghost-danger-button"
                    onClick={() => onRemove(selectedSpaceId)}
                  >
                    移除仓库
                  </button>
                ) : null}
              </div>
            </div>
          </div>
        </div>
      ) : null}

      <button
        type="button"
        className="vault-trigger panel"
        onClick={() => setIsOpen((current) => !current)}
        aria-expanded={isOpen}
      >
        <span className="vault-trigger-icon" aria-hidden="true">
          仓
        </span>
        <span className="vault-trigger-main">
          <strong>{selectedSpace?.name ?? "打开仓库"}</strong>
          <span>
            {selectedSpace
              ? `${selectedIsCloudSpace ? "百度网盘" : "仅本地"} - ${runtimeStatusLabel(selectedStatus)}`
              : "选择或添加同步文件夹"}
          </span>
        </span>
        <span className="vault-trigger-chevron" aria-hidden="true">
          {isOpen ? "v" : "^"}
        </span>
      </button>
    </section>
  );
}
