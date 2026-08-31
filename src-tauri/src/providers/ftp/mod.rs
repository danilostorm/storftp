use crate::error::{Result, StorError};
use crate::models::{CapabilitySet, ConnectionProfile, FileEntry, ProviderKind};
use crate::providers::{join_remote_path, StorageProvider};
use std::io::{Read, Write};
use std::time::UNIX_EPOCH;
use suppaftp::list::ListParser;
use suppaftp::native_tls::TlsConnector;
use suppaftp::{FtpError, FtpStream, NativeTlsConnector, NativeTlsFtpStream};

pub struct FtpProvider { profile: ConnectionProfile, secret: Option<String> }
impl FtpProvider { pub fn new(profile: ConnectionProfile, secret: Option<String>) -> Self { Self { profile, secret } } }

enum Client { Plain(FtpStream), Secure(NativeTlsFtpStream) }

impl FtpProvider {
    fn connect(&self) -> Result<Client> {
        let host=self.profile.host.as_deref().ok_or_else(||StorError::Validation("host FTP ausente".into()))?;
        let port=self.profile.port.unwrap_or(match self.profile.kind {ProviderKind::Ftps=>990,_=>21});
        let addr=format!("{host}:{port}");
        let mut client = match self.profile.kind {
            ProviderKind::Ftp => Client::Plain(FtpStream::connect(addr.as_str())?),
            ProviderKind::Ftpes => {
                let raw=NativeTlsFtpStream::connect(addr.as_str())?;
                let tls=NativeTlsConnector::from(TlsConnector::new().map_err(|e|StorError::Ftp(e.to_string()))?);
                Client::Secure(raw.into_secure(tls,host)?)
            }
            ProviderKind::Ftps => {
                let tls=NativeTlsConnector::from(TlsConnector::new().map_err(|e|StorError::Ftp(e.to_string()))?);
                Client::Secure(NativeTlsFtpStream::connect_secure_implicit(addr.as_str(),tls,host)?)
            }
            _ => return Err(StorError::UnsupportedProvider(self.profile.kind.as_str().into())),
        };
        let user=self.profile.username.as_deref().unwrap_or("anonymous"); let password=self.secret.as_deref().unwrap_or("anonymous@storftp.local");
        match &mut client { Client::Plain(c)=>{c.login(user,password)?;},Client::Secure(c)=>{c.login(user,password)?;} }
        Ok(client)
    }

    fn list_lines(client:&mut Client,path:&str)->std::result::Result<Vec<String>,FtpError>{
        let p=if path.is_empty(){None}else{Some(path)};
        match client {Client::Plain(c)=>c.mlsd(p).or_else(|_|c.list(p)),Client::Secure(c)=>c.mlsd(p).or_else(|_|c.list(p))}
    }
    fn size(client:&mut Client,path:&str)->std::result::Result<usize,FtpError>{match client{Client::Plain(c)=>c.size(path),Client::Secure(c)=>c.size(path)}}
    fn mkdir_client(client:&mut Client,path:&str)->std::result::Result<(),FtpError>{match client{Client::Plain(c)=>c.mkdir(path),Client::Secure(c)=>c.mkdir(path)}}
    fn rm_client(client:&mut Client,path:&str)->std::result::Result<(),FtpError>{match client{Client::Plain(c)=>c.rm(path),Client::Secure(c)=>c.rm(path)}}
    fn rmdir_client(client:&mut Client,path:&str)->std::result::Result<(),FtpError>{match client{Client::Plain(c)=>c.rmdir(path),Client::Secure(c)=>c.rmdir(path)}}
    fn rename_client(client:&mut Client,from:&str,to:&str)->std::result::Result<(),FtpError>{match client{Client::Plain(c)=>c.rename(from,to),Client::Secure(c)=>c.rename(from,to)}}
    fn remove_recursive(client:&mut Client,path:&str)->Result<()> {
        if Self::size(client,path).is_ok(){Self::rm_client(client,path)?;return Ok(());}
        let lines=Self::list_lines(client,path)?;
        for line in lines {
            let parsed=ListParser::parse_mlsd(&line).or_else(|_|line.parse::<suppaftp::list::File>());
            if let Ok(file)=parsed { let child=join_remote_path(path,file.name()); if file.is_directory(){Self::remove_recursive(client,&child)?;}else{Self::rm_client(client,&child)?;} }
        }
        Self::rmdir_client(client,path)?;Ok(())
    }
}

impl StorageProvider for FtpProvider {
    fn kind(&self)->ProviderKind{self.profile.kind}
    fn label(&self)->String{self.profile.name.clone()}
    fn capabilities(&self)->CapabilitySet{CapabilitySet{list:true,stat:true,download:true,upload:true,delete:true,mkdir:true,rename:true,move_:true,copy:false,server_side_copy:false,resumable_upload:true,checksum:false,search:true}}
    fn list(&self,path:&str)->Result<Vec<FileEntry>>{
        let mut client=self.connect()?;let lines=Self::list_lines(&mut client,path)?;let mut out=Vec::new();
        for line in lines {
            let parsed=ListParser::parse_mlsd(&line).or_else(|_|line.parse::<suppaftp::list::File>());
            if let Ok(file)=parsed {let modified_at=file.modified().duration_since(UNIX_EPOCH).ok().map(|d|d.as_secs() as i64);out.push(FileEntry{name:file.name().into(),path:join_remote_path(path,file.name()),is_dir:file.is_directory(),size:file.size() as u64,modified_at,mime_type:None,id:None});}
        }
        Ok(out)
    }
    fn stat(&self,path:&str)->Result<FileEntry>{
        let mut client=self.connect()?;
        if let Ok(size)=Self::size(&mut client,path){return Ok(FileEntry{name:path.rsplit('/').next().unwrap_or(path).into(),path:path.into(),is_dir:false,size:size as u64,modified_at:None,mime_type:None,id:None});}
        let parent=path.rsplit_once('/').map(|x|if x.0.is_empty(){"/"}else{x.0}).unwrap_or("/");let name=path.rsplit('/').next().unwrap_or(path);
        self.list(parent)?.into_iter().find(|x|x.name==name).ok_or_else(||StorError::Ftp(format!("caminho não encontrado: {path}")))
    }
    fn download_to(&self,path:&str,writer:&mut dyn Write,offset:u64)->Result<()> {
        let mut client=self.connect()?;
        match &mut client {
            Client::Plain(c)=>{if offset>0{c.resume_transfer(offset as usize)?;}c.retr(path,|stream|std::io::copy(stream,writer).map(|_|()).map_err(FtpError::ConnectionError))?;}
            Client::Secure(c)=>{if offset>0{c.resume_transfer(offset as usize)?;}c.retr(path,|stream|std::io::copy(stream,writer).map(|_|()).map_err(FtpError::ConnectionError))?;}
        } Ok(())
    }
    fn upload_from(&self,path:&str,reader:&mut dyn Read,_size:Option<u64>,offset:u64)->Result<()> {
        let mut client=self.connect()?;
        match &mut client {Client::Plain(c)=>{if offset>0{c.resume_transfer(offset as usize)?;}c.put_file(path,reader)?;},Client::Secure(c)=>{if offset>0{c.resume_transfer(offset as usize)?;}c.put_file(path,reader)?;}} Ok(())
    }
    fn delete(&self,path:&str,recursive:bool)->Result<()> {let mut client=self.connect()?;if Self::size(&mut client,path).is_ok(){Self::rm_client(&mut client,path)?;Ok(())}else if recursive{Self::remove_recursive(&mut client,path)}else{Self::rmdir_client(&mut client,path)?;Ok(())}}
    fn mkdir(&self,path:&str)->Result<()> {let mut c=self.connect()?;Self::mkdir_client(&mut c,path)?;Ok(())}
    fn rename(&self,from:&str,to:&str)->Result<()> {let mut c=self.connect()?;Self::rename_client(&mut c,from,to)?;Ok(())}
}
