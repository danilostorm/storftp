import { invoke } from "@tauri-apps/api/core";
import type { AppSettings, CapabilitySet, ConnectionProfile, FileEntry, ProviderRef, TransferJob } from "../types";

export interface SftpHostProbe {
  host: string;
  port: number;
  fingerprint: string;
}

export interface ConnectionTestResult {
  ok: boolean;
  protocol: string;
  message: string;
  latencyMs: number;
  authMethod?: string | null;
}

export interface ImportReport {
  imported: ConnectionProfile[];
  skipped: number;
  warnings: string[];
  sources: string[];
}

export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error && typeof error === "object") {
    const value = error as { message?: unknown; technical?: unknown };
    if (typeof value.message === "string" && value.message.trim()) {
      return value.message;
    }
    if (typeof value.technical === "string" && value.technical.trim()) {
      return value.technical;
    }
    try {
      return JSON.stringify(error);
    } catch {
      return "Erro desconhecido";
    }
  }
  return String(error ?? "Erro desconhecido");
}

export const api = {
  homeDirectory: () => invoke<string>("home_directory"),
  list: (provider: ProviderRef, path: string) => invoke<FileEntry[]>("list_entries", { provider, path }),
  capabilities: (provider: ProviderRef) => invoke<CapabilitySet>("provider_capabilities", { provider }),
  connections: () => invoke<ConnectionProfile[]>("list_connections"),
  saveConnection: (profile: ConnectionProfile, secret?: string | null) =>
    invoke<ConnectionProfile>("save_connection", { profile, secret: secret ?? null }),
  deleteConnection: (id: string) => invoke<void>("delete_connection", { id }),
  testConnection: (profile: ConnectionProfile, secret?: string | null) =>
    invoke<ConnectionTestResult>("test_connection", { profile, secret: secret ?? null }),
  importConnections: () => invoke<ImportReport>("import_connections"),
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
  probeSftpProfile: (profile: ConnectionProfile) => invoke<SftpHostProbe>("probe_sftp_profile", { profile }),
  probeSftpHost: (id: string) => invoke<SftpHostProbe>("probe_sftp_host", { id }),
  trustSftpHost: (id: string, fingerprint: string) => invoke<ConnectionProfile>("trust_sftp_host", { id, fingerprint }),
  googleOAuth: (connectionName: string, clientId: string, clientSecret: string) =>
    invoke<ConnectionProfile>("google_oauth_login", { connectionName, clientId, clientSecret })
};
