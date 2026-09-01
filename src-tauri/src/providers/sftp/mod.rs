use crate::error::{Result, StorError};
use crate::models::{CapabilitySet, ConnectionProfile, FileEntry, ProviderKind};
use crate::providers::StorageProvider;
use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine as _};
use sha2::{Digest, Sha256};
use ssh2::{CheckResult, KnownHostFileKind, OpenFlags, OpenType, RenameFlags, Session, Sftp};
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct SftpProvider {
    profile: ConnectionProfile,
    secret: Option<String>,
}

impl SftpProvider {
    pub fn new(profile: ConnectionProfile, secret: Option<String>) -> Self {
        Self { profile, secret }
    }
}

pub fn probe_fingerprint(profile: &ConnectionProfile) -> Result<String> {
    let (_, fingerprint) = handshake(profile, false)?;
    Ok(fingerprint)
}

fn handshake(profile: &ConnectionProfile, verify: bool) -> Result<(Session, String)> {
    let host = profile
        .host
        .as_deref()
        .ok_or_else(|| StorError::Validation("host SFTP ausente".into()))?;
    let port = profile.port.unwrap_or(22);
    let timeout = Duration::from_secs(profile.timeout_seconds.max(5));
    let addr = (host, port)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| StorError::Sftp("host não resolveu para um endereço".into()))?;
    let tcp = TcpStream::connect_timeout(&addr, timeout).map_err(|e| {
        StorError::Sftp(format!("não foi possível conectar a {host}:{port}: {e}"))
    })?;
    tcp.set_read_timeout(Some(timeout))?;
    tcp.set_write_timeout(Some(timeout))?;

    let mut session = Session::new().map_err(|e| StorError::Sftp(e.to_string()))?;
    session.set_tcp_stream(tcp);
    session.handshake()?;
    session.set_keepalive(
        profile.keep_alive_seconds > 0,
        profile.keep_alive_seconds.min(u32::MAX as u64) as u32,
    );

    let (key, _) = session
        .host_key()
        .ok_or_else(|| StorError::Sftp("servidor não apresentou chave de host".into()))?;
    let digest = Sha256::digest(key);
    let fingerprint = format!("SHA256:{}", STANDARD_NO_PAD.encode(digest));
    if verify {
        verify_host_key(&session, profile, host, port, key, &fingerprint)?;
    }
    Ok((session, fingerprint))
}

fn verify_host_key(
    session: &Session,
    profile: &ConnectionProfile,
    host: &str,
    port: u16,
    key: &[u8],
    fingerprint: &str,
) -> Result<()> {
    let mut known_hosts = session.known_hosts()?;
    if let Some(home) = dirs::home_dir() {
        let file = home.join(".ssh").join("known_hosts");
        if file.exists() {
            let _ = known_hosts.read_file(&file, KnownHostFileKind::OpenSSH);
        }
    }

    match known_hosts.check_port(host, port, key) {
        CheckResult::Match => return Ok(()),
        CheckResult::Mismatch => {
            return Err(StorError::Sftp(format!(
                "A chave do servidor mudou. Conexão bloqueada para sua segurança. Host: {host}:{port}; fingerprint atual: {fingerprint}"
            )))
        }
        CheckResult::Failure => {
            return Err(StorError::Sftp(format!(
                "não foi possível validar a chave do host {host}:{port}"
            )))
        }
        CheckResult::NotFound => {}
    }

    if let Some(saved) = profile.extra.get("hostFingerprint").and_then(|v| v.as_str()) {
        if saved == fingerprint {
            return Ok(());
        }
        return Err(StorError::Sftp(format!(
            "Fingerprint diferente do previamente confiado. Esperado: {saved}; recebido: {fingerprint}"
        )));
    }

    Err(StorError::Sftp(format!(
        "Servidor desconhecido. {host}:{port} apresentou {fingerprint}. Confirme o fingerprint antes de confiar neste servidor."
    )))
}

fn auth_method(profile: &ConnectionProfile) -> &str {
    profile
        .extra
        .get("authMethod")
        .and_then(|value| value.as_str())
        .unwrap_or("auto")
}

fn expand_key_path(value: &str) -> PathBuf {
    let trimmed = value.trim();
    if trimmed == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from(trimmed));
    }
    if let Some(rest) = trimmed
        .strip_prefix("~/")
        .or_else(|| trimmed.strip_prefix("~\\"))
    {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(trimmed)
}

fn authenticate(
    session: &Session,
    profile: &ConnectionProfile,
    secret: Option<&str>,
) -> Result<String> {
    let user = profile
        .username
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| StorError::Validation("usuário SFTP ausente".into()))?;
    let method = auth_method(profile);
    let key_value = profile
        .extra
        .get("keyPath")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty());

    match method {
        "password" => {
            let password = secret.ok_or_else(|| {
                StorError::Sftp(
                    "Autenticação por senha selecionada, mas nenhuma senha está salva. Informe a senha e salve a conexão novamente."
                        .into(),
                )
            })?;
            session
                .userauth_password(user, password)
                .map_err(|e| StorError::Sftp(format!("senha rejeitada para {user}: {e}")))?;
            if session.authenticated() {
                return Ok("senha".into());
            }
        }
        "key" => {
            let key_value = key_value.ok_or_else(|| {
                StorError::Sftp(
                    "Autenticação por chave selecionada, mas o caminho da chave privada está vazio."
                        .into(),
                )
            })?;
            let key_path = expand_key_path(key_value);
            if !key_path.is_file() {
                return Err(StorError::Sftp(format!(
                    "Chave SSH privada não encontrada: {}",
                    key_path.display()
                )));
            }
            session
                .userauth_pubkey_file(user, None, &key_path, secret)
                .map_err(|e| {
                    StorError::Sftp(format!(
                        "autenticação por chave falhou usando {}: {e}",
                        key_path.display()
                    ))
                })?;
            if session.authenticated() {
                return Ok("chave privada".into());
            }
        }
        "agent" => {
            session
                .userauth_agent(user)
                .map_err(|e| StorError::Sftp(format!("ssh-agent rejeitou a autenticação: {e}")))?;
            if session.authenticated() {
                return Ok("ssh-agent".into());
            }
        }
        "auto" | "" => {
            let mut failures = Vec::new();

            if let Some(password) = secret.filter(|value| !value.is_empty()) {
                match session.userauth_password(user, password) {
                    Ok(()) if session.authenticated() => return Ok("senha".into()),
                    Ok(()) => failures.push("senha não autenticou".into()),
                    Err(error) => failures.push(format!("senha: {error}")),
                }
            }

            if let Some(key_value) = key_value {
                let key_path = expand_key_path(key_value);
                if key_path.is_file() {
                    match session.userauth_pubkey_file(user, None, &key_path, secret) {
                        Ok(()) if session.authenticated() => return Ok("chave privada".into()),
                        Ok(()) => failures.push("chave privada não autenticou".into()),
                        Err(error) => failures.push(format!(
                            "chave {}: {error}",
                            key_path.display()
                        )),
                    }
                } else {
                    failures.push(format!("chave não encontrada: {}", key_path.display()));
                }
            }

            match session.userauth_agent(user) {
                Ok(()) if session.authenticated() => return Ok("ssh-agent".into()),
                Ok(()) => failures.push("ssh-agent não autenticou".into()),
                Err(error) => failures.push(format!("ssh-agent: {error}")),
            }

            let configured = if secret.is_none() && key_value.is_none() {
                " Nenhuma senha foi encontrada no cofre e nenhum caminho de chave privada foi configurado."
            } else {
                ""
            };
            return Err(StorError::Sftp(format!(
                "Falha de autenticação SFTP para {user}.{configured} Tentativas: {}",
                failures.join("; ")
            )));
        }
        other => {
            return Err(StorError::Validation(format!(
                "método de autenticação SFTP desconhecido: {other}"
            )))
        }
    }

    Err(StorError::Sftp(
        "autenticação rejeitada pelo servidor".into(),
    ))
}

impl SftpProvider {
    fn connect_with_method(&self) -> Result<(Session, Sftp, String)> {
        let (session, _) = handshake(&self.profile, true)?;
        let method = authenticate(&session, &self.profile, self.secret.as_deref())?;
        let sftp = session.sftp()?;
        Ok((session, sftp, method))
    }

    fn connect(&self) -> Result<(Session, Sftp)> {
        let (session, sftp, _) = self.connect_with_method()?;
        Ok((session, sftp))
    }

    pub fn test_connection(&self) -> Result<String> {
        let (_session, _sftp, method) = self.connect_with_method()?;
        Ok(method)
    }

    fn remove_recursive(sftp: &Sftp, path: &Path) -> Result<()> {
        let stat = sftp.stat(path)?;
        if !stat.is_dir() {
            sftp.unlink(path)?;
            return Ok(());
        }
        for (child, child_stat) in sftp.readdir(path)? {
            if child_stat.is_dir() {
                Self::remove_recursive(sftp, &child)?;
            } else {
                sftp.unlink(&child)?;
            }
        }
        sftp.rmdir(path)?;
        Ok(())
    }
}

impl StorageProvider for SftpProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Sftp
    }

    fn label(&self) -> String {
        self.profile.name.clone()
    }

    fn capabilities(&self) -> CapabilitySet {
        CapabilitySet {
            list: true,
            stat: true,
            download: true,
            upload: true,
            delete: true,
            mkdir: true,
            rename: true,
            move_: true,
            copy: false,
            server_side_copy: false,
            resumable_upload: true,
            checksum: false,
            search: true,
        }
    }

    fn list(&self, path: &str) -> Result<Vec<FileEntry>> {
        let (_session, sftp) = self.connect()?;
        let mut out = Vec::new();
        for (entry, stat) in sftp.readdir(Path::new(path))? {
            let name = entry
                .file_name()
                .map(|x| x.to_string_lossy().to_string())
                .unwrap_or_default();
            out.push(FileEntry {
                name,
                path: entry.to_string_lossy().to_string(),
                is_dir: stat.is_dir(),
                size: stat.size.unwrap_or(0),
                modified_at: stat.mtime.map(|v| v as i64),
                mime_type: None,
                id: None,
            });
        }
        Ok(out)
    }

    fn stat(&self, path: &str) -> Result<FileEntry> {
        let (_session, sftp) = self.connect()?;
        let p = PathBuf::from(path);
        let stat = sftp.stat(&p)?;
        Ok(FileEntry {
            name: p
                .file_name()
                .map(|x| x.to_string_lossy().to_string())
                .unwrap_or_else(|| path.into()),
            path: path.into(),
            is_dir: stat.is_dir(),
            size: stat.size.unwrap_or(0),
            modified_at: stat.mtime.map(|v| v as i64),
            mime_type: None,
            id: None,
        })
    }

    fn download_to(&self, path: &str, writer: &mut dyn Write, offset: u64) -> Result<()> {
        let (_session, sftp) = self.connect()?;
        let mut file = sftp.open(Path::new(path))?;
        if offset > 0 {
            file.seek(SeekFrom::Start(offset))?;
        }
        std::io::copy(&mut file, writer)?;
        Ok(())
    }

    fn upload_from(
        &self,
        path: &str,
        reader: &mut dyn Read,
        _size: Option<u64>,
        offset: u64,
    ) -> Result<()> {
        let (_session, sftp) = self.connect()?;
        let flags = if offset > 0 {
            OpenFlags::WRITE | OpenFlags::CREATE
        } else {
            OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::TRUNCATE
        };
        let mut file = sftp.open_mode(Path::new(path), flags, 0o644, OpenType::File)?;
        if offset > 0 {
            file.seek(SeekFrom::Start(offset))?;
        }
        std::io::copy(reader, &mut file)?;
        file.flush()?;
        let _ = file.fsync();
        Ok(())
    }

    fn delete(&self, path: &str, recursive: bool) -> Result<()> {
        let (_session, sftp) = self.connect()?;
        let p = Path::new(path);
        let stat = sftp.stat(p)?;
        if stat.is_dir() {
            if recursive {
                Self::remove_recursive(&sftp, p)
            } else {
                sftp.rmdir(p)?;
                Ok(())
            }
        } else {
            sftp.unlink(p)?;
            Ok(())
        }
    }

    fn mkdir(&self, path: &str) -> Result<()> {
        let (_session, sftp) = self.connect()?;
        sftp.mkdir(Path::new(path), 0o755)?;
        Ok(())
    }

    fn rename(&self, from: &str, to: &str) -> Result<()> {
        let (_session, sftp) = self.connect()?;
        sftp.rename(
            Path::new(from),
            Path::new(to),
            Some(RenameFlags::OVERWRITE),
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::expand_key_path;
    use std::path::PathBuf;

    #[test]
    fn leaves_absolute_key_path_untouched() {
        let input = if cfg!(windows) {
            r"C:\keys\id_ed25519"
        } else {
            "/tmp/id_ed25519"
        };
        assert_eq!(expand_key_path(input), PathBuf::from(input));
    }
}
