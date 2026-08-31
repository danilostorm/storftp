export type ProviderKind = "local" | "sftp" | "ftp" | "ftpes" | "ftps" | "google_drive";

export type TransferStrategy = "server_side" | "direct_stream" | "local_relay";
export type TransferState =
  | "queued"
  | "preparing"
  | "connecting"
  | "transferring"
  | "paused"
  | "retrying"
  | "verifying"
  | "completed"
  | "failed"
  | "cancelled";

export interface ProviderRef {
  kind: ProviderKind;
  connectionId?: string | null;
}

export interface FileEntry {
  name: string;
  path: string;
  isDir: boolean;
  size: number;
  modifiedAt?: number | null;
  mimeType?: string | null;
  id?: string | null;
}

export interface CapabilitySet {
  list: boolean;
  stat: boolean;
  download: boolean;
  upload: boolean;
  delete: boolean;
  mkdir: boolean;
  rename: boolean;
  move: boolean;
  copy: boolean;
  serverSideCopy: boolean;
  resumableUpload: boolean;
  checksum: boolean;
  search: boolean;
}

export interface ConnectionProfile {
  id: string;
  name: string;
  kind: ProviderKind;
  host?: string | null;
  port?: number | null;
  username?: string | null;
  initialPath?: string | null;
  timeoutSeconds: number;
  keepAliveSeconds: number;
  maxConnections: number;
  groupName?: string | null;
  tags: string[];
  favorite: boolean;
  extra: Record<string, unknown>;
}

export interface TransferJob {
  id: string;
  source: ProviderRef;
  destination: ProviderRef;
  sourcePath: string;
  destinationPath: string;
  fileName: string;
  totalBytes: number;
  transferredBytes: number;
  state: TransferState;
  strategy: TransferStrategy;
  speedBps: number;
  averageSpeedBps: number;
  etaSeconds?: number | null;
  attempts: number;
  maxAttempts: number;
  error?: string | null;
  priority: number;
  createdAt: number;
}

export interface AppSettings {
  theme: "dark" | "light" | "system";
  concurrentTransfers: number;
  bufferSizeMiB: number;
  uploadLimitBps: number | null;
  downloadLimitBps: number | null;
  verifyAfterTransfer: boolean;
  developerMode: boolean;
}
