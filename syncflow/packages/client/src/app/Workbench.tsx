import { useEffect, useMemo, useRef, useState } from "react";
import {
  addSyncedSpace,
  getDeviceInfo,
  getDiscoveredDevices,
  getFileDetails,
  getTreeChildren,
  openFile,
  pickFolder,
  previewImageFile,
  previewTextFile,
  removeSyncedSpace,
  listSyncedSpaces,
} from "../lib/tauriClient";
import type {
  DeviceInfo,
  DiscoveredDevice,
  FileDetails,
  PreviewState,
  SyncedSpace,
  TreeNode,
} from "../types/workbench";
import { SpaceList } from "../components/sidebar/SpaceList";
import { FileTree } from "../components/sidebar/FileTree";
import { PreviewPane } from "../components/preview/PreviewPane";
import { DetailsPane } from "../components/details/DetailsPane";

interface WorkbenchProps {
  syncStatus: {
    syncRunning: boolean;
    deviceName: string;
    deviceId: string;
  };
  onStopSync: () => void;
}

const TEXT_EXTENSIONS = new Set([
  "txt",
  "md",
  "json",
  "xml",
  "yml",
  "yaml",
  "toml",
  "csv",
  "html",
  "htm",
  "css",
  "scss",
  "less",
  "js",
  "ts",
  "jsx",
  "tsx",
  "rs",
  "py",
  "go",
  "java",
  "c",
  "cpp",
  "h",
  "hpp",
  "rb",
  "php",
  "log",
  "ini",
  "cfg",
  "conf",
  "env",
  "sh",
  "bat",
  "ps1",
]);

const IMAGE_EXTENSIONS = new Set(["png", "jpg", "jpeg", "gif", "webp", "svg"]);

export function Workbench({ syncStatus, onStopSync }: WorkbenchProps) {
  const [spaces, setSpaces] = useState<SyncedSpace[]>([]);
  const [selectedSpaceId, setSelectedSpaceId] = useState<string | null>(null);
  const [addPath, setAddPath] = useState("");
  const [isPicking, setIsPicking] = useState(false);
  const [spaceError, setSpaceError] = useState<string | null>(null);
  const [rootNodes, setRootNodes] = useState<TreeNode[]>([]);
  const [selectedNode, setSelectedNode] = useState<TreeNode | null>(null);
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set());
  const [childrenByPath, setChildrenByPath] = useState<Record<string, TreeNode[]>>({});
  const [treeLoadingByPath, setTreeLoadingByPath] = useState<Record<string, boolean>>({});
  const [treeErrorByPath, setTreeErrorByPath] = useState<Record<string, string | null>>({});
  const [rootLoading, setRootLoading] = useState(false);
  const [rootError, setRootError] = useState<string | null>(null);
  const [details, setDetails] = useState<FileDetails | null>(null);
  const [detailsError, setDetailsError] = useState<string | null>(null);
  const [preview, setPreview] = useState<PreviewState>({ type: "welcome" });
  const [connectedDevices, setConnectedDevices] = useState<DeviceInfo[]>([]);
  const [discoveredDevices, setDiscoveredDevices] = useState<DiscoveredDevice[]>([]);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    void loadSpaces();
    void loadDeviceState();

    pollRef.current = setInterval(() => {
      void loadDeviceState();
    }, 5000);

    return () => {
      if (pollRef.current) {
        clearInterval(pollRef.current);
      }
    };
  }, []);

  useEffect(() => {
    if (!selectedSpaceId) {
      setRootNodes([]);
      setSelectedNode(null);
      setDetails(null);
      setDetailsError(null);
      setPreview({ type: "welcome" });
      return;
    }

    void loadRootNodes(selectedSpaceId);
  }, [selectedSpaceId]);

  async function loadSpaces() {
    try {
      const nextSpaces = await listSyncedSpaces();
      setSpaces(nextSpaces);
      setSelectedSpaceId((current) => {
        if (current && nextSpaces.some((space) => space.id === current)) {
          return current;
        }
        return nextSpaces[0]?.id ?? null;
      });
      setSpaceError(null);
    } catch (error) {
      setSpaceError(error instanceof Error ? error.message : String(error));
    }
  }

  async function loadDeviceState() {
    try {
      const [nextConnected, nextDiscovered] = await Promise.all([
        getDeviceInfo(),
        getDiscoveredDevices(),
      ]);
      setConnectedDevices(nextConnected);
      setDiscoveredDevices(nextDiscovered);
    } catch {
      setConnectedDevices([]);
      setDiscoveredDevices([]);
    }
  }

  async function loadRootNodes(spaceId: string) {
    setRootLoading(true);
    setRootError(null);
    setSelectedNode(null);
    setDetails(null);
    setDetailsError(null);
    setPreview({ type: "welcome" });
    setExpandedPaths(new Set());
    setChildrenByPath({});
    setTreeLoadingByPath({});
    setTreeErrorByPath({});

    try {
      const nodes = await getTreeChildren(spaceId);
      setRootNodes(nodes);
    } catch (error) {
      setRootError(error instanceof Error ? error.message : String(error));
      setRootNodes([]);
    } finally {
      setRootLoading(false);
    }
  }

  async function handleBrowse() {
    setIsPicking(true);
    try {
      const path = await pickFolder();
      if (path) {
        setAddPath(path);
      }
    } catch (error) {
      setSpaceError(error instanceof Error ? error.message : String(error));
    } finally {
      setIsPicking(false);
    }
  }

  async function handleAddSpace() {
    const path = addPath.trim();
    if (!path) return;

    try {
      const created = await addSyncedSpace(path);
      setAddPath("");
      await loadSpaces();
      setSelectedSpaceId(created.id);
      setSpaceError(null);
    } catch (error) {
      setSpaceError(error instanceof Error ? error.message : String(error));
    }
  }

  async function handleRemoveSpace(spaceId: string) {
    try {
      await removeSyncedSpace(spaceId);
      await loadSpaces();
    } catch (error) {
      setSpaceError(error instanceof Error ? error.message : String(error));
    }
  }

  async function handleToggleNode(node: TreeNode) {
    if (!selectedSpaceId || node.nodeType !== "directory") {
      return;
    }

    const nextExpanded = new Set(expandedPaths);
    if (nextExpanded.has(node.relativePath)) {
      nextExpanded.delete(node.relativePath);
      setExpandedPaths(nextExpanded);
      return;
    }

    nextExpanded.add(node.relativePath);
    setExpandedPaths(nextExpanded);

    if (childrenByPath[node.relativePath]) {
      return;
    }

    setTreeLoadingByPath((current) => ({ ...current, [node.relativePath]: true }));
    setTreeErrorByPath((current) => ({ ...current, [node.relativePath]: null }));

    try {
      const children = await getTreeChildren(selectedSpaceId, node.relativePath);
      setChildrenByPath((current) => ({ ...current, [node.relativePath]: children }));
    } catch (error) {
      setTreeErrorByPath((current) => ({
        ...current,
        [node.relativePath]: error instanceof Error ? error.message : String(error),
      }));
    } finally {
      setTreeLoadingByPath((current) => ({ ...current, [node.relativePath]: false }));
    }
  }

  async function handleSelectNode(node: TreeNode) {
    if (!selectedSpaceId) {
      return;
    }

    setSelectedNode(node);
    setDetailsError(null);
    setPreview(node.nodeType === "directory" ? { type: "directory", node } : { type: "loading", node });

    try {
      const nextDetails = await getFileDetails(selectedSpaceId, node.relativePath);
      setDetails(nextDetails);
    } catch (error) {
      setDetails(null);
      setDetailsError(error instanceof Error ? error.message : String(error));
    }

    if (node.nodeType === "directory") {
      return;
    }

    const extension = node.extension?.toLowerCase() ?? "";
    if (TEXT_EXTENSIONS.has(extension)) {
      try {
        const result = await previewTextFile(selectedSpaceId, node.relativePath);
        setPreview({ type: "text", node, result });
      } catch (error) {
        setPreview({
          type: "error",
          node,
          message: error instanceof Error ? error.message : String(error),
        });
      }
      return;
    }

    if (IMAGE_EXTENSIONS.has(extension)) {
      try {
        const result = await previewImageFile(selectedSpaceId, node.relativePath);
        setPreview({ type: "image", node, result });
      } catch (error) {
        setPreview({
          type: "fallback",
          node,
          reason: error instanceof Error ? error.message : String(error),
        });
      }
      return;
    }

    setPreview({ type: "fallback", node, reason: "当前类型不支持内置预览。" });
  }

  async function handleOpenFile(relativePath: string) {
    if (!selectedSpaceId) {
      return;
    }

    try {
      await openFile(selectedSpaceId, relativePath);
    } catch (error) {
      setPreview({
        type: "error",
        node: selectedNode ?? {
          name: relativePath,
          relativePath,
          nodeType: "file",
          hasChildren: false,
          extension: null,
          size: null,
          modifiedAt: null,
        },
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }

  const selectedPath = selectedNode?.relativePath ?? null;
  const selectedSpace = useMemo(
    () => spaces.find((space) => space.id === selectedSpaceId) ?? null,
    [spaces, selectedSpaceId],
  );

  return (
    <div className="workbench-shell">
      <header className="status-bar panel">
        <div>
          <strong>设备：</strong>
          {syncStatus.deviceName}
        </div>
        <div>
          <strong>状态：</strong>
          <span className={syncStatus.syncRunning ? "status-running" : "status-stopped"}>
            {syncStatus.syncRunning ? "运行中" : "已停止"}
          </span>
        </div>
        <div>
          <strong>当前空间：</strong>
          {selectedSpace?.name ?? "未选择"}
        </div>
        <button className="secondary-button" onClick={onStopSync}>
          停止同步
        </button>
      </header>

      <main className="workbench-grid">
        <aside className="left-column">
          <SpaceList
            spaces={spaces}
            selectedSpaceId={selectedSpaceId}
            addPath={addPath}
            isPicking={isPicking}
            error={spaceError}
            onAddPathChange={setAddPath}
            onBrowse={handleBrowse}
            onAdd={handleAddSpace}
            onSelect={setSelectedSpaceId}
            onRemove={handleRemoveSpace}
          />
          <FileTree
            roots={rootNodes}
            selectedPath={selectedPath}
            expandedPaths={expandedPaths}
            childrenByPath={childrenByPath}
            treeLoadingByPath={treeLoadingByPath}
            treeErrorByPath={treeErrorByPath}
            rootLoading={rootLoading}
            rootError={rootError}
            onToggle={handleToggleNode}
            onSelect={handleSelectNode}
          />
        </aside>

        <section className="panel preview-panel">
          <div className="section-header">
            <div>
              <h2>预览</h2>
              <p>支持文本与常见图片；不支持时可直接系统打开。</p>
            </div>
          </div>
          <PreviewPane preview={preview} onOpenFallback={handleOpenFile} />
        </section>

        <DetailsPane details={details} error={detailsError} />
      </main>

      <footer className="device-status panel">
        <div className="device-block">
          <h3>已连接设备</h3>
          {connectedDevices.length === 0 ? (
            <p>暂无已连接设备</p>
          ) : (
            <ul>
              {connectedDevices.map((device) => (
                <li key={device.device_id}>
                  {device.device_name} ({device.platform})
                </li>
              ))}
            </ul>
          )}
        </div>
        <div className="device-block">
          <h3>发现设备</h3>
          {discoveredDevices.length === 0 ? (
            <p>当前未发现新的局域网设备</p>
          ) : (
            <ul>
              {discoveredDevices.map((device) => (
                <li key={device.device_id}>
                  {device.device_name} · {device.ip} · {device.platform}
                </li>
              ))}
            </ul>
          )}
        </div>
      </footer>
    </div>
  );
}
