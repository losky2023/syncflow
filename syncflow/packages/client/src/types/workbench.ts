export interface DeviceInfo {
  device_id: string;
  device_name: string;
  platform: string;
  is_online: boolean;
}

export interface DiscoveredDevice {
  device_id: string;
  device_name: string;
  ip: string;
  platform: string;
}

export interface SyncStatus {
  syncRunning: boolean;
  deviceName: string;
  deviceId: string;
}

export interface SyncedSpace {
  id: string;
  name: string;
  rootPath: string;
  status: string;
  createdAt: string;
  lastScannedAt: string | null;
}

export interface TreeNode {
  name: string;
  relativePath: string;
  nodeType: "directory" | "file";
  hasChildren: boolean;
  extension: string | null;
  size: number | null;
  modifiedAt: string | null;
}

export interface FileDetails {
  name: string;
  nodeType: "directory" | "file";
  extension: string | null;
  size: number;
  modifiedAt: string | null;
  spaceName: string;
  relativePath: string;
}

export interface TextPreviewResult {
  content: string;
  truncated: boolean;
  size: number;
  maxBytes: number;
}

export interface ImagePreviewResult {
  dataUrl: string;
  mimeType: string;
  size: number;
  truncated: false;
}

export type PreviewState =
  | { type: "welcome" }
  | { type: "directory"; node: TreeNode }
  | { type: "loading"; node: TreeNode }
  | { type: "text"; node: TreeNode; result: TextPreviewResult }
  | { type: "image"; node: TreeNode; result: ImagePreviewResult }
  | { type: "fallback"; node: TreeNode; reason?: string }
  | { type: "error"; node: TreeNode; message: string };
