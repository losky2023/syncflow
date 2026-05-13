import { useEffect, useMemo, useRef, useState } from "react";
import type { BaiduRemoteRepository, SyncRuntimeStatus, SyncedSpace } from "../../types/workbench";

interface SpaceListProps {
  spaces: SyncedSpace[];
  statusesBySpaceId: Record<string, SyncRuntimeStatus>;
  remoteRepositories: BaiduRemoteRepository[];
  selectedSpaceId: string | null;
  addPath: string;
  importParentPath: string;
  isPicking: boolean;
  isImportPicking: boolean;
  remoteRepositoriesLoading: boolean;
  importingRemotePath: string | null;
  error: string | null;
  onAddPathChange: (value: string) => void;
  onImportParentPathChange: (value: string) => void;
  onBrowse: () => void;
  onBrowseImportParent: () => void;
  onAdd: () => void;
  onLoadRemoteRepositories: () => void;
  onImportRemoteRepository: (repository: BaiduRemoteRepository) => void;
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
  remoteRepositories,
  selectedSpaceId,
  addPath,
  importParentPath,
  isPicking,
  isImportPicking,
  remoteRepositoriesLoading,
  importingRemotePath,
  error,
  onAddPathChange,
  onImportParentPathChange,
  onBrowse,
  onBrowseImportParent,
  onAdd,
  onLoadRemoteRepositories,
  onImportRemoteRepository,
  onSelect,
  onRemove,
  onStartSync,
  onStopSync,
  onBindBaiduSpace,
  canBindBaidu,
  syncActionBySpaceId,
}: SpaceListProps) {
  const switcherRef = useRef<HTMLElement | null>(null);
  const previousSelectedSpaceIdRef = useRef<string | null>(selectedSpaceId);
  const [isOpen, setIsOpen] = useState(false);
  const [isAdding, setIsAdding] = useState(false);
  const [isImporting, setIsImporting] = useState(false);
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

  useEffect(() => {
    const previous = previousSelectedSpaceIdRef.current;
    previousSelectedSpaceIdRef.current = selectedSpaceId;
    if (!isOpen || !selectedSpaceId || previous === selectedSpaceId) return;
    setIsOpen(false);
    setIsAdding(false);
    setIsImporting(false);
  }, [isOpen, selectedSpaceId]);

  useEffect(() => {
    if (!isOpen) return;

    function handlePointerDown(event: PointerEvent) {
      const target = event.target;
      if (!(target instanceof Node)) return;
      if (switcherRef.current?.contains(target)) return;
      setIsOpen(false);
      setIsAdding(false);
      setIsImporting(false);
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key !== "Escape") return;
      setIsOpen(false);
      setIsAdding(false);
      setIsImporting(false);
    }

    document.addEventListener("pointerdown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("pointerdown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [isOpen]);

  function closeMenu() {
    setIsOpen(false);
    setIsAdding(false);
    setIsImporting(false);
  }

  function selectSpace(spaceId: string) {
    onSelect(spaceId);
    if (spaceId !== selectedSpaceId) {
      closeMenu();
    }
  }

  function removeSelectedSpace() {
    if (!selectedSpaceId || !selectedSpace) return;
    const shouldRemove = window.confirm(`确定移除仓库「${selectedSpace.name}」吗？本地文件不会被删除。`);
    if (!shouldRemove) return;
    onRemove(selectedSpaceId);
    closeMenu();
  }

  function toggleMenu() {
    setIsOpen((current) => {
      const next = !current;
      if (next && spaces.length === 0) {
        setIsAdding(true);
        setIsImporting(false);
      }
      return next;
    });
  }

  return (
    <section className="vault-switcher" ref={switcherRef}>
      {isOpen ? (
        <div className="vault-menu panel" role="dialog" aria-label="管理仓库">
          <div className="vault-menu-header">
            <div className="vault-menu-title">
              <strong>管理仓库</strong>
              <span>已配置 {spaces.length} 个同步文件夹</span>
            </div>
            <div className="vault-menu-header-actions">
              <button
                type="button"
                className="secondary-button secondary-button-compact"
                onClick={() => {
                  setIsAdding((current) => !current);
                  setIsImporting(false);
                }}
              >
                {isAdding ? "收起添加" : "添加仓库"}
              </button>
              <button
                type="button"
                className="secondary-button secondary-button-compact"
                disabled={!canBindBaidu}
                title={canBindBaidu ? "浏览 /apps/SyncFlow 下的百度网盘仓库" : "请先连接百度网盘账号"}
                onClick={() => {
                  const next = !isImporting;
                  setIsImporting(next);
                  setIsAdding(false);
                  if (next) {
                    onLoadRemoteRepositories();
                  }
                }}
              >
                {isImporting ? "收起导入" : spaces.length === 0 ? "导入仓库" : "从网盘导入"}
              </button>
              <button
                type="button"
                className="icon-button"
                onClick={closeMenu}
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

          {isImporting ? (
            <div className="vault-add-panel vault-import-panel">
              <div className="vault-section-title">
                <strong>从网盘导入</strong>
                <span>选择本机保存位置，再导入云端仓库</span>
              </div>
              <div className="vault-import-toolbar">
                <div className="vault-import-path-field">
                  <span className="vault-field-label">保存到</span>
                  <input
                    value={importParentPath}
                    onChange={(event) => onImportParentPathChange(event.target.value)}
                    placeholder="选择本机父文件夹"
                  />
                </div>
                <button
                  type="button"
                  className="secondary-button secondary-button-compact"
                  onClick={onBrowseImportParent}
                  disabled={isImportPicking}
                >
                  {isImportPicking ? "选择中..." : "选择位置"}
                </button>
                <button
                  type="button"
                  className="secondary-button secondary-button-compact"
                  onClick={onLoadRemoteRepositories}
                  disabled={remoteRepositoriesLoading}
                >
                  {remoteRepositoriesLoading ? "刷新中..." : "刷新网盘"}
                </button>
              </div>
              <div className="vault-import-summary">
                <strong>百度网盘 /apps/SyncFlow</strong>
                <span>{remoteRepositoriesLoading ? "正在读取" : `${remoteRepositories.length} 个云端仓库`}</span>
              </div>
              <div className="vault-remote-list" role="listbox" aria-label="百度网盘仓库">
                {remoteRepositoriesLoading ? (
                  <div className="empty-card empty-card-compact">
                    <strong>正在读取网盘</strong>
                    <p>正在获取云端仓库列表。</p>
                  </div>
                ) : remoteRepositories.length === 0 ? (
                  <div className="empty-card empty-card-compact">
                    <strong>没有找到云端仓库</strong>
                    <p>确认百度网盘 /apps/SyncFlow 下已经有仓库目录。</p>
                  </div>
                ) : (
                  remoteRepositories.map((repository) => (
                    <div className="vault-remote-row" key={repository.remoteRootPath}>
                      <button
                        type="button"
                        className="vault-row vault-remote-select"
                        onClick={() => onImportRemoteRepository(repository)}
                        disabled={!importParentPath.trim() || importingRemotePath === repository.remoteRootPath}
                      >
                        <span className="vault-remote-main">
                          <span className="vault-name-row">
                            <span className="vault-name">{repository.name}</span>
                            {importingRemotePath === repository.remoteRootPath ? (
                              <span className="vault-selected-pill">导入中</span>
                            ) : null}
                          </span>
                          <span className="vault-path" title={repository.remoteRootPath}>
                            {repository.remoteRootPath}
                          </span>
                          <span className="vault-meta">
                            {repository.updatedAt ? `更新 ${new Date(repository.updatedAt).toLocaleString()}` : "百度网盘目录"}
                          </span>
                        </span>
                        <span className="vault-remote-action">
                          {importingRemotePath === repository.remoteRootPath ? "导入中" : "导入"}
                        </span>
                      </button>
                    </div>
                  ))
                )}
              </div>
            </div>
          ) : null}

          {error ? <div className="error-banner error-banner-compact">{error}</div> : null}

          <div className={selectedSpace ? "vault-manager-body" : "vault-manager-body vault-manager-body-empty"}>
            <div className="vault-list-section">
              <div className="vault-section-title">
                <strong>仓库列表</strong>
                <span>选择后会自动返回工作区</span>
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
                        onClick={() => selectSpace(space.id)}
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
                        <span className={isSelected ? "vault-row-action current" : "vault-row-action"}>
                          {isSelected ? "当前仓库" : "切换"}
                        </span>
                      </button>
                    );
                  })
                )}
              </div>
            </div>

            {selectedSpace ? (
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
                    onClick={removeSelectedSpace}
                  >
                  移除仓库
                  </button>
                ) : null}
              </div>
            </div>
            ) : null}
          </div>
        </div>
      ) : null}

      <button
        type="button"
        className="vault-trigger panel"
        onClick={toggleMenu}
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
