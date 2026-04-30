import { useEffect, useMemo, useRef, useState } from "react";
import {
  addSyncedSpace,
  bindBaiduSpace,
  clearBaiduApiConfig,
  completeBaiduImplicitOAuth,
  createTreeFile,
  createTreeFolder,
  disconnectBaiduAccount,
  dismissRemoteDeletedNotice,
  dismissConflict,
  getAllSyncStatuses,
  getBaiduAccountStatus,
  getBaiduApiConfig,
  getConflictDetail,
  getConflicts,
  getFileDetails,
  getSyncDiagnostics,
  getTreeChildren,
  ignoreCloudSyncTask,
  openExternalUrl,
  openFile,
  pickFolder,
  previewImageFile,
  previewTextFile,
  removeSyncedSpace,
  resolveConflictKeepLocal,
  resolveConflictKeepRemote,
  retryCloudSyncTask,
  restoreRemoteDeletedFile,
  listSyncedSpaces,
  startSpaceSync,
  stopSpaceSync,
  saveBaiduApiConfig,
  saveTextFile,
  startBaiduOAuth,
} from "../lib/tauriClient";
import type {
  BaiduAccountStatus,
  BaiduApiConfig,
  BaiduImplicitOAuthPayload,
  ConflictDetail,
  ConflictInfo,
  FileDetails,
  PreviewState,
  SyncDiagnostics,
  SyncRuntimeStatus,
  SyncedSpace,
  TreeNode,
} from "../types/workbench";

function parseBaiduImplicitOAuthPayload(value: string, fallbackState?: string): BaiduImplicitOAuthPayload | null {
  const trimmed = value.trim();
  if (!trimmed) {
    return null;
  }

  const tokenParts = trimmed.includes("=")
    ? trimmed
        .replace(/^[^#?]*[#?]/, "")
        .split(/[&#]/)
        .filter(Boolean)
    : [`access_token=${encodeURIComponent(trimmed)}`];
  const params = new URLSearchParams(tokenParts.join("&"));
  const accessToken = params.get("access_token")?.trim();
  if (!accessToken) {
    return null;
  }

  const expiresInText = params.get("expires_in");
  const expiresIn = expiresInText ? Number.parseInt(expiresInText, 10) : null;
  return {
    accessToken,
    expiresIn: Number.isFinite(expiresIn) ? expiresIn : null,
    scope: params.get("scope"),
    state: params.get("state") ?? fallbackState ?? null,
  };
}
import { SpaceList } from "../components/sidebar/SpaceList";
import { FileTree, type TreeCreateDraft } from "../components/sidebar/FileTree";
import { PreviewPane } from "../components/preview/PreviewPane";
import { DetailsPane } from "../components/details/DetailsPane";
import { countMarkdownWords } from "../components/preview/MarkdownEditor";

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
const MARKDOWN_EXTENSIONS = new Set(["md", "markdown"]);

function runtimeStatusText(status?: SyncRuntimeStatus | null) {
  switch (status?.status) {
    case "watching":
    case "syncing":
      return "运行中";
    case "starting":
    case "indexing":
      return "启动中";
    case "error":
      return "异常";
    default:
      return "未启动";
  }
}

function taskKindText(kind: string) {
  switch (kind) {
    case "upload":
      return "上传";
    case "download":
      return "下载";
    case "mkdir":
      return "创建文件夹";
    case "delete":
      return "删除";
    default:
      return kind;
  }
}

function formatDateTime(value?: string | null) {
  if (!value) return "暂无";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString();
}

function formatLastSuccessfulSync(value?: string | null) {
  return value ? formatDateTime(value) : "尚未完成同步";
}

function syncSummaryState(
  status?: SyncRuntimeStatus | null,
  diagnostics?: SyncDiagnostics | null,
) {
  if (!status || status.status === "stopped") {
    return {
      label: "未启动",
      tone: "idle",
      message: "启动同步后会监听本仓库变化。",
    };
  }
  if (status.status === "starting" || status.status === "indexing") {
    return {
      label: "启动中",
      tone: "working",
      message: "正在准备仓库和同步基线。",
    };
  }
  if (status.status === "error" || status.lastError) {
    return {
      label: "有问题",
      tone: "issue",
      message: status.lastError ?? "同步运行时异常，请查看处理项。",
    };
  }
  const issueCount = status.cloudConflictCount + (diagnostics?.remoteDeletions.length ?? 0);
  if (issueCount > 0) {
    return {
      label: "需要处理",
      tone: "issue",
      message: `有 ${issueCount} 个问题需要处理。`,
    };
  }
  if (status.pendingCount > 0) {
    return {
      label: "同步中",
      tone: "working",
      message: `还有 ${status.pendingCount} 个任务正在同步。`,
    };
  }
  return {
    label: "已同步",
    tone: "ok",
    message: "所有文件已同步，后续变化会自动处理。",
  };
}

export function Workbench() {
  const [spaces, setSpaces] = useState<SyncedSpace[]>([]);
  const [runtimeStatusesBySpaceId, setRuntimeStatusesBySpaceId] = useState<Record<string, SyncRuntimeStatus>>({});
  const [selectedSpaceId, setSelectedSpaceId] = useState<string | null>(null);
  const [selectedSpaceConflicts, setSelectedSpaceConflicts] = useState<ConflictInfo[]>([]);
  const [conflictError, setConflictError] = useState<string | null>(null);
  const [selectedConflictId, setSelectedConflictId] = useState<number | null>(null);
  const [conflictDetail, setConflictDetail] = useState<ConflictDetail | null>(null);
  const [conflictDetailError, setConflictDetailError] = useState<string | null>(null);
  const [conflictActionError, setConflictActionError] = useState<string | null>(null);
  const [conflictActionLoading, setConflictActionLoading] = useState<
    "keep-local" | "keep-remote" | "dismiss" | null
  >(null);
  const [addPath, setAddPath] = useState("");
  const [isPicking, setIsPicking] = useState(false);
  const [spaceError, setSpaceError] = useState<string | null>(null);
  const [rootNodes, setRootNodes] = useState<TreeNode[]>([]);
  const [selectedNode, setSelectedNode] = useState<TreeNode | null>(null);
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set());
  const [childrenByPath, setChildrenByPath] = useState<Record<string, TreeNode[]>>({});
  const [treeLoadingByPath, setTreeLoadingByPath] = useState<Record<string, boolean>>({});
  const [treeErrorByPath, setTreeErrorByPath] = useState<Record<string, string | null>>({});
  const [treeCreateDraft, setTreeCreateDraft] = useState<TreeCreateDraft | null>(null);
  const [treeCreateName, setTreeCreateName] = useState("");
  const [treeCreateError, setTreeCreateError] = useState<string | null>(null);
  const [treeCreating, setTreeCreating] = useState(false);
  const [rootLoading, setRootLoading] = useState(false);
  const [rootError, setRootError] = useState<string | null>(null);
  const [details, setDetails] = useState<FileDetails | null>(null);
  const [detailsError, setDetailsError] = useState<string | null>(null);
  const [detailsOpen, setDetailsOpen] = useState(false);
  const [preview, setPreview] = useState<PreviewState>({ type: "welcome" });
  const [markdownSaveState, setMarkdownSaveState] = useState<{
    relativePath: string;
    isSaving: boolean;
    error: string | null;
  } | null>(null);
  const [markdownEditorState, setMarkdownEditorState] = useState<{
    content: string;
    isDirty: boolean;
    wordCount: number;
  } | null>(null);
  const [baiduStatus, setBaiduStatus] = useState<BaiduAccountStatus | null>(null);
  const [baiduAuthUrl, setBaiduAuthUrl] = useState("");
  const [baiduAuthCode, setBaiduAuthCode] = useState("");
  const [baiduAuthState, setBaiduAuthState] = useState("");
  const [baiduError, setBaiduError] = useState<string | null>(null);
  const [baiduLoading, setBaiduLoading] = useState(false);
  const [baiduApiConfig, setBaiduApiConfig] = useState<BaiduApiConfig | null>(null);
  const [baiduApiDeviceId, setBaiduApiDeviceId] = useState("");
  const [baiduApiClientId, setBaiduApiClientId] = useState("");
  const [baiduApiClientSecret, setBaiduApiClientSecret] = useState("");
  const [baiduApiRedirectUri, setBaiduApiRedirectUri] = useState("oob");
  const [baiduApiScopes, setBaiduApiScopes] = useState("basic netdisk");
  const [baiduConfigOpen, setBaiduConfigOpen] = useState(false);
  const [syncDiagnosticsOpen, setSyncDiagnosticsOpen] = useState(false);
  const [syncDiagnostics, setSyncDiagnostics] = useState<SyncDiagnostics | null>(null);
  const [syncDiagnosticsLoading, setSyncDiagnosticsLoading] = useState(false);
  const [syncDiagnosticsError, setSyncDiagnosticsError] = useState<string | null>(null);
  const [queueTaskActionLoading, setQueueTaskActionLoading] = useState<number | null>(null);
  const [remoteDeletionActionLoading, setRemoteDeletionActionLoading] = useState<number | null>(null);
  const [baiduConfigLoading, setBaiduConfigLoading] = useState(false);
  const [baiduConfigError, setBaiduConfigError] = useState<string | null>(null);
  const [syncActionBySpaceId, setSyncActionBySpaceId] = useState<Record<string, "start" | "stop" | undefined>>({});
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const selectedSpaceIdRef = useRef<string | null>(null);
  const expandedPathsRef = useRef<Set<string>>(new Set());

  useEffect(() => {
    selectedSpaceIdRef.current = selectedSpaceId;
  }, [selectedSpaceId]);

  useEffect(() => {
    expandedPathsRef.current = expandedPaths;
  }, [expandedPaths]);

  useEffect(() => {
    void loadSpaces();
    void loadRuntimeStatuses();
    void loadBaiduStatus();
    void loadBaiduApiConfig();

    pollRef.current = setInterval(() => {
        void loadRuntimeStatuses();
      void loadBaiduStatus();
      if (selectedSpaceId) {
        void loadConflicts(selectedSpaceId);
        if (syncDiagnosticsOpen) {
          void loadSyncDiagnostics(selectedSpaceId);
        }
        void refreshVisibleTree(selectedSpaceId);
      }
    }, 5000);

    return () => {
      if (pollRef.current) {
        clearInterval(pollRef.current);
      }
    };
  }, [selectedSpaceId, syncDiagnosticsOpen]);

  useEffect(() => {
    if (!selectedSpaceId) {
      setRootNodes([]);
      setSelectedNode(null);
      setDetails(null);
      setDetailsError(null);
      setSelectedSpaceConflicts([]);
      setConflictError(null);
      setSelectedConflictId(null);
      setConflictDetail(null);
      setConflictDetailError(null);
      setConflictActionError(null);
      setConflictActionLoading(null);
      setSyncDiagnostics(null);
      setSyncDiagnosticsError(null);
      setTreeCreateDraft(null);
      setTreeCreateName("");
      setTreeCreateError(null);
      setMarkdownSaveState(null);
      setMarkdownEditorState(null);
      setPreview({ type: "welcome" });
      return;
    }

    void Promise.all([loadRootNodes(selectedSpaceId), loadConflicts(selectedSpaceId)]);
  }, [selectedSpaceId]);

  useEffect(() => {
    if (!selectedConflictId) {
      setConflictDetail(null);
      setConflictDetailError(null);
      return;
    }
    const conflictId = selectedConflictId;

    let cancelled = false;

    async function loadSelectedConflictDetail() {
      try {
        const detail = await getConflictDetail(conflictId);
        if (cancelled) return;
        setConflictDetail(detail);
        setConflictDetailError(null);
      } catch (error) {
        if (cancelled) return;
        setConflictDetail(null);
        setConflictDetailError(error instanceof Error ? error.message : String(error));
      }
    }

    void loadSelectedConflictDetail();

    return () => {
      cancelled = true;
    };
  }, [selectedConflictId]);

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

  async function loadRuntimeStatuses() {
    try {
      const statuses = await getAllSyncStatuses();
      setRuntimeStatusesBySpaceId(
        Object.fromEntries(statuses.map((status) => [status.spaceId, status])) as Record<
          string,
          SyncRuntimeStatus
        >,
      );
    } catch (error) {
      setSpaceError(error instanceof Error ? error.message : String(error));
    }
  }


  async function loadBaiduStatus() {
    try {
      const status = await getBaiduAccountStatus();
      setBaiduStatus(status);
      setBaiduError(null);
    } catch (error) {
      setBaiduError(error instanceof Error ? error.message : String(error));
    }
  }

  async function loadBaiduApiConfig() {
    try {
      const config = await getBaiduApiConfig();
      setBaiduApiConfig(config);
      setBaiduApiDeviceId(config.deviceId ?? "");
      setBaiduApiClientId(config.clientId);
      setBaiduApiClientSecret(config.clientSecret ?? "");
      setBaiduApiRedirectUri(config.redirectUri);
      setBaiduApiScopes(config.scopes.join(" "));
      setBaiduConfigError(null);
    } catch (error) {
      setBaiduConfigError(error instanceof Error ? error.message : String(error));
    }
  }

  async function loadConflicts(spaceId: string) {
    try {
      const conflicts = await getConflicts(spaceId);
      setSelectedSpaceConflicts(conflicts);
      setConflictError(null);
      setSelectedConflictId((current) => {
        if (current && conflicts.some((conflict) => conflict.id === current)) {
          return current;
        }
        return conflicts[0]?.id ?? null;
      });
    } catch (error) {
      setSelectedSpaceConflicts([]);
      setConflictError(error instanceof Error ? error.message : String(error));
      setSelectedConflictId(null);
      setConflictDetail(null);
      setConflictDetailError(null);
    }
  }

  async function loadRootNodes(spaceId: string) {
    setRootLoading(true);
    setRootError(null);
    setSelectedNode(null);
    setDetails(null);
    setDetailsError(null);
    setConflictActionError(null);
    setPreview({ type: "welcome" });
    setExpandedPaths(new Set());
    setChildrenByPath({});
    setTreeLoadingByPath({});
    setTreeErrorByPath({});
    setTreeCreateDraft(null);
    setTreeCreateName("");
    setTreeCreateError(null);

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

  async function refreshVisibleTree(spaceId: string) {
    const expandedPathsSnapshot = Array.from(expandedPathsRef.current);

    try {
      const roots = await getTreeChildren(spaceId);
      const childResults = await Promise.all(
        expandedPathsSnapshot.map(async (relativePath) => {
          try {
            const children = await getTreeChildren(spaceId, relativePath);
            return { relativePath, children, error: null as string | null };
          } catch (error) {
            return {
              relativePath,
              children: null,
              error: error instanceof Error ? error.message : String(error),
            };
          }
        }),
      );

      if (selectedSpaceIdRef.current !== spaceId) {
        return;
      }

      setRootNodes(roots);
      setRootError(null);
      setChildrenByPath((current) => {
        const next = { ...current };
        for (const result of childResults) {
          if (result.children) {
            next[result.relativePath] = result.children;
          }
        }
        return next;
      });
      setTreeErrorByPath((current) => {
        const next = { ...current };
        for (const result of childResults) {
          next[result.relativePath] = result.error;
        }
        return next;
      });
    } catch (error) {
      if (selectedSpaceIdRef.current !== spaceId) {
        return;
      }

      setRootError(error instanceof Error ? error.message : String(error));
    }
  }

  async function handleSaveBaiduApiConfig() {
    setBaiduConfigLoading(true);
    setBaiduConfigError(null);

    try {
      const config = await saveBaiduApiConfig({
        deviceId: baiduApiDeviceId || null,
        clientId: baiduApiClientId,
        clientSecret: baiduApiClientSecret || null,
        redirectUri: baiduApiRedirectUri || null,
        scopes: baiduApiScopes
          .split(/[\s,]+/)
          .map((scope) => scope.trim())
          .filter(Boolean),
      });
      setBaiduApiConfig(config);
      setBaiduApiClientSecret(config.clientSecret ?? "");
      await loadBaiduStatus();
    } catch (error) {
      setBaiduConfigError(error instanceof Error ? error.message : String(error));
    } finally {
      setBaiduConfigLoading(false);
    }
  }

  async function handleClearBaiduApiConfig() {
    setBaiduConfigLoading(true);
    setBaiduConfigError(null);

    try {
      const config = await clearBaiduApiConfig();
      setBaiduApiConfig(config);
      setBaiduApiDeviceId(config.deviceId ?? "");
      setBaiduApiClientId(config.clientId);
      setBaiduApiClientSecret("");
      setBaiduApiRedirectUri(config.redirectUri);
      setBaiduApiScopes(config.scopes.join(" "));
    } catch (error) {
      setBaiduConfigError(error instanceof Error ? error.message : String(error));
    } finally {
      setBaiduConfigLoading(false);
    }
  }

  async function handleStartBaiduOAuth() {
    setBaiduLoading(true);
    setBaiduError(null);

    try {
      const result = await startBaiduOAuth();
      setBaiduAuthUrl(result.authorizationUrl);
      setBaiduAuthState(result.state);
      await navigator.clipboard?.writeText(result.authorizationUrl);
      await openExternalUrl(result.authorizationUrl);
    } catch (error) {
      setBaiduError(error instanceof Error ? error.message : String(error));
    } finally {
      setBaiduLoading(false);
    }
  }

  async function handleCompleteBaiduOAuth() {
    const payload = parseBaiduImplicitOAuthPayload(baiduAuthCode, baiduAuthState || undefined);
    if (!payload) {
      setBaiduError("请粘贴百度授权返回的 access_token，或包含 access_token 的完整地址。");
      return;
    }

    setBaiduLoading(true);
    setBaiduError(null);

    try {
      const result = await completeBaiduImplicitOAuth(payload);
      setBaiduStatus(result.status);
      setBaiduAuthCode("");
      setBaiduAuthUrl("");
      setBaiduAuthState("");
      await Promise.all([loadSpaces(), loadRuntimeStatuses()]);
    } catch (error) {
      setBaiduError(error instanceof Error ? error.message : String(error));
    } finally {
      setBaiduLoading(false);
    }
  }

  async function handleDisconnectBaidu() {
    setBaiduLoading(true);
    setBaiduError(null);

    try {
      await disconnectBaiduAccount();
      await loadBaiduStatus();
    } catch (error) {
      setBaiduError(error instanceof Error ? error.message : String(error));
    } finally {
      setBaiduLoading(false);
    }
  }

  async function handleBindBaiduSpace(spaceId: string) {
    try {
      const updated = await bindBaiduSpace(spaceId);
      await Promise.all([loadSpaces(), loadRuntimeStatuses()]);
      setSelectedSpaceId(updated.id);
      setSpaceError(null);
    } catch (error) {
      setSpaceError(error instanceof Error ? error.message : String(error));
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
      await Promise.all([loadSpaces(), loadRuntimeStatuses()]);
      setSelectedSpaceId(created.id);
      setSpaceError(null);
    } catch (error) {
      setSpaceError(error instanceof Error ? error.message : String(error));
    }
  }


  async function handleRemoveSpace(spaceId: string) {
    try {
      await removeSyncedSpace(spaceId);
      await Promise.all([loadSpaces(), loadRuntimeStatuses()]);
    } catch (error) {
      setSpaceError(error instanceof Error ? error.message : String(error));
    }
  }

  async function handleStartSpace(spaceId: string) {
    setSyncActionBySpaceId((current) => ({ ...current, [spaceId]: "start" }));
    try {
      await startSpaceSync(spaceId);
      await Promise.all([loadRuntimeStatuses(), loadConflicts(spaceId)]);
      setSpaceError(null);
    } catch (error) {
      setSpaceError(error instanceof Error ? error.message : String(error));
    } finally {
      setSyncActionBySpaceId((current) => {
        const next = { ...current };
        delete next[spaceId];
        return next;
      });
    }
  }

  async function loadSyncDiagnostics(spaceId = selectedSpaceId) {
    if (!spaceId) return;
    setSyncDiagnosticsLoading(true);
    try {
      const diagnostics = await getSyncDiagnostics(spaceId);
      setSyncDiagnostics(diagnostics);
      setSyncDiagnosticsError(null);
    } catch (error) {
      setSyncDiagnosticsError(error instanceof Error ? error.message : String(error));
    } finally {
      setSyncDiagnosticsLoading(false);
    }
  }

  async function handleQueueTaskAction(taskId: number, action: "retry" | "ignore") {
    if (!selectedSpaceId) return;
    setQueueTaskActionLoading(taskId);
    setSyncDiagnosticsError(null);
    try {
      if (action === "retry") {
        await retryCloudSyncTask(selectedSpaceId, taskId);
      } else {
        await ignoreCloudSyncTask(selectedSpaceId, taskId);
      }
      await Promise.all([loadSyncDiagnostics(selectedSpaceId), loadRuntimeStatuses()]);
    } catch (error) {
      setSyncDiagnosticsError(error instanceof Error ? error.message : String(error));
    } finally {
      setQueueTaskActionLoading(null);
    }
  }

  async function handleRemoteDeletionAction(noticeId: number, action: "restore" | "dismiss") {
    if (!selectedSpaceId) return;
    setRemoteDeletionActionLoading(noticeId);
    setSyncDiagnosticsError(null);
    try {
      if (action === "restore") {
        await restoreRemoteDeletedFile(selectedSpaceId, noticeId);
      } else {
        await dismissRemoteDeletedNotice(selectedSpaceId, noticeId);
      }
      await Promise.all([loadSyncDiagnostics(selectedSpaceId), loadRuntimeStatuses(), loadConflicts(selectedSpaceId)]);
    } catch (error) {
      setSyncDiagnosticsError(error instanceof Error ? error.message : String(error));
    } finally {
      setRemoteDeletionActionLoading(null);
    }
  }

  async function handleStopSpace(spaceId: string) {
    setSyncActionBySpaceId((current) => ({ ...current, [spaceId]: "stop" }));
    try {
      await stopSpaceSync(spaceId);
      await Promise.all([loadRuntimeStatuses(), loadConflicts(spaceId)]);
      setSpaceError(null);
    } catch (error) {
      setSpaceError(error instanceof Error ? error.message : String(error));
    } finally {
      setSyncActionBySpaceId((current) => {
        const next = { ...current };
        delete next[spaceId];
        return next;
      });
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
    setConflictActionError(null);
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
        setMarkdownSaveState(null);
        setPreview({
          type: MARKDOWN_EXTENSIONS.has(extension) ? "markdown" : "text",
          node,
          result,
        });
        setMarkdownEditorState(
          MARKDOWN_EXTENSIONS.has(extension)
            ? {
                content: result.content,
                isDirty: false,
                wordCount: countMarkdownWords(result.content),
              }
            : null,
        );
      } catch (error) {
        setPreview({
          type: "error",
          node,
          message: error instanceof Error ? error.message : String(error),
        });
      }
      return;
    }

    setMarkdownEditorState(null);
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

    setPreview({ type: "fallback", node, reason: "当前类型暂不支持内置预览。" });
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

  function parentPathForCreate() {
    if (!selectedNode) return null;
    if (selectedNode.nodeType === "directory") return selectedNode.relativePath;
    const parts = selectedNode.relativePath.split("/");
    parts.pop();
    return parts.length > 0 ? parts.join("/") : null;
  }

  function handleStartTreeCreate(parentRelativePath: string | null, kind: "file" | "folder") {
    setTreeCreateDraft({ parentRelativePath, kind });
    setTreeCreateName("");
    setTreeCreateError(null);
  }

  function handleStartCreateFromSelection(kind: "file" | "folder") {
    handleStartTreeCreate(parentPathForCreate(), kind);
  }

  function normalizeCreateName(kind: "file" | "folder", value: string) {
    const trimmed = value.trim();
    if (!trimmed) return trimmed;
    if (kind === "file" && !trimmed.includes(".")) {
      return `${trimmed}.md`;
    }
    return trimmed;
  }

  async function refreshTreeParent(spaceId: string, parentRelativePath: string | null) {
    if (parentRelativePath) {
      const children = await getTreeChildren(spaceId, parentRelativePath);
      setChildrenByPath((current) => ({ ...current, [parentRelativePath]: children }));
      setTreeErrorByPath((current) => ({ ...current, [parentRelativePath]: null }));
      setExpandedPaths((current) => new Set(current).add(parentRelativePath));
      return;
    }
    const roots = await getTreeChildren(spaceId);
    setRootNodes(roots);
    setRootError(null);
  }

  async function handleCommitTreeCreate() {
    if (!selectedSpaceId || !treeCreateDraft || treeCreating) return;
    const name = normalizeCreateName(treeCreateDraft.kind, treeCreateName);
    if (!name) {
      setTreeCreateError("请输入名称");
      return;
    }

    setTreeCreating(true);
    setTreeCreateError(null);
    try {
      const created =
        treeCreateDraft.kind === "file"
          ? await createTreeFile(selectedSpaceId, treeCreateDraft.parentRelativePath, name)
          : await createTreeFolder(selectedSpaceId, treeCreateDraft.parentRelativePath, name);
      await refreshTreeParent(selectedSpaceId, treeCreateDraft.parentRelativePath);
      setTreeCreateDraft(null);
      setTreeCreateName("");
      setTreeCreateError(null);
      if (created.nodeType === "file") {
        await handleSelectNode(created);
      } else {
        setExpandedPaths((current) => new Set(current).add(created.relativePath));
        setSelectedNode(created);
        setPreview({ type: "directory", node: created });
      }
      await loadRuntimeStatuses();
    } catch (error) {
      setTreeCreateError(error instanceof Error ? error.message : String(error));
    } finally {
      setTreeCreating(false);
    }
  }

  function handleCancelTreeCreate() {
    if (treeCreating) return;
    setTreeCreateDraft(null);
    setTreeCreateName("");
    setTreeCreateError(null);
  }

  async function handleSaveMarkdown(relativePath: string, content: string) {
    if (!selectedSpaceId || !selectedNode || selectedNode.relativePath !== relativePath) {
      return;
    }
    setMarkdownSaveState({ relativePath, isSaving: true, error: null });
    try {
      const result = await saveTextFile(selectedSpaceId, relativePath, content);
      setDetails(result.details);
      setPreview((current) => {
        if (
          current.type !== "markdown" ||
          current.node.relativePath !== relativePath
        ) {
          return current;
        }
        return {
          type: "markdown",
          node: {
            ...current.node,
            size: result.details.size,
            modifiedAt: result.details.modifiedAt,
          },
          result: {
            ...current.result,
            content,
            size: result.details.size,
            truncated: false,
          },
        };
      });
      setSelectedNode((current) =>
        current && current.relativePath === relativePath
          ? {
              ...current,
              size: result.details.size,
              modifiedAt: result.details.modifiedAt,
            }
          : current,
      );
      await Promise.all([refreshVisibleTree(selectedSpaceId), loadRuntimeStatuses()]);
      setMarkdownSaveState({ relativePath, isSaving: false, error: null });
      setMarkdownEditorState((current) =>
        current
          ? {
              ...current,
              content,
              isDirty: false,
              wordCount: countMarkdownWords(content),
            }
          : current,
      );
    } catch (error) {
      setMarkdownSaveState({
        relativePath,
        isSaving: false,
        error: error instanceof Error ? error.message : String(error),
      });
      throw error;
    }
  }

  async function refreshAfterConflictAction(spaceId: string, relativePath?: string) {
    await Promise.all([loadConflicts(spaceId), loadRuntimeStatuses(), refreshVisibleTree(spaceId)]);

    if (selectedNode && relativePath && selectedNode.relativePath === relativePath) {
      await handleSelectNode(selectedNode);
    }
  }

  async function handleResolveConflict(
    mode: "keep-local" | "keep-remote" | "dismiss",
    conflictId: number,
  ) {
    if (!selectedSpaceId) {
      return;
    }

    const activeConflict = selectedSpaceConflicts.find((conflict) => conflict.id === conflictId);
    setConflictActionError(null);
    setConflictActionLoading(mode);

    try {
      if (mode === "keep-local") {
        await resolveConflictKeepLocal(conflictId);
      } else if (mode === "keep-remote") {
        await resolveConflictKeepRemote(conflictId);
      } else {
        await dismissConflict(conflictId);
      }

      await refreshAfterConflictAction(selectedSpaceId, activeConflict?.relativePath);
    } catch (error) {
      setConflictActionError(error instanceof Error ? error.message : String(error));
    } finally {
      setConflictActionLoading(null);
    }
  }

  const selectedPath = selectedNode?.relativePath ?? null;
  const selectedSpace = useMemo(
    () => spaces.find((space) => space.id === selectedSpaceId) ?? null,
    [spaces, selectedSpaceId],
  );
  const selectedRuntime = selectedSpaceId ? runtimeStatusesBySpaceId[selectedSpaceId] ?? null : null;
  const baiduConnected = Boolean(baiduStatus?.connected && !baiduStatus.reconnectRequired);
  const baiduStatusText = baiduConnected
    ? baiduStatus?.displayName ?? baiduStatus?.accountId ?? "已连接"
    : baiduStatus?.reconnectRequired
      ? "需要重新连接"
      : "未连接";
  const baiduConfigSourceText =
    baiduApiConfig?.source === "local" ? "客户端配置" : baiduApiConfig?.source === "env" ? "环境变量" : "未配置";
  const syncSummary = syncSummaryState(selectedRuntime, syncDiagnostics);
  const actionItemCount =
    (selectedRuntime?.cloudConflictCount ?? 0) + (syncDiagnostics?.remoteDeletions.length ?? 0);
  const failedSyncTasks = (syncDiagnostics?.queue ?? []).filter((task) => Boolean(task.lastError));
  const activeSyncTasks = (syncDiagnostics?.queue ?? []).filter((task) => !task.lastError);
  const pendingSyncCount = selectedRuntime?.pendingCount ?? syncDiagnostics?.queue.length ?? 0;
  const cloudPath = syncDiagnostics?.cloudRemotePath ?? selectedRuntime?.cloudRemotePath ?? selectedSpace?.cloudBinding?.remoteRootPath ?? null;
  const isMarkdownPreview = preview.type === "markdown";
  const markdownHeaderState =
    isMarkdownPreview && markdownEditorState
      ? markdownEditorState
      : isMarkdownPreview
        ? {
            content: preview.result.content,
            isDirty: false,
            wordCount: countMarkdownWords(preview.result.content),
          }
        : null;
  const activeMarkdownPath = isMarkdownPreview ? preview.node.relativePath : null;
  const activeMarkdownSaveState =
    activeMarkdownPath && markdownSaveState?.relativePath === activeMarkdownPath
      ? markdownSaveState
      : null;

  return (
    <div className="workbench-shell">
      <header className="panel workspace-topbar">
        <div className="workspace-title">
          <strong>{selectedSpace?.name ?? "未选择仓库"}</strong>
          <span>{selectedSpace?.rootPath ?? "使用左下角仓库切换器添加或打开同步文件夹。"}</span>
        </div>
        <div className="workspace-status-strip">
          <span className={`space-status-badge status-${selectedRuntime?.status ?? "stopped"}`}>
            {runtimeStatusText(selectedRuntime)}
          </span>
          <span>文件 {selectedRuntime?.fileCount ?? 0}</span>
          <span>队列 {selectedRuntime?.pendingCount ?? 0}</span>
          <span>云冲突 {selectedRuntime?.cloudConflictCount ?? 0}</span>
          <span className={baiduConnected ? "topbar-cloud connected" : "topbar-cloud"}>
            网盘：{baiduStatusText}
          </span>
        </div>
        <div className="workspace-action-group">
          <button
            type="button"
            className="secondary-button secondary-button-compact cloud-settings-button"
            onClick={() => {
              if (selectedSpaceId) {
                setSyncDiagnosticsOpen(true);
                void loadSyncDiagnostics(selectedSpaceId);
              }
            }}
            disabled={!selectedSpaceId}
          >
            同步详情
          </button>
          <button
            type="button"
            className="secondary-button secondary-button-compact cloud-settings-button"
            onClick={() => setBaiduConfigOpen(true)}
            title={`百度网盘配置来源：${baiduConfigSourceText}`}
          >
            网盘配置
          </button>
        </div>
      </header>

      {syncDiagnosticsOpen ? (
        <div className="settings-overlay" role="presentation" onMouseDown={() => setSyncDiagnosticsOpen(false)}>
          <aside
            className="panel cloud-settings-drawer sync-diagnostics-drawer"
            role="dialog"
            aria-modal="true"
            aria-label="同步详情"
            onMouseDown={(event) => event.stopPropagation()}
          >
            <div className="cloud-settings-header">
              <div>
                <span>当前仓库</span>
                <strong>同步详情</strong>
                <small>日常只看同步状态、最近同步时间和是否有待处理问题。</small>
              </div>
              <button
                type="button"
                className="icon-button"
                onClick={() => setSyncDiagnosticsOpen(false)}
                aria-label="关闭同步详情"
                title="关闭"
              >
                x
              </button>
            </div>

            <div className="cloud-settings-stack sync-status-stack">
              <section className={`cloud-settings-card sync-overview-card sync-overview-${syncSummary.tone}`}>
                <div className="settings-card-title">
                  <div>
                    <span>{syncDiagnostics?.spaceName ?? selectedSpace?.name ?? "未选择仓库"}</span>
                    <strong>{syncSummary.label}</strong>
                  </div>
                  <button
                    type="button"
                    className="secondary-button secondary-button-compact"
                    onClick={() => void loadSyncDiagnostics()}
                    disabled={syncDiagnosticsLoading || !selectedSpaceId}
                  >
                    {syncDiagnosticsLoading ? "刷新中" : "刷新"}
                  </button>
                </div>
                {syncDiagnosticsError ? <div className="error-banner error-banner-compact">{syncDiagnosticsError}</div> : null}
                <p>{syncSummary.message}</p>
                <div className="sync-overview-grid">
                  <span>
                    最近同步
                    <strong>{formatLastSuccessfulSync(syncDiagnostics?.summary.lastSuccessfulSyncAt)}</strong>
                  </span>
                  <span>
                    本地文件
                    <strong>{selectedRuntime?.fileCount ?? syncDiagnostics?.manifest?.fileCount ?? 0}</strong>
                  </span>
                  <span>
                    待处理
                    <strong>{pendingSyncCount}</strong>
                  </span>
                  <span>
                    异常
                    <strong>{actionItemCount + failedSyncTasks.length}</strong>
                  </span>
                </div>
                <div className="sync-overview-paths">
                  <span>最近活动：{formatDateTime(syncDiagnostics?.summary.lastSyncActivityAt ?? selectedRuntime?.lastIndexedAt)}</span>
                  <span>本地仓库：{syncDiagnostics?.rootPath ?? selectedSpace?.rootPath ?? "未选择"}</span>
                  <span>百度网盘：{cloudPath ?? "未绑定"}</span>
                  {syncDiagnostics?.summary.lastSyncError ? (
                    <span title={syncDiagnostics.summary.lastSyncError}>
                      最近错误：{formatDateTime(syncDiagnostics.summary.lastSyncErrorAt)} · {syncDiagnostics.summary.lastSyncError}
                    </span>
                  ) : null}
                </div>
              </section>

              <section className="cloud-settings-card sync-diagnostics-card">
                <div className="settings-card-title">
                  <div>
                    <span>当前还没完成的同步动作</span>
                    <strong>正在同步</strong>
                  </div>
                </div>
                {activeSyncTasks.length ? (
                  <div className="sync-task-list">
                    {activeSyncTasks.slice(0, 8).map((task) => (
                      <div className="sync-task-row" key={task.id}>
                        <span className="sync-task-kind">{taskKindText(task.taskKind)}</span>
                        <strong title={task.localRelativePath}>{task.localRelativePath || "(根目录)"}</strong>
                        <small title={task.remotePath}>{task.remotePath}</small>
                        <span>更新时间 {formatDateTime(task.updatedAt)}</span>
                      </div>
                    ))}
                  </div>
                ) : (
                  <p className="sync-empty-text">当前没有等待同步的文件。</p>
                )}
              </section>

              <section className="cloud-settings-card sync-diagnostics-card">
                <div className="settings-card-title">
                  <div>
                    <span>需要你决定如何处理</span>
                    <strong>待处理问题</strong>
                  </div>
                </div>
                {failedSyncTasks.length || syncDiagnostics?.remoteDeletions.length || syncDiagnostics?.conflicts.length ? (
                  <div className="sync-task-list">
                    {failedSyncTasks.map((task) => (
                      <div className="sync-task-row sync-remote-delete-row" key={`failed-${task.id}`}>
                        <span className="sync-task-kind sync-task-warning">失败</span>
                        <strong title={task.localRelativePath}>{task.localRelativePath || "(根目录)"}</strong>
                        <small title={task.lastError ?? undefined}>{task.lastError}</small>
                        <span className="sync-task-actions">
                          <button
                            type="button"
                            className="secondary-button secondary-button-compact"
                            onClick={() => void handleQueueTaskAction(task.id, "retry")}
                            disabled={queueTaskActionLoading === task.id}
                          >
                            {queueTaskActionLoading === task.id ? "处理中" : "重试"}
                          </button>
                          <button
                            type="button"
                            className="ghost-danger-button"
                            onClick={() => void handleQueueTaskAction(task.id, "ignore")}
                            disabled={queueTaskActionLoading === task.id}
                          >
                            忽略
                          </button>
                        </span>
                      </div>
                    ))}
                    {(syncDiagnostics?.remoteDeletions ?? []).map((notice) => (
                      <div className="sync-task-row sync-remote-delete-row" key={`remote-${notice.id}`}>
                        <span className="sync-task-kind sync-task-warning">云端缺失</span>
                        <strong title={notice.relativePath}>{notice.relativePath}</strong>
                        <small>发现时间 {formatDateTime(notice.detectedAt)}</small>
                        <span>本地文件已保留，不会被自动删除。</span>
                        <span className="sync-task-actions">
                          <button
                            type="button"
                            className="secondary-button secondary-button-compact"
                            onClick={() => void handleRemoteDeletionAction(notice.id, "restore")}
                            disabled={remoteDeletionActionLoading === notice.id}
                          >
                            {remoteDeletionActionLoading === notice.id ? "处理中" : "重新上传"}
                          </button>
                          <button
                            type="button"
                            className="ghost-danger-button"
                            onClick={() => void handleRemoteDeletionAction(notice.id, "dismiss")}
                            disabled={remoteDeletionActionLoading === notice.id}
                          >
                            标记已处理
                          </button>
                        </span>
                      </div>
                    ))}
                    {(syncDiagnostics?.conflicts ?? []).map((conflict) => (
                      <div className="sync-task-row sync-remote-delete-row" key={`conflict-${conflict.id}`}>
                        <span className="sync-task-kind sync-task-warning">冲突</span>
                        <strong title={conflict.relativePath}>{conflict.relativePath}</strong>
                        <small>来源：{conflict.remoteDevice}</small>
                        <span>发现时间 {formatDateTime(conflict.detectedAt)}</span>
                        <span className="sync-task-actions">
                          <button
                            type="button"
                            className="secondary-button secondary-button-compact"
                            onClick={() => void handleResolveConflict("keep-local", conflict.id)}
                            disabled={conflictActionLoading !== null}
                          >
                            {conflictActionLoading === "keep-local" ? "处理中" : "保留本地"}
                          </button>
                          <button
                            type="button"
                            className="secondary-button secondary-button-compact"
                            onClick={() => void handleResolveConflict("keep-remote", conflict.id)}
                            disabled={conflictActionLoading !== null}
                          >
                            {conflictActionLoading === "keep-remote" ? "处理中" : "保留云端"}
                          </button>
                          <button
                            type="button"
                            className="ghost-danger-button"
                            onClick={() => void handleResolveConflict("dismiss", conflict.id)}
                            disabled={conflictActionLoading !== null}
                          >
                            暂时忽略
                          </button>
                        </span>
                      </div>
                    ))}
                  </div>
                ) : (
                  <p className="sync-empty-text">没有需要处理的问题。</p>
                )}
              </section>

              <section className="cloud-settings-card sync-diagnostics-card">
                <div className="settings-card-title">
                  <div>
                    <span>同步范围</span>
                    <strong>仓库位置</strong>
                  </div>
                </div>
                <div className="sync-overview-paths">
                  <span title={syncDiagnostics?.rootPath ?? selectedSpace?.rootPath}>本地：{syncDiagnostics?.rootPath ?? selectedSpace?.rootPath ?? "未选择"}</span>
                  <span title={cloudPath ?? undefined}>云端：{cloudPath ?? "未绑定百度网盘"}</span>
                  <span>状态：{runtimeStatusText(selectedRuntime)}</span>
                </div>
              </section>
            </div>
          </aside>
        </div>
      ) : null}

      {baiduConfigOpen ? (
        <div className="settings-overlay" role="presentation" onMouseDown={() => setBaiduConfigOpen(false)}>
          <aside
            className="panel cloud-settings-drawer"
            role="dialog"
            aria-modal="true"
            aria-label="百度网盘设置"
            onMouseDown={(event) => event.stopPropagation()}
          >
            <div className="cloud-settings-header">
              <div>
                <span>独立配置</span>
                <strong>百度网盘</strong>
                <small>账号授权、API 参数和明文云同步说明集中在这里。</small>
              </div>
              <button
                type="button"
                className="icon-button"
                onClick={() => setBaiduConfigOpen(false)}
                aria-label="关闭百度网盘设置"
                title="关闭"
              >
                x
              </button>
            </div>

            <div className="cloud-settings-stack">
              <section className="cloud-settings-card cloud-account-card">
                <div className="settings-card-title">
                  <div>
                    <span>账号状态</span>
                    <strong className={baiduConnected ? "status-running" : "status-stopped"}>{baiduStatusText}</strong>
                  </div>
                  <span className={baiduConnected ? "cloud-state-dot connected" : "cloud-state-dot"} />
                </div>
                <div className="cloud-account-meta">
                  <span>API 来源：{baiduConfigSourceText}</span>
                  <span>权限范围：{baiduApiScopes || "basic netdisk"}</span>
                </div>
                <div className="cloud-sync-note" title="百度网盘云端文件不加密，固定限制在 /apps/SyncFlow 下。">
                  百度网盘同步使用官方开放平台 API。云端文件以明文保存，并限制在 /apps/SyncFlow 下。
                  {baiduError ? <span>{baiduError}</span> : null}
                </div>
                <div className="baidu-connect-controls cloud-settings-actions">
                  <button
                    type="button"
                    className="primary-button primary-button-compact"
                    onClick={handleStartBaiduOAuth}
                    disabled={baiduLoading}
                    title="打开百度简化模式授权地址，授权后将返回地址或 access_token 粘贴到这里。"
                  >
                    {baiduConnected ? "重新连接" : "连接网盘"}
                  </button>
                  {baiduAuthUrl ? (
                    <input
                      value={baiduAuthUrl}
                      readOnly
                      title="完整授权地址，浏览器未正确打开时可手动复制"
                      onFocus={(event) => event.currentTarget.select()}
                    />
                  ) : null}
                  {baiduAuthUrl ? (
                    <input
                      value={baiduAuthCode}
                      onChange={(event) => setBaiduAuthCode(event.target.value)}
                      placeholder="粘贴 access_token 或完整返回地址"
                      title={baiduAuthUrl}
                      onKeyDown={(event) => {
                        if (event.key === "Enter") {
                          void handleCompleteBaiduOAuth();
                        }
                      }}
                    />
                  ) : null}
                  {baiduAuthUrl ? (
                    <button
                      type="button"
                      className="secondary-button secondary-button-compact"
                      onClick={handleCompleteBaiduOAuth}
                      disabled={baiduLoading}
                    >
                      完成授权
                    </button>
                  ) : null}
                  {baiduStatus?.connected ? (
                    <button
                      type="button"
                      className="secondary-button secondary-button-compact"
                      onClick={handleDisconnectBaidu}
                      disabled={baiduLoading}
                    >
                      断开连接
                    </button>
                  ) : null}
                </div>
              </section>

              <section className="cloud-settings-card baidu-api-config-panel">
                <div className="baidu-api-config-header">
                  <div>
                    <strong>开放平台 API</strong>
                    <span>当前来源：{baiduConfigSourceText}</span>
                  </div>
                </div>
                <div className="baidu-api-config-grid">
                  <label>
                    AppID / Device ID
                    <input
                      value={baiduApiDeviceId}
                      onChange={(event) => setBaiduApiDeviceId(event.target.value)}
                      placeholder="百度网盘开放平台应用 AppID"
                    />
                  </label>
                  <label>
                    API Key / Client ID
                    <input
                      value={baiduApiClientId}
                      onChange={(event) => setBaiduApiClientId(event.target.value)}
                      placeholder="百度开放平台应用 API Key"
                    />
                  </label>
                  <label>
                    Secret Key / Client Secret
                    <input
                      value={baiduApiClientSecret}
                      onChange={(event) => setBaiduApiClientSecret(event.target.value)}
                      placeholder={baiduApiConfig?.hasClientSecret ? "已保存，留空可清除" : "可选"}
                      type="password"
                    />
                  </label>
                  <label>
                    回调地址
                    <input
                      value={baiduApiRedirectUri}
                      onChange={(event) => setBaiduApiRedirectUri(event.target.value)}
                      placeholder="oob"
                    />
                  </label>
                  <label>
                    Scopes
                    <input
                      value={baiduApiScopes}
                      onChange={(event) => setBaiduApiScopes(event.target.value)}
                      placeholder="basic netdisk"
                    />
                  </label>
                </div>
                <div className="baidu-api-config-actions">
                  {baiduConfigError ? <span className="baidu-config-error">{baiduConfigError}</span> : null}
                  <button
                    type="button"
                    className="secondary-button secondary-button-compact"
                    onClick={handleClearBaiduApiConfig}
                    disabled={baiduConfigLoading}
                  >
                    清除配置
                  </button>
                  <button
                    type="button"
                    className="primary-button primary-button-compact"
                    onClick={handleSaveBaiduApiConfig}
                    disabled={baiduConfigLoading}
                  >
                    保存配置
                  </button>
                </div>
              </section>

              <section className="cloud-settings-card cloud-policy-card">
                <strong>同步范围</strong>
                <span>新绑定的仓库会使用 /apps/SyncFlow 下的应用目录。</span>
                <span>云端文件保持明文，便于在百度网盘客户端和网页端直接查看。</span>
                <span>仓库绑定和切换请从左下角仓库管理入口操作。</span>
              </section>
            </div>
          </aside>
        </div>
      ) : null}

      <main className={detailsOpen ? "workbench-grid details-open" : "workbench-grid"}>
        <aside className="left-column">
          <FileTree
            roots={rootNodes}
            selectedPath={selectedPath}
            expandedPaths={expandedPaths}
            childrenByPath={childrenByPath}
            treeLoadingByPath={treeLoadingByPath}
            treeErrorByPath={treeErrorByPath}
            rootLoading={rootLoading}
            rootError={rootError}
            createDraft={treeCreateDraft}
            createName={treeCreateName}
            createError={treeCreateError}
            creating={treeCreating}
            onToggle={handleToggleNode}
            onSelect={handleSelectNode}
            onStartCreate={(parentRelativePath, kind) => {
              if (parentRelativePath === null) {
                handleStartCreateFromSelection(kind);
              } else {
                handleStartTreeCreate(parentRelativePath, kind);
              }
            }}
            onCreateNameChange={setTreeCreateName}
            onCommitCreate={() => void handleCommitTreeCreate()}
            onCancelCreate={handleCancelTreeCreate}
          />
          <SpaceList
            spaces={spaces}
            statusesBySpaceId={runtimeStatusesBySpaceId}
            selectedSpaceId={selectedSpaceId}
            addPath={addPath}
            isPicking={isPicking}
            error={spaceError}
            onAddPathChange={setAddPath}
            onBrowse={handleBrowse}
            onAdd={handleAddSpace}
            onSelect={setSelectedSpaceId}
            onRemove={handleRemoveSpace}
            onStartSync={handleStartSpace}
            onStopSync={handleStopSpace}
            onBindBaiduSpace={handleBindBaiduSpace}
            canBindBaidu={baiduConnected}
            syncActionBySpaceId={syncActionBySpaceId}
          />
        </aside>

        <section className="panel preview-panel">
          <div className="section-header preview-header compact-header">
            <h2>预览</h2>
            <span
              className="preview-path"
              title={selectedNode ? selectedNode.relativePath || selectedNode.name : undefined}
            >
              {selectedNode ? selectedNode.relativePath || selectedNode.name : "选择文件后在这里查看内容"}
            </span>
            {isMarkdownPreview ? (
              <div className="preview-markdown-actions">
                <span className={markdownHeaderState?.isDirty ? "markdown-save-state dirty" : "markdown-save-state"}>
                  {activeMarkdownSaveState?.isSaving
                    ? "自动保存中"
                    : markdownHeaderState?.isDirty
                      ? "等待自动保存"
                      : "已自动保存"}
                </span>
                <span className="preview-word-count">字数 {markdownHeaderState?.wordCount ?? 0}</span>
              </div>
            ) : null}
            <button
              type="button"
              className="details-toggle"
              onClick={() => setDetailsOpen((current) => !current)}
              aria-label={detailsOpen ? "收起详情栏" : "展开详情栏"}
              title={detailsOpen ? "收起详情栏" : "展开详情栏"}
              aria-pressed={detailsOpen}
            >
              <svg viewBox="0 0 20 20" aria-hidden="true" className="details-toggle-icon">
                <path d="M3.5 4.5A1.5 1.5 0 0 1 5 3h10a1.5 1.5 0 0 1 1.5 1.5v11A1.5 1.5 0 0 1 15 17H5a1.5 1.5 0 0 1-1.5-1.5v-11Z" />
                <path d="M11.5 3v14" />
                <path d={detailsOpen ? "m8.5 10 2-2v4l-2-2Z" : "m13 10-2-2v4l2-2Z"} />
              </svg>
            </button>
          </div>
          <div className="preview-content">
            <PreviewPane
              preview={preview}
              onOpenFallback={handleOpenFile}
              onSaveMarkdown={handleSaveMarkdown}
              markdownSaveState={markdownSaveState}
              onMarkdownStateChange={setMarkdownEditorState}
            />
          </div>
        </section>

        {detailsOpen ? (
          <DetailsPane
            details={details}
            error={detailsError}
            conflicts={selectedSpaceConflicts}
            conflictError={conflictError}
            selectedConflictId={selectedConflictId}
            conflictDetail={conflictDetail}
            conflictDetailError={conflictDetailError}
            conflictActionError={conflictActionError}
            conflictActionLoading={conflictActionLoading}
            onSelectConflict={setSelectedConflictId}
            onResolveKeepLocal={(conflictId) => void handleResolveConflict("keep-local", conflictId)}
            onResolveKeepRemote={(conflictId) => void handleResolveConflict("keep-remote", conflictId)}
            onDismissConflict={(conflictId) => void handleResolveConflict("dismiss", conflictId)}
          />
        ) : null}
      </main>
    </div>
  );
}
