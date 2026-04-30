import type {
  BaiduAccountStatus,
  BaiduApiConfig,
  BaiduImplicitOAuthPayload,
  SaveBaiduApiConfigRequest,
  BaiduOAuthCompleteResult,
  BaiduOAuthStartResult,
  ConflictDetail,
  ConflictInfo,
  DeviceState,
  DiscoveredDevice,
  FileDetails,
  ImagePreviewResult,
  SaveTextFileResult,
  SyncDiagnostics,
  SyncRuntimeStatus,
  SyncedSpace,
  TextPreviewResult,
  TreeNode,
} from "../types/workbench";

type InvokeFn = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

function getInvoke(): InvokeFn {
  const invoke = (window as any).__TAURI_INTERNALS__?.invoke;
  if (!invoke) {
    throw new Error("请在 Tauri 应用窗口中使用，而不是外部浏览器。");
  }
  return invoke;
}

function normalizeError(error: unknown): Error {
  if (error instanceof Error) return error;
  if (typeof error === "string") return new Error(error);
  return new Error("发生未知错误");
}

function mapSpace(raw: any): SyncedSpace {
  return {
    id: raw.id,
    syncKey: raw.syncKey,
    name: raw.name,
    rootPath: raw.rootPath,
    status: raw.status,
    createdAt: raw.createdAt,
    lastScannedAt: raw.lastScannedAt ?? null,
    cloudBinding: raw.cloudBinding ?? null,
  };
}

function mapTreeNode(raw: any): TreeNode {
  return {
    name: raw.name,
    relativePath: raw.relativePath,
    nodeType: raw.nodeType,
    hasChildren: raw.hasChildren,
    extension: raw.extension ?? null,
    size: raw.size ?? null,
    modifiedAt: raw.modifiedAt ?? null,
  };
}

function mapBaiduApiConfig(raw: any): BaiduApiConfig {
  return {
    configured: Boolean(raw.configured),
    provider: raw.provider,
    deviceId: raw.deviceId ?? null,
    clientId: raw.clientId ?? "",
    hasClientSecret: Boolean(raw.hasClientSecret),
    clientSecret: raw.clientSecret ?? null,
    redirectUri: raw.redirectUri ?? "",
    scopes: raw.scopes ?? [],
    source: raw.source ?? "none",
  };
}

function mapBaiduAccountStatus(raw: any): BaiduAccountStatus {
  return {
    connected: Boolean(raw.connected),
    provider: raw.provider,
    accountId: raw.accountId ?? null,
    displayName: raw.displayName ?? null,
    expiresAt: raw.expiresAt ?? null,
    scopes: raw.scopes ?? [],
    reconnectRequired: Boolean(raw.reconnectRequired),
  };
}

function mapBaiduOAuthStartResult(raw: any): BaiduOAuthStartResult {
  return {
    authorizationUrl: raw.authorizationUrl,
    state: raw.state,
    redirectUri: raw.redirectUri,
    scopes: raw.scopes ?? [],
  };
}

function mapRuntimeStatus(raw: any): SyncRuntimeStatus {
  return {
    spaceId: raw.spaceId,
    status: raw.status,
    fileCount: raw.fileCount,
    pendingCount: raw.pendingCount,
    conflictCount: raw.conflictCount,
    cloudConflictCount: raw.cloudConflictCount ?? 0,
    connectedPeerCount: raw.connectedPeerCount,
    discoveredPeerCount: raw.discoveredPeerCount,
    cloudProvider: raw.cloudProvider ?? null,
    cloudRemotePath: raw.cloudRemotePath ?? null,
    lastCloudScanAt: raw.lastCloudScanAt ?? null,
    lastIndexedAt: raw.lastIndexedAt ?? null,
    lastTransportEvent: raw.lastTransportEvent ?? null,
    lastTransportEventAt: raw.lastTransportEventAt ?? null,
    lastError: raw.lastError ?? null,
  };
}

function mapSyncDiagnostics(raw: any): SyncDiagnostics {
  return {
    spaceId: raw.spaceId,
    spaceName: raw.spaceName,
    rootPath: raw.rootPath,
    cloudProvider: raw.cloudProvider ?? null,
    cloudRemotePath: raw.cloudRemotePath ?? null,
    summary: {
      lastSuccessfulSyncAt: raw.summary?.lastSuccessfulSyncAt ?? null,
      lastSyncActivityAt: raw.summary?.lastSyncActivityAt ?? null,
      lastSyncErrorAt: raw.summary?.lastSyncErrorAt ?? null,
      lastSyncError: raw.summary?.lastSyncError ?? null,
    },
    manifest: raw.manifest
      ? {
          path: raw.manifest.path,
          version: raw.manifest.version,
          manifestId: raw.manifest.manifestId ?? null,
          sequence: raw.manifest.sequence,
          baseRemoteRevision: raw.manifest.baseRemoteRevision ?? null,
          updatedByDeviceId: raw.manifest.updatedByDeviceId ?? null,
          updatedAt: raw.manifest.updatedAt ?? null,
          entryCount: raw.manifest.entryCount,
          fileCount: raw.manifest.fileCount,
          directoryCount: raw.manifest.directoryCount,
        }
      : null,
    queue: (raw.queue ?? []).map((task: any) => ({
      id: task.id,
      taskKind: task.taskKind,
      localRelativePath: task.localRelativePath,
      remotePath: task.remotePath,
      attempts: task.attempts,
      lastError: task.lastError ?? null,
      createdAt: task.createdAt,
      updatedAt: task.updatedAt,
      nextAttemptAt: task.nextAttemptAt ?? null,
    })),
    conflicts: (raw.conflicts ?? []).map(mapConflict),
    remoteDeletions: (raw.remoteDeletions ?? []).map((notice: any) => ({
      id: notice.id,
      relativePath: notice.relativePath,
      detectedAt: notice.detectedAt,
      localVersion: notice.localVersion,
    })),
    safetyNotes: raw.safetyNotes ?? [],
  };
}

function mapDeviceState(raw: any): DeviceState {
  return {
    deviceId: raw.deviceId,
    deviceName: raw.deviceName,
    platform: raw.platform,
    state: raw.state,
    ip: raw.ip ?? null,
    lastSeenAt: raw.lastSeenAt ?? null,
  };
}

function mapConflict(raw: any): ConflictInfo {
  return {
    id: raw.id,
    spaceId: raw.spaceId,
    relativePath: raw.relativePath,
    localVersion: raw.localVersion,
    remoteVersion: raw.remoteVersion,
    remoteDevice: raw.remoteDevice,
    detectedAt: raw.detectedAt,
  };
}

function mapConflictDetail(raw: any): ConflictDetail {
  return {
    id: raw.id,
    spaceId: raw.spaceId,
    spaceName: raw.spaceName,
    relativePath: raw.relativePath,
    remoteDevice: raw.remoteDevice,
    detectedAt: raw.detectedAt,
    localVersion: raw.localVersion,
    remoteVersion: raw.remoteVersion,
    localFileExists: raw.localFileExists,
    isText: raw.isText,
    localTextContent: raw.localTextContent ?? null,
    localTextTruncated: raw.localTextTruncated ?? null,
    remoteTextContent: raw.remoteTextContent ?? null,
    remoteTextTruncated: raw.remoteTextTruncated ?? null,
    canKeepLocal: raw.canKeepLocal,
    canKeepRemote: raw.canKeepRemote,
    canCompareText: raw.canCompareText,
    missingRemoteSnapshotReason: raw.missingRemoteSnapshotReason ?? null,
  };
}

export async function login(username: string, password: string) {
  try {
    const invoke = getInvoke();
    return await invoke<{
      success: boolean;
      error?: string;
      account_id: string;
      device_id: string;
      device_name: string;
    }>("login", { username, password });
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function getBaiduApiConfig(): Promise<BaiduApiConfig> {
  try {
    const invoke = getInvoke();
    const result = await invoke<any>("get_baidu_api_config");
    return mapBaiduApiConfig(result);
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function saveBaiduApiConfig(
  request: SaveBaiduApiConfigRequest,
): Promise<BaiduApiConfig> {
  try {
    const invoke = getInvoke();
    const result = await invoke<any>("save_baidu_api_config", { request });
    return mapBaiduApiConfig(result);
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function clearBaiduApiConfig(): Promise<BaiduApiConfig> {
  try {
    const invoke = getInvoke();
    const result = await invoke<any>("clear_baidu_api_config");
    return mapBaiduApiConfig(result);
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function getBaiduAccountStatus(): Promise<BaiduAccountStatus> {
  try {
    const invoke = getInvoke();
    const result = await invoke<any>("get_baidu_account_status");
    return mapBaiduAccountStatus(result);
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function startBaiduOAuth(): Promise<BaiduOAuthStartResult> {
  try {
    const invoke = getInvoke();
    const result = await invoke<any>("start_baidu_oauth");
    return mapBaiduOAuthStartResult(result);
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function openExternalUrl(url: string): Promise<boolean> {
  try {
    const invoke = getInvoke();
    return await invoke<boolean>("open_url", { url });
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function completeBaiduOAuth(
  code: string,
  state?: string,
): Promise<BaiduOAuthCompleteResult> {
  try {
    const invoke = getInvoke();
    const result = await invoke<any>("complete_baidu_oauth", {
      request: { code, state },
    });
    return {
      success: Boolean(result.success),
      status: mapBaiduAccountStatus(result.status),
    };
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function completeBaiduImplicitOAuth(
  payload: BaiduImplicitOAuthPayload,
): Promise<BaiduOAuthCompleteResult> {
  try {
    const invoke = getInvoke();
    const result = await invoke<any>("complete_baidu_implicit_oauth", {
      request: payload,
    });
    return {
      success: Boolean(result.success),
      status: mapBaiduAccountStatus(result.status),
    };
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function disconnectBaiduAccount(): Promise<boolean> {
  try {
    const invoke = getInvoke();
    return await invoke<boolean>("disconnect_baidu_account");
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function pickFolder(): Promise<string | null> {
  try {
    const invoke = getInvoke();
    const result = await invoke<string | null>("pick_folder");
    return typeof result === "string" ? result : null;
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function listSyncedSpaces(): Promise<SyncedSpace[]> {
  try {
    const invoke = getInvoke();
    const result = await invoke<any[]>("get_synced_folders");
    return result.map(mapSpace);
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function addSyncedSpace(path: string): Promise<SyncedSpace> {
  try {
    const invoke = getInvoke();
    const result = await invoke<any>("add_synced_folder", { path });
    return mapSpace(result);
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function bindBaiduSpace(
  spaceId: string,
  remoteRootPath?: string,
): Promise<SyncedSpace> {
  try {
    const invoke = getInvoke();
    const result = await invoke<any>("bind_baidu_space", {
      request: { spaceId, remoteRootPath },
    });
    return mapSpace(result);
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function createBaiduSyncedSpace(
  path: string,
  remoteRootPath?: string,
): Promise<SyncedSpace> {
  try {
    const invoke = getInvoke();
    const result = await invoke<any>("create_baidu_synced_space", {
      path,
      remoteRootPath,
    });
    return mapSpace(result);
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function exportSpaceInvite(spaceId: string): Promise<string> {
  try {
    const invoke = getInvoke();
    return await invoke<string>("export_space_invite", { spaceId });
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function joinSpaceFromInvite(inviteCode: string, password: string): Promise<SyncedSpace> {
  try {
    const invoke = getInvoke();
    const result = await invoke<any>("join_space_from_invite", { inviteCode, password });
    return mapSpace(result);
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function removeSyncedSpace(spaceId: string): Promise<boolean> {
  try {
    const invoke = getInvoke();
    return await invoke<boolean>("remove_synced_folder", { spaceId });
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function getTreeChildren(
  spaceId: string,
  parentRelativePath?: string,
): Promise<TreeNode[]> {
  try {
    const invoke = getInvoke();
    const result = await invoke<any[]>("get_tree_children", {
      spaceId,
      parentRelativePath,
    });
    return result.map(mapTreeNode);
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function createTreeFile(
  spaceId: string,
  parentRelativePath: string | null,
  name: string,
): Promise<TreeNode> {
  try {
    const invoke = getInvoke();
    const result = await invoke<any>("create_tree_file", {
      request: { spaceId, parentRelativePath, name },
    });
    return mapTreeNode(result);
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function createTreeFolder(
  spaceId: string,
  parentRelativePath: string | null,
  name: string,
): Promise<TreeNode> {
  try {
    const invoke = getInvoke();
    const result = await invoke<any>("create_tree_folder", {
      request: { spaceId, parentRelativePath, name },
    });
    return mapTreeNode(result);
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function getFileDetails(
  spaceId: string,
  relativePath: string,
): Promise<FileDetails> {
  try {
    const invoke = getInvoke();
    return await invoke<FileDetails>("get_file_details", {
      spaceId,
      relativePath,
    });
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function previewTextFile(
  spaceId: string,
  relativePath: string,
  maxBytes?: number,
): Promise<TextPreviewResult> {
  try {
    const invoke = getInvoke();
    return await invoke<TextPreviewResult>("preview_file_text", {
      spaceId,
      relativePath,
      maxBytes,
    });
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function saveTextFile(
  spaceId: string,
  relativePath: string,
  content: string,
): Promise<SaveTextFileResult> {
  try {
    const invoke = getInvoke();
    return await invoke<SaveTextFileResult>("save_text_file", {
      request: { spaceId, relativePath, content },
    });
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function previewImageFile(
  spaceId: string,
  relativePath: string,
  maxBytes?: number,
): Promise<ImagePreviewResult> {
  try {
    const invoke = getInvoke();
    return await invoke<ImagePreviewResult>("preview_file_image", {
      spaceId,
      relativePath,
      maxBytes,
    });
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function openFile(spaceId: string, relativePath: string): Promise<boolean> {
  try {
    const invoke = getInvoke();
    return await invoke<boolean>("open_file", {
      spaceId,
      relativePath,
    });
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function startSync(password: string, deviceName: string): Promise<boolean> {
  try {
    const invoke = getInvoke();
    return await invoke<boolean>("start_sync", { password, device_name: deviceName });
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function stopSync(): Promise<boolean> {
  try {
    const invoke = getInvoke();
    return await invoke<boolean>("stop_sync");
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function startSpaceSync(spaceId: string): Promise<SyncRuntimeStatus> {
  try {
    const invoke = getInvoke();
    const result = await invoke<any>("start_space_sync", { spaceId });
    return mapRuntimeStatus(result);
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function stopSpaceSync(spaceId: string): Promise<SyncRuntimeStatus> {
  try {
    const invoke = getInvoke();
    const result = await invoke<any>("stop_space_sync", { spaceId });
    return mapRuntimeStatus(result);
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function getSyncStatus(spaceId: string): Promise<SyncRuntimeStatus> {
  try {
    const invoke = getInvoke();
    const result = await invoke<any>("get_sync_status", { spaceId });
    return mapRuntimeStatus(result);
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function getAllSyncStatuses(): Promise<SyncRuntimeStatus[]> {
  try {
    const invoke = getInvoke();
    const result = await invoke<any[]>("get_all_sync_statuses");
    return result.map(mapRuntimeStatus);
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function getSyncDiagnostics(spaceId: string): Promise<SyncDiagnostics> {
  try {
    const invoke = getInvoke();
    const result = await invoke<any>("get_sync_diagnostics", { spaceId });
    return mapSyncDiagnostics(result);
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function retryCloudSyncTask(spaceId: string, taskId: number): Promise<boolean> {
  try {
    const invoke = getInvoke();
    return await invoke<boolean>("retry_cloud_sync_task", { request: { spaceId, taskId } });
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function ignoreCloudSyncTask(spaceId: string, taskId: number): Promise<boolean> {
  try {
    const invoke = getInvoke();
    return await invoke<boolean>("ignore_cloud_sync_task", { request: { spaceId, taskId } });
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function restoreRemoteDeletedFile(spaceId: string, noticeId: number): Promise<boolean> {
  try {
    const invoke = getInvoke();
    return await invoke<boolean>("restore_remote_deleted_file", { request: { spaceId, noticeId } });
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function dismissRemoteDeletedNotice(spaceId: string, noticeId: number): Promise<boolean> {
  try {
    const invoke = getInvoke();
    return await invoke<boolean>("dismiss_remote_deleted_notice", { request: { spaceId, noticeId } });
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function getDeviceInfo(): Promise<DeviceState[]> {
  try {
    const invoke = getInvoke();
    const result = await invoke<any[]>("get_device_info");
    return result.map(mapDeviceState);
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function getDiscoveredDevices(): Promise<DiscoveredDevice[]> {
  try {
    const invoke = getInvoke();
    return await invoke<DiscoveredDevice[]>("get_discovered_devices");
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function getConflicts(spaceId?: string): Promise<ConflictInfo[]> {
  try {
    const invoke = getInvoke();
    const result = await invoke<any[]>("get_conflicts", { spaceId });
    return result.map(mapConflict);
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function getConflictDetail(conflictId: number): Promise<ConflictDetail> {
  try {
    const invoke = getInvoke();
    const result = await invoke<any>("get_conflict_detail", { conflictId });
    return mapConflictDetail(result);
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function resolveConflictKeepLocal(conflictId: number): Promise<boolean> {
  try {
    const invoke = getInvoke();
    return await invoke<boolean>("resolve_conflict_keep_local", { conflictId });
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function resolveConflictKeepRemote(conflictId: number): Promise<boolean> {
  try {
    const invoke = getInvoke();
    return await invoke<boolean>("resolve_conflict_keep_remote", { conflictId });
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function dismissConflict(conflictId: number): Promise<boolean> {
  try {
    const invoke = getInvoke();
    return await invoke<boolean>("dismiss_conflict", { conflictId });
  } catch (error) {
    throw normalizeError(error);
  }
}
