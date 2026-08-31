use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorError {
    #[error("Não foi possível acessar o arquivo ou diretório: {0}")]
    Io(#[from] std::io::Error),
    #[error("Erro no banco local: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Falha de serialização: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("A credencial segura não pôde ser acessada: {0}")]
    Credential(String),
    #[error("Conexão não encontrada: {0}")]
    ConnectionNotFound(String),
    #[error("Provider não suportado: {0}")]
    UnsupportedProvider(String),
    #[error("Operação não suportada por este provider: {0}")]
    UnsupportedOperation(String),
    #[error("Falha SFTP/SSH: {0}")]
    Sftp(String),
    #[error("Falha FTP/FTPS: {0}")]
    Ftp(String),
    #[error("Falha no Google Drive: {0}")]
    GoogleDrive(String),
    #[error("Falha HTTP: {0}")]
    Http(String),
    #[error("Transferência cancelada")]
    Cancelled,
    #[error("Entrada inválida: {0}")]
    Validation(String),
    #[error("Erro interno: {0}")]
    Internal(String),
}

impl From<reqwest::Error> for StorError {
    fn from(value: reqwest::Error) -> Self {
        Self::Http(value.to_string())
    }
}

impl From<ssh2::Error> for StorError {
    fn from(value: ssh2::Error) -> Self {
        Self::Sftp(value.to_string())
    }
}

impl From<suppaftp::FtpError> for StorError {
    fn from(value: suppaftp::FtpError) -> Self {
        Self::Ftp(value.to_string())
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiError {
    pub message: String,
    pub technical: String,
}

impl From<StorError> for UiError {
    fn from(value: StorError) -> Self {
        let technical = format!("{value:?}");
        Self { message: value.to_string(), technical }
    }
}

pub type Result<T> = std::result::Result<T, StorError>;
