import type {
  DeviceInfo,
  DiscoveredDevice,
  FileDetails,
  ImagePreviewResult,
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
    name: raw.name,
    rootPath: raw.rootPath,
    status: raw.status,
    createdAt: raw.createdAt,
    lastScannedAt: raw.lastScannedAt ?? null,
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

export async function login(username: string, password: string) {
  try {
    const invoke = getInvoke();
    return await invoke<{ success: boolean; error?: string; device_id: string; device_name: string }>(
      "login",
      { username, password },
    );
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

export async function removeSyncedSpace(spaceId: string): Promise<boolean> {
  try {
    const invoke = getInvoke();
    return await invoke<boolean>("remove_synced_folder", { space_id: spaceId });
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
      space_id: spaceId,
      parent_relative_path: parentRelativePath,
    });
    return result.map(mapTreeNode);
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
      space_id: spaceId,
      relative_path: relativePath,
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
      space_id: spaceId,
      relative_path: relativePath,
      max_bytes: maxBytes,
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
      space_id: spaceId,
      relative_path: relativePath,
      max_bytes: maxBytes,
    });
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function openFile(spaceId: string, relativePath: string): Promise<boolean> {
  try {
    const invoke = getInvoke();
    return await invoke<boolean>("open_file", {
      space_id: spaceId,
      relative_path: relativePath,
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

export async function getDeviceInfo(): Promise<DeviceInfo[]> {
  try {
    const invoke = getInvoke();
    return await invoke<DeviceInfo[]>("get_device_info");
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
