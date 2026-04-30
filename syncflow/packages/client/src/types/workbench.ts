export interface DeviceState {
  deviceId: string;
  deviceName: string;
  platform: string;
  state: "connected" | "discovered" | "offline";
  ip: string | null;
  lastSeenAt: string | null;
}

export interface DiscoveredDevice {
  device_id: string;
  device_name: string;
  ip: string;
  platform: string;
}

export interface SyncStatus {
  syncRunning: boolean;
  accountId: string;
  deviceName: string;
  deviceId: string;
}

export interface BaiduApiConfig {
  configured: boolean;
  provider: string;
  deviceId: string | null;
  clientId: string;
  hasClientSecret: boolean;
  clientSecret: string | null;
  redirectUri: string;
  scopes: string[];
  source: string;
}

export interface SaveBaiduApiConfigRequest {
  deviceId?: string | null;
  clientId: string;
  clientSecret?: string | null;
  redirectUri?: string | null;
  scopes?: string[] | null;
}

export interface BaiduOAuthStartResult {
  authorizationUrl: string;
  state: string;
  redirectUri: string;
  scopes: string[];
}

export interface BaiduAccountStatus {
  connected: boolean;
  provider: string;
  accountId: string | null;
  displayName: string | null;
  expiresAt: string | null;
  scopes: string[];
  reconnectRequired: boolean;
}

export interface BaiduOAuthCompleteResult {
  success: boolean;
  status: BaiduAccountStatus;
}

export interface BaiduImplicitOAuthPayload {
  accessToken: string;
  expiresIn?: number | null;
  scope?: string | null;
  state?: string | null;
}

export interface SyncRuntimeStatus {
  spaceId: string;
  status: "stopped" | "starting" | "indexing" | "watching" | "syncing" | "error";
  fileCount: number;
  pendingCount: number;
  conflictCount: number;
  cloudConflictCount: number;
  connectedPeerCount: number;
  discoveredPeerCount: number;
  cloudProvider: string | null;
  cloudRemotePath: string | null;
  lastCloudScanAt: string | null;
  lastIndexedAt: string | null;
  lastTransportEvent: string | null;
  lastTransportEventAt: string | null;
  lastError: string | null;
}

export interface SyncDiagnostics {
  spaceId: string;
  spaceName: string;
  rootPath: string;
  cloudProvider: string | null;
  cloudRemotePath: string | null;
  summary: SyncSummary;
  manifest: SyncManifestSummary | null;
  queue: CloudSyncTask[];
  conflicts: ConflictInfo[];
  remoteDeletions: RemoteDeletionNotice[];
  safetyNotes: string[];
}

export interface SyncSummary {
  lastSuccessfulSyncAt: string | null;
  lastSyncActivityAt: string | null;
  lastSyncErrorAt: string | null;
  lastSyncError: string | null;
}

export interface SyncManifestSummary {
  path: string;
  version: number;
  manifestId: string | null;
  sequence: number;
  baseRemoteRevision: string | null;
  updatedByDeviceId: string | null;
  updatedAt: string | null;
  entryCount: number;
  fileCount: number;
  directoryCount: number;
}

export interface CloudSyncTask {
  id: number;
  taskKind: string;
  localRelativePath: string;
  remotePath: string;
  attempts: number;
  lastError: string | null;
  createdAt: string;
  updatedAt: string;
  nextAttemptAt: string | null;
}

export interface RemoteDeletionNotice {
  id: number;
  relativePath: string;
  detectedAt: string;
  localVersion: string;
}

export interface ConflictInfo {
  id: number;
  spaceId: string;
  relativePath: string;
  localVersion: string;
  remoteVersion: string;
  remoteDevice: string;
  detectedAt: string;
}

export interface ConflictDetail {
  id: number;
  spaceId: string;
  spaceName: string;
  relativePath: string;
  remoteDevice: string;
  detectedAt: string;
  localVersion: string;
  remoteVersion: string;
  localFileExists: boolean;
  isText: boolean;
  localTextContent: string | null;
  localTextTruncated: boolean | null;
  remoteTextContent: string | null;
  remoteTextTruncated: boolean | null;
  canKeepLocal: boolean;
  canKeepRemote: boolean;
  canCompareText: boolean;
  missingRemoteSnapshotReason: string | null;
}

export interface SyncedSpace {
  id: string;
  syncKey: string;
  name: string;
  rootPath: string;
  status: string;
  createdAt: string;
  lastScannedAt: string | null;
  cloudBinding: CloudSpaceBinding | null;
}

export interface CloudSpaceBinding {
  spaceId: string;
  provider: string;
  remoteRootPath: string;
  remoteRootId: string | null;
  syncMode: string;
  plaintext: boolean;
  createdAt: string;
  updatedAt: string;
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

export interface SaveTextFileResult {
  details: FileDetails;
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
  | { type: "markdown"; node: TreeNode; result: TextPreviewResult }
  | { type: "image"; node: TreeNode; result: ImagePreviewResult }
  | { type: "fallback"; node: TreeNode; reason?: string }
  | { type: "error"; node: TreeNode; message: string };
