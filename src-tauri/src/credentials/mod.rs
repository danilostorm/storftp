use crate::error::{Result, StorError};

pub trait CredentialStore: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<String>>;
    fn set(&self, key: &str, value: &str) -> Result<()>;
    fn delete(&self, key: &str) -> Result<()>;
}

#[derive(Default)]
pub struct SystemCredentialStore;

impl SystemCredentialStore {
    const SERVICE: &'static str = "StorFTP";

    fn entry(key: &str) -> Result<keyring::Entry> {
        keyring::Entry::new(Self::SERVICE, key).map_err(|e| StorError::Credential(e.to_string()))
    }
}

impl CredentialStore for SystemCredentialStore {
    fn get(&self, key: &str) -> Result<Option<String>> {
        let entry = Self::entry(key)?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(StorError::Credential(error.to_string())),
        }
    }

    fn set(&self, key: &str, value: &str) -> Result<()> {
        Self::entry(key)?.set_password(value).map_err(|e| StorError::Credential(e.to_string()))
    }

    fn delete(&self, key: &str) -> Result<()> {
        let entry = Self::entry(key)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(StorError::Credential(error.to_string())),
        }
    }
}

pub fn connection_secret_key(connection_id: &str) -> String { format!("connection:{connection_id}:secret") }
pub fn google_refresh_key(connection_id: &str) -> String { format!("google:{connection_id}:refresh_token") }
pub fn google_client_secret_key(connection_id: &str) -> String { format!("google:{connection_id}:client_secret") }
