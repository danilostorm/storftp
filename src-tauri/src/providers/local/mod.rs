use crate::error::{Result, StorError};
use crate::models::{CapabilitySet, FileEntry, ProviderKind};
use crate::providers::StorageProvider;
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use walkdir::WalkDir;

pub struct LocalProvider;
impl LocalProvider { pub fn new() -> Self { Self } }

fn metadata_to_entry(path: PathBuf, metadata: fs::Metadata) -> FileEntry {
    let name = path.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_else(|| path.to_string_lossy().to_string());
    let modified_at = metadata.modified().ok().and_then(|value| value.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64);
    FileEntry { name, path: path.to_string_lossy().to_string(), is_dir: metadata.is_dir(), size: if metadata.is_file() { metadata.len() } else { 0 }, modified_at, mime_type: None, id: None }
}

impl StorageProvider for LocalProvider {
    fn kind(&self) -> ProviderKind { ProviderKind::Local }
    fn label(&self) -> String { "Este computador".into() }
    fn capabilities(&self) -> CapabilitySet { CapabilitySet { list:true,stat:true,download:true,upload:true,delete:true,mkdir:true,rename:true,move_:true,copy:true,server_side_copy:true,resumable_upload:true,checksum:true,search:true } }
    fn list(&self, path: &str) -> Result<Vec<FileEntry>> {
        let mut items = Vec::new();
        for entry in fs::read_dir(Path::new(path))? {
            let entry = entry?; let metadata = entry.metadata()?;
            items.push(metadata_to_entry(entry.path(), metadata));
        }
        Ok(items)
    }
    fn stat(&self, path: &str) -> Result<FileEntry> { let p=PathBuf::from(path); Ok(metadata_to_entry(p.clone(), fs::metadata(&p)?)) }
    fn download_to(&self, path: &str, writer: &mut dyn Write, offset: u64) -> Result<()> {
        let mut file = File::open(path)?; if offset > 0 { file.seek(SeekFrom::Start(offset))?; }
        std::io::copy(&mut file, writer)?; Ok(())
    }
    fn upload_from(&self, path: &str, reader: &mut dyn Read, _size: Option<u64>, offset: u64) -> Result<()> {
        if let Some(parent)=Path::new(path).parent(){ fs::create_dir_all(parent)?; }
        let mut file = OpenOptions::new().create(true).write(true).truncate(offset==0).open(path)?;
        if offset > 0 { file.seek(SeekFrom::Start(offset))?; }
        std::io::copy(reader, &mut file)?; file.flush()?; Ok(())
    }
    fn delete(&self, path: &str, recursive: bool) -> Result<()> {
        let metadata = fs::metadata(path)?;
        if metadata.is_dir() { if recursive { fs::remove_dir_all(path)?; } else { fs::remove_dir(path)?; } } else { fs::remove_file(path)?; }
        Ok(())
    }
    fn mkdir(&self, path: &str) -> Result<()> { fs::create_dir_all(path)?; Ok(()) }
    fn rename(&self, from: &str, to: &str) -> Result<()> { fs::rename(from,to)?; Ok(()) }
    fn copy(&self, from: &str, to: &str) -> Result<()> {
        let metadata=fs::metadata(from)?;
        if metadata.is_file() { if let Some(parent)=Path::new(to).parent(){fs::create_dir_all(parent)?;} fs::copy(from,to)?; return Ok(()); }
        for entry in WalkDir::new(from) {
            let entry=entry.map_err(|e| StorError::Io(std::io::Error::new(std::io::ErrorKind::Other,e)))?;
            let rel=entry.path().strip_prefix(from).map_err(|e| StorError::Validation(e.to_string()))?;
            let target=Path::new(to).join(rel);
            if entry.file_type().is_dir(){fs::create_dir_all(target)?;}else{if let Some(parent)=target.parent(){fs::create_dir_all(parent)?;}fs::copy(entry.path(),target)?;}
        }
        Ok(())
    }
    fn server_side_copy(&self, from:&str,to:&str)->Result<()> { self.copy(from,to) }
    fn checksum(&self, path:&str)->Result<Option<String>> {
        let mut f=File::open(path)?; let mut hasher=Sha256::new(); let mut buf=[0u8;1024*1024];
        loop { let n=f.read(&mut buf)?; if n==0{break;} hasher.update(&buf[..n]); }
        Ok(Some(format!("sha256:{:x}",hasher.finalize())))
    }
    fn search(&self,path:&str,text:&str)->Result<Vec<FileEntry>> {
        let needle=text.to_lowercase(); let mut out=Vec::new();
        for entry in WalkDir::new(path).max_depth(8).into_iter().filter_map(|e|e.ok()).take(10000) {
            let name=entry.file_name().to_string_lossy().to_string(); if name.to_lowercase().contains(&needle) { if let Ok(meta)=entry.metadata(){out.push(metadata_to_entry(entry.path().to_path_buf(),meta));} }
        }
        Ok(out)
    }
}
