use crate::credentials::{google_client_secret_key, google_refresh_key, CredentialStore};
use crate::error::{Result, StorError};
use crate::models::{CapabilitySet, ConnectionProfile, FileEntry, ProviderKind};
use crate::providers::StorageProvider;
use chrono::DateTime;
use reqwest::blocking::{Client, Response};
use reqwest::header::{AUTHORIZATION, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, LOCATION};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::time::Duration;
use url::Url;
use uuid::Uuid;

const DRIVE_API: &str = "https://www.googleapis.com/drive/v3";
const DRIVE_UPLOAD: &str = "https://www.googleapis.com/upload/drive/v3";

pub struct GoogleDriveProvider {
    profile: ConnectionProfile,
    client: Client,
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse { access_token: String, #[serde(default)] refresh_token: Option<String>, #[serde(default)] expires_in: Option<u64>, #[serde(default)] token_type: Option<String> }
#[derive(Debug, Deserialize)]
#[serde(rename_all="camelCase")]
struct DriveFile { id:String, name:String, mime_type:String, #[serde(default)] size:Option<String>, #[serde(default)] modified_time:Option<String>, #[serde(default)] md5_checksum:Option<String>, #[serde(default)] parents:Vec<String> }
#[derive(Debug, Deserialize)]
#[serde(rename_all="camelCase")]
struct DriveList { #[serde(default)] next_page_token:Option<String>, #[serde(default)] files:Vec<DriveFile> }

impl GoogleDriveProvider {
    pub fn new(profile: ConnectionProfile, credentials: Arc<dyn CredentialStore>) -> Result<Self> {
        let client=Client::builder().timeout(Duration::from_secs(profile.timeout_seconds.max(30))).build()?;
        let client_id=profile.extra.get("clientId").and_then(Value::as_str).ok_or_else(||StorError::GoogleDrive("OAuth clientId ausente".into()))?;
        let refresh=credentials.get(&google_refresh_key(&profile.id))?.ok_or_else(||StorError::GoogleDrive("refresh token ausente; reconecte a conta Google".into()))?;
        let client_secret=credentials.get(&google_client_secret_key(&profile.id))?.unwrap_or_default();
        let mut params=vec![("client_id",client_id.to_string()),("refresh_token",refresh),("grant_type","refresh_token".into())];
        if !client_secret.is_empty(){params.push(("client_secret",client_secret));}
        let response=client.post("https://oauth2.googleapis.com/token").form(&params).send()?;
        let token:TokenResponse=parse_google(response,"renovar acesso ao Google Drive")?;
        Ok(Self{profile,client,access_token:token.access_token})
    }

    fn auth(&self, request:reqwest::blocking::RequestBuilder)->reqwest::blocking::RequestBuilder { request.header(AUTHORIZATION,format!("Bearer {}",self.access_token)) }
    fn root_id(&self)->String { self.profile.extra.get("driveId").and_then(Value::as_str).unwrap_or("root").to_string() }

    fn resolve_path(&self,path:&str)->Result<DriveFile>{
        let normalized=path.trim_matches('/');
        if normalized.is_empty(){return Ok(DriveFile{id:self.root_id(),name:"/".into(),mime_type:"application/vnd.google-apps.folder".into(),size:None,modified_time:None,md5_checksum:None,parents:vec![]});}
        let mut parent=self.root_id(); let mut current=None;
        for component in normalized.split('/') {
            let escaped=escape_drive_query(component);
            let q=format!("'{}' in parents and name = '{}' and trashed = false",parent,escaped);
            let mut files=self.query_files(&q,None)?;
            if files.is_empty(){return Err(StorError::GoogleDrive(format!("caminho não encontrado: {path}")));}
            let file=files.remove(0); parent=file.id.clone(); current=Some(file);
        }
        current.ok_or_else(||StorError::GoogleDrive(format!("caminho inválido: {path}")))
    }
    fn resolve_folder_id(&self,path:&str)->Result<String>{let f=self.resolve_path(path)?;if f.mime_type!="application/vnd.google-apps.folder"{return Err(StorError::GoogleDrive(format!("não é uma pasta: {path}")));}Ok(f.id)}
    fn split_destination<'a>(&self,path:&'a str)->(&'a str,&'a str){
        let p=path.trim_end_matches('/'); match p.rsplit_once('/') {Some(("",name))=>("/",name),Some((parent,name))=>(parent,name),None=>("/",p)}
    }
    fn query_files(&self,q:&str,page_token:Option<&str>)->Result<Vec<DriveFile>>{
        let mut req=self.auth(self.client.get(format!("{DRIVE_API}/files"))).query(&[("q",q),("fields","nextPageToken,files(id,name,mimeType,size,modifiedTime,md5Checksum,parents)"),("pageSize","1000"),("supportsAllDrives","true"),("includeItemsFromAllDrives","true")]);
        if let Some(drive_id)=self.profile.extra.get("driveId").and_then(Value::as_str){req=req.query(&[("corpora","drive"),("driveId",drive_id)]);}
        if let Some(token)=page_token{req=req.query(&[("pageToken",token)]);}
        let list:DriveList=parse_google(req.send()?,"listar arquivos")?;Ok(list.files)
    }
    fn list_all(&self,q:&str)->Result<Vec<DriveFile>>{
        let mut token:Option<String>=None;let mut out=Vec::new();
        loop {
            let mut req=self.auth(self.client.get(format!("{DRIVE_API}/files"))).query(&[("q",q),("fields","nextPageToken,files(id,name,mimeType,size,modifiedTime,md5Checksum,parents)"),("pageSize","1000"),("supportsAllDrives","true"),("includeItemsFromAllDrives","true")]);
            if let Some(drive_id)=self.profile.extra.get("driveId").and_then(Value::as_str){req=req.query(&[("corpora","drive"),("driveId",drive_id)]);}
            if let Some(t)=token.as_deref(){req=req.query(&[("pageToken",t)]);}
            let list:DriveList=parse_google(req.send()?,"listar arquivos")?;out.extend(list.files);token=list.next_page_token;if token.is_none(){break;}
        } Ok(out)
    }
    fn entry(&self,file:DriveFile,parent_path:&str)->FileEntry{
        let is_dir=file.mime_type=="application/vnd.google-apps.folder";let size=file.size.as_deref().and_then(|v|v.parse().ok()).unwrap_or(0);
        let modified_at=file.modified_time.as_deref().and_then(|v|DateTime::parse_from_rfc3339(v).ok()).map(|d|d.timestamp());
        let path=if parent_path=="/"{format!("/{}",file.name)}else{format!("{}/{}",parent_path.trim_end_matches('/'),file.name)};
        FileEntry{name:file.name,path,is_dir,size,modified_at,mime_type:Some(file.mime_type),id:Some(file.id)}
    }
    fn existing_child(&self,parent_id:&str,name:&str)->Result<Option<DriveFile>>{
        let q=format!("'{}' in parents and name = '{}' and trashed = false",parent_id,escape_drive_query(name));Ok(self.query_files(&q,None)?.into_iter().next())
    }
    fn start_resumable_session(&self,path:&str,size:u64)->Result<String>{
        let (parent_path,name)=self.split_destination(path); let parent=self.resolve_folder_id(parent_path)?; let existing=self.existing_child(&parent,name)?;
        let (url,method)=if let Some(file)=existing {(format!("{DRIVE_UPLOAD}/files/{}?uploadType=resumable&supportsAllDrives=true",file.id),"PATCH")}else{(format!("{DRIVE_UPLOAD}/files?uploadType=resumable&supportsAllDrives=true"),"POST")};
        let metadata=if method=="POST"{json!({"name":name,"parents":[parent]})}else{json!({"name":name})};
        let request=if method=="POST"{self.client.post(url)}else{self.client.patch(url)};
        let response=self.auth(request).header(CONTENT_TYPE,"application/json; charset=UTF-8").header("X-Upload-Content-Length",size).header("X-Upload-Content-Type","application/octet-stream").json(&metadata).send()?;
        if !response.status().is_success(){return Err(google_http_error(response,"iniciar upload resumível"));}
        response.headers().get(LOCATION).and_then(|v|v.to_str().ok()).map(str::to_string).ok_or_else(||StorError::GoogleDrive("Google não retornou URL de sessão resumível".into()))
    }
}

impl StorageProvider for GoogleDriveProvider {
    fn kind(&self)->ProviderKind{ProviderKind::GoogleDrive}
    fn label(&self)->String{self.profile.name.clone()}
    fn capabilities(&self)->CapabilitySet{CapabilitySet{list:true,stat:true,download:true,upload:true,delete:true,mkdir:true,rename:true,move_:true,copy:true,server_side_copy:true,resumable_upload:true,checksum:true,search:true}}
    fn list(&self,path:&str)->Result<Vec<FileEntry>>{let parent=self.resolve_folder_id(path)?;let q=format!("'{}' in parents and trashed = false",parent);Ok(self.list_all(&q)?.into_iter().map(|f|self.entry(f,path)).collect())}
    fn stat(&self,path:&str)->Result<FileEntry>{let file=self.resolve_path(path)?;let parent=path.trim_end_matches('/').rsplit_once('/').map(|x|if x.0.is_empty(){"/"}else{x.0}).unwrap_or("/");Ok(self.entry(file,parent))}
    fn download_to(&self,path:&str,writer:&mut dyn Write,offset:u64)->Result<()> {
        let file=self.resolve_path(path)?;if file.mime_type.starts_with("application/vnd.google-apps.") {return Err(StorError::GoogleDrive("Arquivos nativos Google Docs/Sheets/Slides precisam de exportação; download binário direto não se aplica".into()));}
        let mut req=self.auth(self.client.get(format!("{DRIVE_API}/files/{}",file.id))).query(&[("alt","media"),("supportsAllDrives","true")]);if offset>0{req=req.header("Range",format!("bytes={offset}-"));}
        let mut response=req.send()?;if !response.status().is_success() && response.status().as_u16()!=206{return Err(google_http_error(response,"baixar arquivo"));}std::io::copy(&mut response,writer)?;Ok(())
    }
    fn upload_from(&self,path:&str,reader:&mut dyn Read,size:Option<u64>,_offset:u64)->Result<()> {
        let total=size.ok_or_else(||StorError::GoogleDrive("tamanho é necessário para upload resumível".into()))?;let session=self.start_resumable_session(path,total)?;
        let chunk_size=8*1024*1024usize;let mut buffer=vec![0u8;chunk_size];let mut sent=0u64;
        while sent<total {let to_read=((total-sent) as usize).min(chunk_size);let mut read=0usize;while read<to_read{let n=reader.read(&mut buffer[read..to_read])?;if n==0{break;}read+=n;}if read==0{break;}let end=sent+read as u64-1;
            let response=self.client.put(&session).header(CONTENT_LENGTH,read).header(CONTENT_RANGE,format!("bytes {sent}-{end}/{total}")).body(buffer[..read].to_vec()).send()?;
            if !(response.status().is_success()||response.status().as_u16()==308){return Err(google_http_error(response,"enviar bloco ao Google Drive"));}sent+=read as u64;
        }
        if sent!=total{return Err(StorError::GoogleDrive(format!("upload terminou antes do esperado: {sent}/{total} bytes")));}Ok(())
    }
    fn delete(&self,path:&str,_recursive:bool)->Result<()> {let f=self.resolve_path(path)?;let response=self.auth(self.client.delete(format!("{DRIVE_API}/files/{}",f.id))).query(&[("supportsAllDrives","true")]).send()?;if !response.status().is_success(){return Err(google_http_error(response,"excluir arquivo"));}Ok(())}
    fn mkdir(&self,path:&str)->Result<()> {let (parent_path,name)=self.split_destination(path);let parent=self.resolve_folder_id(parent_path)?;let response=self.auth(self.client.post(format!("{DRIVE_API}/files"))).query(&[("supportsAllDrives","true")]).json(&json!({"name":name,"mimeType":"application/vnd.google-apps.folder","parents":[parent]})).send()?;let _:DriveFile=parse_google(response,"criar pasta")?;Ok(())}
    fn rename(&self,from:&str,to:&str)->Result<()> {let f=self.resolve_path(from)?;let (old_parent,_)=self.split_destination(from);let (new_parent,new_name)=self.split_destination(to);let old_parent_id=self.resolve_folder_id(old_parent)?;let new_parent_id=self.resolve_folder_id(new_parent)?;let mut req=self.auth(self.client.patch(format!("{DRIVE_API}/files/{}",f.id))).query(&[("supportsAllDrives","true")]).json(&json!({"name":new_name}));if old_parent_id!=new_parent_id{req=req.query(&[("addParents",new_parent_id.as_str()),("removeParents",old_parent_id.as_str())]);}let _:DriveFile=parse_google(req.send()?,"mover/renomear arquivo")?;Ok(())}
    fn copy(&self,from:&str,to:&str)->Result<()> {self.server_side_copy(from,to)}
    fn server_side_copy(&self,from:&str,to:&str)->Result<()> {let source=self.resolve_path(from)?;let (parent_path,name)=self.split_destination(to);let parent=self.resolve_folder_id(parent_path)?;let response=self.auth(self.client.post(format!("{DRIVE_API}/files/{}/copy",source.id))).query(&[("supportsAllDrives","true")]).json(&json!({"name":name,"parents":[parent]})).send()?;let _:DriveFile=parse_google(response,"cópia server-side")?;Ok(())}
    fn checksum(&self,path:&str)->Result<Option<String>>{let f=self.resolve_path(path)?;Ok(f.md5_checksum.map(|v|format!("md5:{v}")))}
    fn search(&self,path:&str,text:&str)->Result<Vec<FileEntry>>{let parent=self.resolve_folder_id(path)?;let q=format!("'{}' in parents and name contains '{}' and trashed = false",parent,escape_drive_query(text));Ok(self.list_all(&q)?.into_iter().map(|f|self.entry(f,path)).collect())}
}

pub fn oauth_login(connection_name:String,client_id:String,client_secret:String,credentials:Arc<dyn CredentialStore>)->Result<ConnectionProfile>{
    if client_id.trim().is_empty(){return Err(StorError::Validation("OAuth Client ID é obrigatório".into()));}
    let listener=TcpListener::bind("127.0.0.1:0")?;let port=listener.local_addr()?.port();let redirect_uri=format!("http://127.0.0.1:{port}/callback");
    use base64::{engine::general_purpose::{URL_SAFE_NO_PAD},Engine as _};use rand::RngCore;use sha2::{Digest,Sha256};
    let mut random=[0u8;32];rand::rng().fill_bytes(&mut random);let verifier=URL_SAFE_NO_PAD.encode(random);let challenge=URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let mut state_bytes=[0u8;24];rand::rng().fill_bytes(&mut state_bytes);let state=URL_SAFE_NO_PAD.encode(state_bytes);
    let mut auth=Url::parse("https://accounts.google.com/o/oauth2/v2/auth").map_err(|e|StorError::GoogleDrive(e.to_string()))?;
    auth.query_pairs_mut().append_pair("client_id",&client_id).append_pair("redirect_uri",&redirect_uri).append_pair("response_type","code").append_pair("scope","https://www.googleapis.com/auth/drive https://www.googleapis.com/auth/userinfo.email").append_pair("access_type","offline").append_pair("prompt","consent").append_pair("code_challenge",&challenge).append_pair("code_challenge_method","S256").append_pair("state",&state);
    open_browser(auth.as_str())?;
    listener.set_nonblocking(false)?;let (mut stream,_)=listener.accept()?;use std::io::{BufRead,BufReader};let mut reader=BufReader::new(stream.try_clone()?);let mut first=String::new();reader.read_line(&mut first)?;let uri=first.split_whitespace().nth(1).ok_or_else(||StorError::GoogleDrive("callback OAuth inválido".into()))?;let callback=Url::parse(&format!("http://127.0.0.1{uri}")).map_err(|e|StorError::GoogleDrive(e.to_string()))?;
    let params=callback.query_pairs().collect::<std::collections::HashMap<_,_>>();if params.get("state").map(|v|v.as_ref())!=Some(state.as_str()){return Err(StorError::GoogleDrive("estado OAuth inválido; autenticação cancelada".into()));}let code=params.get("code").ok_or_else(||StorError::GoogleDrive(params.get("error").map(|v|v.to_string()).unwrap_or_else(||"Google não retornou código OAuth".into())))?.to_string();
    use std::io::Write as _;let body="<html><body style='font-family:system-ui;background:#0e1421;color:#e7edf8;padding:40px'><h2>StorFTP conectado ao Google Drive</h2><p>Você pode fechar esta janela e voltar ao StorFTP.</p></body></html>";write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body)?;
    let client=Client::builder().timeout(Duration::from_secs(60)).build()?;let mut form=vec![("client_id",client_id.clone()),("code",code),("code_verifier",verifier),("redirect_uri",redirect_uri),("grant_type","authorization_code".into())];if !client_secret.is_empty(){form.push(("client_secret",client_secret.clone()));}
    let token:TokenResponse=parse_google(client.post("https://oauth2.googleapis.com/token").form(&form).send()?,"concluir OAuth")?;let refresh=token.refresh_token.ok_or_else(||StorError::GoogleDrive("Google não retornou refresh token; remova a autorização antiga e tente novamente".into()))?;
    let user:Value=parse_google(client.get("https://www.googleapis.com/oauth2/v3/userinfo").header(AUTHORIZATION,format!("Bearer {}",token.access_token)).send()?,"consultar conta Google")?;let email=user.get("email").and_then(Value::as_str).unwrap_or("Google Drive");let id=Uuid::new_v4().to_string();credentials.set(&google_refresh_key(&id),&refresh)?;if !client_secret.is_empty(){credentials.set(&google_client_secret_key(&id),&client_secret)?;}
    let mut extra=Map::new();extra.insert("clientId".into(),Value::String(client_id));extra.insert("email".into(),Value::String(email.into()));
    Ok(ConnectionProfile{id,name:if connection_name.trim().is_empty(){email.into()}else{connection_name},kind:ProviderKind::GoogleDrive,host:None,port:None,username:Some(email.into()),initial_path:Some("/".into()),timeout_seconds:60,keep_alive_seconds:0,max_connections:4,group_name:Some("Google Drive".into()),tags:vec!["cloud".into()],favorite:false,extra})
}

fn parse_google<T:for<'de>Deserialize<'de>>(response:Response,action:&str)->Result<T>{if !response.status().is_success(){return Err(google_http_error(response,action));}response.json::<T>().map_err(StorError::from)}
fn google_http_error(response:Response,action:&str)->StorError{let status=response.status();let retry=response.headers().get("Retry-After").and_then(|v|v.to_str().ok()).map(str::to_string);let text=response.text().unwrap_or_default();StorError::GoogleDrive(format!("não foi possível {action} (HTTP {status}){}: {}",retry.map(|v|format!("; Retry-After {v}s")).unwrap_or_default(),text.chars().take(500).collect::<String>()))}
fn escape_drive_query(value:&str)->String{value.replace('\\',"\\\\").replace('\'',"\\'")}
fn open_browser(url:&str)->Result<()> {let result=if cfg!(target_os="windows"){std::process::Command::new("cmd").args(["/C","start","",url]).spawn()}else if cfg!(target_os="macos"){std::process::Command::new("open").arg(url).spawn()}else{std::process::Command::new("xdg-open").arg(url).spawn()};result.map(|_|()).map_err(|e|StorError::GoogleDrive(format!("não foi possível abrir o navegador: {e}")))}
