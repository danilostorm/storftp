use crate::credentials::{connection_secret_key, CredentialStore};
use crate::error::{Result as StorResult, StorError, UiError};
use crate::models::{AppSettings, CapabilitySet, ConnectionProfile, FileEntry, HistoryEntry, ProviderRef, TransferJob};
use crate::providers::sftp::probe_fingerprint;
use crate::sync::CompareEntry;
use crate::AppState;
use serde::Serialize;
use serde_json::Value;
use tauri::State;

pub type CommandResult<T> = std::result::Result<T, UiError>;

async fn blocking<T, F>(task: F) -> CommandResult<T>
where T: Send + 'static, F: FnOnce() -> StorResult<T> + Send + 'static {
    tauri::async_runtime::spawn_blocking(task).await
        .map_err(|error| UiError { message: "A operação interna foi interrompida.".into(), technical: error.to_string() })?
        .map_err(UiError::from)
}

#[tauri::command]
pub fn home_directory() -> String { dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/")).to_string_lossy().to_string() }

#[tauri::command]
pub async fn list_entries(state: State<'_, AppState>, provider: ProviderRef, path: String) -> CommandResult<Vec<FileEntry>> {
    let factory = state.factory.clone(); blocking(move || factory.build(&provider)?.list(&path)).await
}

#[tauri::command]
pub async fn provider_capabilities(state: State<'_, AppState>, provider: ProviderRef) -> CommandResult<CapabilitySet> {
    let factory = state.factory.clone(); blocking(move || Ok(factory.build(&provider)?.capabilities())).await
}

#[tauri::command]
pub fn list_connections(state: State<'_, AppState>) -> CommandResult<Vec<ConnectionProfile>> { state.db.list_connections().map_err(UiError::from) }

#[tauri::command]
pub fn save_connection(state: State<'_, AppState>, profile: ConnectionProfile, secret: Option<String>) -> CommandResult<ConnectionProfile> {
    profile.validate().map_err(UiError::from)?;
    state.db.save_connection(&profile).map_err(UiError::from)?;
    if let Some(value) = secret.filter(|v| !v.is_empty()) { state.credentials.set(&connection_secret_key(&profile.id), &value).map_err(UiError::from)?; }
    Ok(profile)
}

#[tauri::command]
pub fn delete_connection(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    let _ = state.credentials.delete(&connection_secret_key(&id));
    state.db.delete_connection(&id).map_err(UiError::from)
}

#[tauri::command]
pub async fn create_directory(state: State<'_, AppState>, provider: ProviderRef, path: String) -> CommandResult<()> {
    let factory=state.factory.clone(); blocking(move || factory.build(&provider)?.mkdir(&path)).await
}
#[tauri::command]
pub async fn delete_entry(state: State<'_, AppState>, provider: ProviderRef, path: String, recursive: bool) -> CommandResult<()> {
    let factory=state.factory.clone(); blocking(move || factory.build(&provider)?.delete(&path, recursive)).await
}
#[tauri::command]
pub async fn rename_entry(state: State<'_, AppState>, provider: ProviderRef, from: String, to: String) -> CommandResult<()> {
    let factory=state.factory.clone(); blocking(move || factory.build(&provider)?.rename(&from,&to)).await
}
#[tauri::command]
pub async fn search_entries(state: State<'_, AppState>, provider: ProviderRef, path: String, text: String) -> CommandResult<Vec<FileEntry>> {
    let factory=state.factory.clone(); blocking(move || factory.build(&provider)?.search(&path,&text)).await
}

#[tauri::command]
pub fn enqueue_transfer(state: State<'_, AppState>, source: ProviderRef, destination: ProviderRef, source_path: String, destination_path: String) -> CommandResult<TransferJob> {
    state.transfers.enqueue(source,destination,source_path,destination_path).map_err(UiError::from)
}
#[tauri::command]
pub fn list_transfers(state: State<'_, AppState>) -> CommandResult<Vec<TransferJob>> { state.db.list_transfers().map_err(UiError::from) }
#[tauri::command]
pub fn pause_transfer(state: State<'_, AppState>, id:String)->CommandResult<()> { state.transfers.pause(&id).map_err(UiError::from) }
#[tauri::command]
pub fn resume_transfer(state: State<'_, AppState>, id:String)->CommandResult<()> { state.transfers.resume(&id).map_err(UiError::from) }
#[tauri::command]
pub fn cancel_transfer(state: State<'_, AppState>, id:String)->CommandResult<()> { state.transfers.cancel(&id).map_err(UiError::from) }
#[tauri::command]
pub fn retry_transfer(state: State<'_, AppState>, id:String)->CommandResult<()> { state.transfers.retry(&id).map_err(UiError::from) }

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>)->CommandResult<AppSettings> { state.db.get_settings().map_err(UiError::from) }
#[tauri::command]
pub fn save_settings(state: State<'_, AppState>,settings:AppSettings)->CommandResult<AppSettings> { state.db.save_settings(&settings).map_err(UiError::from)?; Ok(settings) }
#[tauri::command]
pub fn list_history(state: State<'_, AppState>, query:Option<String>)->CommandResult<Vec<HistoryEntry>> { state.db.list_history(query.as_deref()).map_err(UiError::from) }

#[tauri::command]
pub async fn google_oauth_login(state: State<'_, AppState>, connection_name:String,client_id:String,client_secret:String)->CommandResult<ConnectionProfile> {
    let credentials=state.credentials.clone(); let db=state.db.clone();
    let profile=blocking(move || crate::providers::google_drive::oauth_login(connection_name,client_id,client_secret,credentials)).await?;
    db.save_connection(&profile).map_err(UiError::from)?; Ok(profile)
}

#[derive(Debug, Serialize)]
#[serde(rename_all="camelCase")]
pub struct SftpHostProbe { pub host:String,pub port:u16,pub fingerprint:String }

#[tauri::command]
pub async fn probe_sftp_host(state:State<'_,AppState>, id:String)->CommandResult<SftpHostProbe> {
    let db=state.db.clone(); blocking(move || { let profile=db.get_connection(&id)?; if profile.kind!=crate::models::ProviderKind::Sftp{return Err(StorError::Validation("a conexão não é SFTP".into()));} let fingerprint=probe_fingerprint(&profile)?; Ok(SftpHostProbe{host:profile.host.unwrap_or_default(),port:profile.port.unwrap_or(22),fingerprint}) }).await
}

#[tauri::command]
pub async fn trust_sftp_host(state:State<'_,AppState>,id:String,fingerprint:String)->CommandResult<ConnectionProfile> {
    let db=state.db.clone(); blocking(move || { let mut profile=db.get_connection(&id)?; if profile.kind!=crate::models::ProviderKind::Sftp{return Err(StorError::Validation("a conexão não é SFTP".into()));} let current=probe_fingerprint(&profile)?; if current!=fingerprint{return Err(StorError::Sftp("o fingerprint mudou antes da confirmação; operação cancelada".into()));} profile.extra.insert("hostFingerprint".into(),Value::String(current)); db.save_connection(&profile)?; Ok(profile) }).await
}

#[tauri::command]
pub async fn compare_directories(state:State<'_,AppState>,left:ProviderRef,left_path:String,right:ProviderRef,right_path:String)->CommandResult<Vec<CompareEntry>> {
    let factory=state.factory.clone(); blocking(move || crate::sync::compare(&factory,&left,&left_path,&right,&right_path)).await
}
