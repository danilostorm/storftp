import { invoke } from "@tauri-apps/api/core";
import type { AppSettings, CapabilitySet, ConnectionProfile, FileEntry, ProviderRef, TransferJob } from "../types";

export interface SftpHostProbe { host: string; port: number; fingerprint: string }

export const api = {
  homeDirectory: () => invoke<string>("home_directory"),
  list: (provider: ProviderRef, path: string) => invoke<FileEntry[]>("list_entries", { provider, path }),
  capabilities: (provider: ProviderRef) => invoke<CapabilitySet>("provider_capabilities", { provider }),
  connections: () => invoke<ConnectionProfile[]>("list_connections"),
  saveConnection: (profile: ConnectionProfile, secret?: string | null) =>
    invoke<ConnectionProfile>("save_connection", { profile, secret: secret ?? null }),
  deleteConnection: (id: string) => invoke<void>("delete_connection", { id }),
  createDirectory: (provider: ProviderRef, path: string) => invoke<void>("create_directory", { provider, path }),
  deleteEntry: (provider: ProviderRef, path: string, recursive = false) =>
    invoke<void>("delete_entry", { provider, path, recursive }),
  renameEntry: (provider: ProviderRef, from: string, to: string) => invoke<void>("rename_entry", { provider, from, to }),
  enqueueTransfer: (source: ProviderRef, destination: ProviderRef, sourcePath: string, destinationPath: string) =>
    invoke<TransferJob>("enqueue_transfer", { source, destination, sourcePath, destinationPath }),
  transfers: () => invoke<TransferJob[]>("list_transfers"),
  cancelTransfer: (id: string) => invoke<void>("cancel_transfer", { id }),
  retryTransfer: (id: string) => invoke<void>("retry_transfer", { id }),
  pauseTransfer: (id: string) => invoke<void>("pause_transfer", { id }),
  resumeTransfer: (id: string) => invoke<void>("resume_transfer", { id }),
  settings: () => invoke<AppSettings>("get_settings"),
  saveSettings: (settings: AppSettings) => invoke<AppSettings>("save_settings", { settings }),
  probeSftpHost: (id: string) => invoke<SftpHostProbe>("probe_sftp_host", { id }),
  trustSftpHost: (id: string, fingerprint: string) => invoke<ConnectionProfile>("trust_sftp_host", { id, fingerprint }),
  googleOAuth: (connectionName: string, clientId: string, clientSecret: string) =>
    invoke<ConnectionProfile>("google_oauth_login", { connectionName, clientId, clientSecret })
};
