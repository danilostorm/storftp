use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Local,
    Sftp,
    Ftp,
    Ftpes,
    Ftps,
    GoogleDrive,
}

impl ProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Sftp => "sftp",
            Self::Ftp => "ftp",
            Self::Ftpes => "ftpes",
            Self::Ftps => "ftps",
            Self::GoogleDrive => "google_drive",
        }
    }
}

impl std::str::FromStr for ProviderKind {
    type Err = String;
    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "local" => Ok(Self::Local), "sftp" => Ok(Self::Sftp), "ftp" => Ok(Self::Ftp),
            "ftpes" => Ok(Self::Ftpes), "ftps" => Ok(Self::Ftps), "google_drive" => Ok(Self::GoogleDrive),
            other => Err(format!("provider desconhecido: {other}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct ProviderRef {
    pub kind: ProviderKind,
    #[serde(default)]
    pub connection_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified_at: Option<i64>,
    pub mime_type: Option<String>,
    pub id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CapabilitySet {
    pub list: bool,
    pub stat: bool,
    pub download: bool,
    pub upload: bool,
    pub delete: bool,
    pub mkdir: bool,
    pub rename: bool,
    #[serde(rename = "move")]
    pub move_: bool,
    pub copy: bool,
    pub server_side_copy: bool,
    pub resumable_upload: bool,
    pub checksum: bool,
    pub search: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionProfile {
    pub id: String,
    pub name: String,
    pub kind: ProviderKind,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub initial_path: Option<String>,
    pub timeout_seconds: u64,
    pub keep_alive_seconds: u64,
    pub max_connections: u32,
    pub group_name: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub extra: Map<String, Value>,
}

impl ConnectionProfile {
    pub fn validate(&self) -> crate::error::Result<()> {
        if self.id.trim().is_empty() || Uuid::parse_str(&self.id).is_err() {
            return Err(crate::error::StorError::Validation("id da conexão inválido".into()));
        }
        if self.name.trim().is_empty() { return Err(crate::error::StorError::Validation("nome da conexão é obrigatório".into())); }
        if self.kind != ProviderKind::GoogleDrive && self.kind != ProviderKind::Local && self.host.as_deref().unwrap_or("").trim().is_empty() {
            return Err(crate::error::StorError::Validation("host é obrigatório".into()));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransferStrategy { ServerSide, DirectStream, LocalRelay }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransferState { Queued, Preparing, Connecting, Transferring, Paused, Retrying, Verifying, Completed, Failed, Cancelled }

impl TransferState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued", Self::Preparing => "preparing", Self::Connecting => "connecting", Self::Transferring => "transferring",
            Self::Paused => "paused", Self::Retrying => "retrying", Self::Verifying => "verifying", Self::Completed => "completed", Self::Failed => "failed", Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferJob {
    pub id: String,
    pub source: ProviderRef,
    pub destination: ProviderRef,
    pub source_path: String,
    pub destination_path: String,
    pub file_name: String,
    pub total_bytes: u64,
    pub transferred_bytes: u64,
    pub state: TransferState,
    pub strategy: TransferStrategy,
    pub speed_bps: f64,
    pub average_speed_bps: f64,
    pub eta_seconds: Option<f64>,
    pub attempts: u32,
    pub max_attempts: u32,
    pub error: Option<String>,
    pub priority: i32,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub theme: String,
    pub concurrent_transfers: usize,
    pub buffer_size_mi_b: usize,
    pub upload_limit_bps: Option<u64>,
    pub download_limit_bps: Option<u64>,
    pub verify_after_transfer: bool,
    pub developer_mode: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self { theme: "dark".into(), concurrent_transfers: 4, buffer_size_mi_b: 4, upload_limit_bps: None, download_limit_bps: None, verify_after_transfer: true, developer_mode: false }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    pub transfer_id: String,
    pub file_name: String,
    pub source_label: String,
    pub destination_label: String,
    pub size: u64,
    pub completed_at: i64,
    pub average_speed_bps: f64,
    pub strategy: TransferStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub text: Option<String>,
    pub extensions: Vec<String>,
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPlan {
    pub source: ProviderRef,
    pub destination: ProviderRef,
    pub source_path: String,
    pub destination_path: String,
    pub bidirectional: bool,
    pub delete_extraneous: bool,
    pub dry_run: bool,
    pub options: BTreeMap<String, Value>,
}
