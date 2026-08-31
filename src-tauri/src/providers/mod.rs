pub mod ftp;
pub mod google_drive;
pub mod local;
pub mod sftp;

use crate::credentials::{connection_secret_key, CredentialStore};
use crate::database::Database;
use crate::error::{Result, StorError};
use crate::models::{CapabilitySet, FileEntry, ProviderKind, ProviderRef};
use std::io::{Read, Write};
use std::sync::Arc;

pub trait StorageProvider: Send + Sync {
    fn kind(&self) -> ProviderKind;
    fn label(&self) -> String;
    fn capabilities(&self) -> CapabilitySet;
    fn list(&self, path: &str) -> Result<Vec<FileEntry>>;
    fn stat(&self, path: &str) -> Result<FileEntry>;
    fn download_to(&self, path: &str, writer: &mut dyn Write, offset: u64) -> Result<()>;
    fn upload_from(&self, path: &str, reader: &mut dyn Read, size: Option<u64>, offset: u64) -> Result<()>;
    fn delete(&self, path: &str, recursive: bool) -> Result<()>;
    fn mkdir(&self, path: &str) -> Result<()>;
    fn rename(&self, from: &str, to: &str) -> Result<()>;
    fn copy(&self, _from: &str, _to: &str) -> Result<()> { Err(StorError::UnsupportedOperation("copy".into())) }
    fn server_side_copy(&self, _from: &str, _to: &str) -> Result<()> { Err(StorError::UnsupportedOperation("server-side copy".into())) }
    fn checksum(&self, _path: &str) -> Result<Option<String>> { Ok(None) }
    fn search(&self, path: &str, text: &str) -> Result<Vec<FileEntry>> {
        let lowered = text.to_lowercase();
        Ok(self.list(path)?.into_iter().filter(|entry| entry.name.to_lowercase().contains(&lowered)).collect())
    }
}

#[derive(Clone)]
pub struct ProviderFactory {
    db: Arc<Database>,
    credentials: Arc<dyn CredentialStore>,
}

impl ProviderFactory {
    pub fn new(db: Arc<Database>, credentials: Arc<dyn CredentialStore>) -> Self { Self { db, credentials } }

    pub fn build(&self, provider_ref: &ProviderRef) -> Result<Box<dyn StorageProvider>> {
        if provider_ref.kind == ProviderKind::Local { return Ok(Box::new(local::LocalProvider::new())); }
        let id = provider_ref.connection_id.as_deref().ok_or_else(|| StorError::Validation("connectionId é obrigatório para provider remoto".into()))?;
        let profile = self.db.get_connection(id)?;
        if profile.kind != provider_ref.kind { return Err(StorError::Validation("provider e conexão não correspondem".into())); }
        match profile.kind {
            ProviderKind::Sftp => {
                let secret = self.credentials.get(&connection_secret_key(id))?;
                Ok(Box::new(sftp::SftpProvider::new(profile, secret)))
            }
            ProviderKind::Ftp | ProviderKind::Ftpes | ProviderKind::Ftps => {
                let secret = self.credentials.get(&connection_secret_key(id))?;
                Ok(Box::new(ftp::FtpProvider::new(profile, secret)))
            }
            ProviderKind::GoogleDrive => Ok(Box::new(google_drive::GoogleDriveProvider::new(profile, self.credentials.clone())?)),
            ProviderKind::Local => Ok(Box::new(local::LocalProvider::new())),
        }
    }
}

pub fn join_remote_path(base: &str, name: &str) -> String {
    if base == "/" { format!("/{name}") } else { format!("{}/{}", base.trim_end_matches('/'), name) }
}

#[cfg(test)]
mod tests {
    use super::join_remote_path;
    #[test] fn normalizes_join() { assert_eq!(join_remote_path("/media/", "file.mkv"), "/media/file.mkv"); assert_eq!(join_remote_path("/", "file.mkv"), "/file.mkv"); }
}
