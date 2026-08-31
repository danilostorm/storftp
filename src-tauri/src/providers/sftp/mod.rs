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

pub struct SftpProvider { profile: ConnectionProfile, secret: Option<String> }
impl SftpProvider { pub fn new(profile: ConnectionProfile, secret: Option<String>) -> Self { Self { profile, secret } } }

pub fn probe_fingerprint(profile: &ConnectionProfile) -> Result<String> {
    let (_, fingerprint) = handshake(profile, false)?;
    Ok(fingerprint)
}

fn handshake(profile: &ConnectionProfile, verify: bool) -> Result<(Session, String)> {
    let host = profile.host.as_deref().ok_or_else(|| StorError::Validation("host SFTP ausente".into()))?;
    let port = profile.port.unwrap_or(22);
    let timeout = Duration::from_secs(profile.timeout_seconds.max(5));
    let addr = (host, port).to_socket_addrs()?.next().ok_or_else(|| StorError::Sftp("host não resolveu para um endereço".into()))?;
    let tcp = TcpStream::connect_timeout(&addr, timeout).map_err(|e| StorError::Sftp(format!("não foi possível conectar a {host}:{port}: {e}")))?;
    tcp.set_read_timeout(Some(timeout))?; tcp.set_write_timeout(Some(timeout))?;
    let mut session = Session::new().map_err(|e| StorError::Sftp(e.to_string()))?;
    session.set_tcp_stream(tcp);
    session.handshake()?;
    session.set_keepalive(profile.keep_alive_seconds > 0, profile.keep_alive_seconds.min(u32::MAX as u64) as u32);
    let (key, _) = session.host_key().ok_or_else(|| StorError::Sftp("servidor não apresentou chave de host".into()))?;
    let digest = Sha256::digest(key);
    let fingerprint = format!("SHA256:{}", STANDARD_NO_PAD.encode(digest));
    if verify { verify_host_key(&session, profile, host, port, key, &fingerprint)?; }
    Ok((session, fingerprint))
}

fn verify_host_key(session: &Session, profile: &ConnectionProfile, host: &str, port: u16, key: &[u8], fingerprint: &str) -> Result<()> {
    let mut known_hosts = session.known_hosts()?;
    if let Some(home) = dirs::home_dir() {
        let file = home.join(".ssh").join("known_hosts");
        if file.exists() { let _ = known_hosts.read_file(&file, KnownHostFileKind::OpenSSH); }
    }
    match known_hosts.check_port(host, port, key) {
        CheckResult::Match => return Ok(()),
        CheckResult::Mismatch => return Err(StorError::Sftp(format!("A chave do servidor mudou. Conexão bloqueada para sua segurança. Host: {host}:{port}; fingerprint atual: {fingerprint}"))),
        CheckResult::Failure => return Err(StorError::Sftp(format!("não foi possível validar a chave do host {host}:{port}"))),
        CheckResult::NotFound => {}
    }
    if let Some(saved) = profile.extra.get("hostFingerprint").and_then(|v| v.as_str()) {
        if saved == fingerprint { return Ok(()); }
        return Err(StorError::Sftp(format!("Fingerprint diferente do previamente confiado. Esperado: {saved}; recebido: {fingerprint}")));
    }
    Err(StorError::Sftp(format!("Servidor desconhecido. {host}:{port} apresentou {fingerprint}. Confirme o fingerprint antes de confiar neste servidor.")))
}

impl SftpProvider {
    fn connect(&self) -> Result<(Session, Sftp)> {
        let (session, _) = handshake(&self.profile, true)?;
        let user = self.profile.username.as_deref().ok_or_else(|| StorError::Validation("usuário SFTP ausente".into()))?;
        if let Some(key_path) = self.profile.extra.get("keyPath").and_then(|v| v.as_str()).filter(|v| !v.is_empty()) {
            session.userauth_pubkey_file(user, None, Path::new(key_path), self.secret.as_deref())?;
        } else if let Some(password) = self.secret.as_deref() {
            session.userauth_password(user, password)?;
        } else {
            session.userauth_agent(user).map_err(|e| StorError::Sftp(format!("autenticação falhou e nenhuma senha/chave foi configurada: {e}")))?;
        }
        if !session.authenticated() { return Err(StorError::Sftp("autenticação rejeitada pelo servidor".into())); }
        let sftp = session.sftp()?;
        Ok((session, sftp))
    }

    fn remove_recursive(sftp: &Sftp, path: &Path) -> Result<()> {
        let stat = sftp.stat(path)?;
        if !stat.is_dir() { sftp.unlink(path)?; return Ok(()); }
        for (child, child_stat) in sftp.readdir(path)? {
            if child_stat.is_dir() { Self::remove_recursive(sftp, &child)?; } else { sftp.unlink(&child)?; }
        }
        sftp.rmdir(path)?; Ok(())
    }
}

impl StorageProvider for SftpProvider {
    fn kind(&self) -> ProviderKind { ProviderKind::Sftp }
    fn label(&self) -> String { self.profile.name.clone() }
    fn capabilities(&self) -> CapabilitySet { CapabilitySet { list:true,stat:true,download:true,upload:true,delete:true,mkdir:true,rename:true,move_:true,copy:false,server_side_copy:false,resumable_upload:true,checksum:false,search:true } }
    fn list(&self, path:&str)->Result<Vec<FileEntry>> {
        let (_session,sftp)=self.connect()?; let mut out=Vec::new();
        for (entry,stat) in sftp.readdir(Path::new(path))? {
            let name=entry.file_name().map(|x|x.to_string_lossy().to_string()).unwrap_or_default();
            out.push(FileEntry{name,path:entry.to_string_lossy().to_string(),is_dir:stat.is_dir(),size:stat.size.unwrap_or(0),modified_at:stat.mtime.map(|v|v as i64),mime_type:None,id:None});
        }
        Ok(out)
    }
    fn stat(&self,path:&str)->Result<FileEntry>{
        let (_session,sftp)=self.connect()?;let p=PathBuf::from(path);let stat=sftp.stat(&p)?;
        Ok(FileEntry{name:p.file_name().map(|x|x.to_string_lossy().to_string()).unwrap_or_else(||path.into()),path:path.into(),is_dir:stat.is_dir(),size:stat.size.unwrap_or(0),modified_at:stat.mtime.map(|v|v as i64),mime_type:None,id:None})
    }
    fn download_to(&self,path:&str,writer:&mut dyn Write,offset:u64)->Result<()> {
        let (_session,sftp)=self.connect()?;let mut file=sftp.open(Path::new(path))?;if offset>0{file.seek(SeekFrom::Start(offset))?;}std::io::copy(&mut file,writer)?;Ok(())
    }
    fn upload_from(&self,path:&str,reader:&mut dyn Read,_size:Option<u64>,offset:u64)->Result<()> {
        let (_session,sftp)=self.connect()?;
        let flags=if offset>0 {OpenFlags::WRITE|OpenFlags::CREATE} else {OpenFlags::WRITE|OpenFlags::CREATE|OpenFlags::TRUNCATE};
        let mut file=sftp.open_mode(Path::new(path),flags,0o644,OpenType::File)?;if offset>0{file.seek(SeekFrom::Start(offset))?;}std::io::copy(reader,&mut file)?;file.flush()?;let _=file.fsync();Ok(())
    }
    fn delete(&self,path:&str,recursive:bool)->Result<()> {
        let (_session,sftp)=self.connect()?;let p=Path::new(path);let stat=sftp.stat(p)?;
        if stat.is_dir(){if recursive{Self::remove_recursive(&sftp,p)}else{sftp.rmdir(p)?;Ok(())}}else{sftp.unlink(p)?;Ok(())}
    }
    fn mkdir(&self,path:&str)->Result<()> { let (_session,sftp)=self.connect()?;sftp.mkdir(Path::new(path),0o755)?;Ok(()) }
    fn rename(&self,from:&str,to:&str)->Result<()> { let (_session,sftp)=self.connect()?;sftp.rename(Path::new(from),Path::new(to),Some(RenameFlags::OVERWRITE))?;Ok(()) }
}
