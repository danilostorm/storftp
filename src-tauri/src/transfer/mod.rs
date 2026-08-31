use crate::credentials::CredentialStore;
use crate::database::Database;
use crate::error::{Result, StorError};
use crate::models::{HistoryEntry, ProviderKind, ProviderRef, TransferJob, TransferState, TransferStrategy};
use crate::providers::{ProviderFactory, StorageProvider};
use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use parking_lot::{Condvar, Mutex};
use std::collections::HashMap;
use std::io::{Cursor, Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Weak};
use std::thread;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Default)]
struct TransferControl {
    cancelled: AtomicBool,
    paused: AtomicBool,
}

struct ConcurrencyGate {
    active: Mutex<usize>,
    changed: Condvar,
}

impl ConcurrencyGate {
    fn new() -> Self { Self { active: Mutex::new(0), changed: Condvar::new() } }
    fn enter(&self, limit: usize) -> GateGuard<'_> {
        let mut active = self.active.lock();
        while *active >= limit.max(1) { self.changed.wait(&mut active); }
        *active += 1;
        GateGuard { gate: self }
    }
}

struct GateGuard<'a> { gate: &'a ConcurrencyGate }
impl Drop for GateGuard<'_> {
    fn drop(&mut self) {
        let mut active = self.gate.active.lock();
        *active = active.saturating_sub(1);
        self.gate.changed.notify_all();
    }
}

pub struct TransferManager {
    db: Arc<Database>,
    factory: ProviderFactory,
    sender: Sender<String>,
    controls: Mutex<HashMap<String, Arc<TransferControl>>>,
    gate: ConcurrencyGate,
}

impl TransferManager {
    pub fn new(db: Arc<Database>, credentials: Arc<dyn CredentialStore>) -> Arc<Self> {
        let (sender, receiver) = unbounded::<String>();
        let manager = Arc::new(Self {
            db: db.clone(),
            factory: ProviderFactory::new(db, credentials),
            sender,
            controls: Mutex::new(HashMap::new()),
            gate: ConcurrencyGate::new(),
        });
        Self::spawn_workers(&manager, receiver);
        if let Ok(jobs) = manager.db.recover_incomplete_transfers() {
            for job in jobs { manager.ensure_control(&job.id); let _ = manager.sender.send(job.id); }
        }
        manager
    }

    fn spawn_workers(manager: &Arc<Self>, receiver: Receiver<String>) {
        let weak = Arc::downgrade(manager);
        for index in 0..16 {
            let rx = receiver.clone();
            let manager_weak = weak.clone();
            thread::Builder::new().name(format!("storftp-transfer-{index}")).spawn(move || {
                while let Ok(id) = rx.recv() {
                    let Some(manager) = manager_weak.upgrade() else { break; };
                    let limit = manager.db.get_settings().map(|s| s.concurrent_transfers).unwrap_or(4);
                    let _slot = manager.gate.enter(limit);
                    manager.process(&id);
                }
            }).expect("não foi possível iniciar worker de transferência");
        }
    }

    fn ensure_control(&self, id: &str) -> Arc<TransferControl> {
        self.controls.lock().entry(id.to_string()).or_insert_with(|| Arc::new(TransferControl::default())).clone()
    }

    pub fn enqueue(&self, source: ProviderRef, destination: ProviderRef, source_path: String, destination_path: String) -> Result<TransferJob> {
        let source_provider = self.factory.build(&source)?;
        let stat = source_provider.stat(&source_path)?;
        if stat.is_dir { return Err(StorError::Validation("transferência de diretório deve ser criada por expansão recursiva".into())); }
        let destination_provider = self.factory.build(&destination)?;
        let strategy = resolve_strategy(source_provider.as_ref(), destination_provider.as_ref(), &source, &destination);
        let job = TransferJob {
            id: Uuid::new_v4().to_string(), source, destination, source_path, destination_path,
            file_name: stat.name, total_bytes: stat.size, transferred_bytes: 0,
            state: TransferState::Queued, strategy, speed_bps: 0.0, average_speed_bps: 0.0,
            eta_seconds: None, attempts: 0, max_attempts: 5, error: None, priority: 0,
            created_at: chrono::Utc::now().timestamp(),
        };
        self.db.save_transfer(&job)?;
        self.ensure_control(&job.id);
        self.sender.send(job.id.clone()).map_err(|e| StorError::Internal(e.to_string()))?;
        Ok(job)
    }

    pub fn pause(&self, id: &str) -> Result<()> {
        let control = self.ensure_control(id);
        control.paused.store(true, Ordering::SeqCst);
        let mut job = self.db.get_transfer(id)?;
        if matches!(job.state, TransferState::Transferring | TransferState::Retrying | TransferState::Queued) {
            job.state = TransferState::Paused; job.speed_bps = 0.0; self.db.save_transfer(&job)?;
        }
        Ok(())
    }

    pub fn resume(&self, id: &str) -> Result<()> {
        let control = self.ensure_control(id);
        control.paused.store(false, Ordering::SeqCst);
        let mut job = self.db.get_transfer(id)?;
        if job.state == TransferState::Paused {
            job.state = TransferState::Queued; self.db.save_transfer(&job)?;
            self.sender.send(id.to_string()).map_err(|e| StorError::Internal(e.to_string()))?;
        }
        Ok(())
    }

    pub fn cancel(&self, id: &str) -> Result<()> {
        let control = self.ensure_control(id);
        control.cancelled.store(true, Ordering::SeqCst);
        control.paused.store(false, Ordering::SeqCst);
        let mut job = self.db.get_transfer(id)?;
        job.state = TransferState::Cancelled; job.speed_bps = 0.0; job.error = None;
        self.db.save_transfer(&job)
    }

    pub fn retry(&self, id: &str) -> Result<()> {
        let control = self.ensure_control(id);
        control.cancelled.store(false, Ordering::SeqCst);
        control.paused.store(false, Ordering::SeqCst);
        let mut job = self.db.get_transfer(id)?;
        job.state = TransferState::Queued; job.error = None; job.speed_bps = 0.0; job.attempts = 0;
        self.db.save_transfer(&job)?;
        self.sender.send(id.to_string()).map_err(|e| StorError::Internal(e.to_string()))?;
        Ok(())
    }

    fn process(&self, id: &str) {
        let control = self.ensure_control(id);
        if control.cancelled.load(Ordering::SeqCst) { return; }
        if control.paused.load(Ordering::SeqCst) { return; }
        let Ok(mut job) = self.db.get_transfer(id) else { return; };
        if matches!(job.state, TransferState::Completed | TransferState::Cancelled) { return; }

        loop {
            if control.cancelled.load(Ordering::SeqCst) { let _ = self.mark_cancelled(&mut job); return; }
            job.attempts += 1;
            job.state = TransferState::Preparing;
            job.error = None;
            let _ = self.db.save_transfer(&job);
            match self.execute_once(&mut job, control.clone()) {
                Ok(()) => { let _ = self.mark_completed(&mut job); return; }
                Err(StorError::Cancelled) => { let _ = self.mark_cancelled(&mut job); return; }
                Err(error) if job.attempts < job.max_attempts => {
                    job.state = TransferState::Retrying;
                    job.error = Some(error.user_message());
                    job.speed_bps = 0.0;
                    let _ = self.db.save_transfer(&job);
                    let delay = 2_u64.saturating_pow(job.attempts.min(5));
                    for _ in 0..delay * 10 {
                        if control.cancelled.load(Ordering::SeqCst) { let _ = self.mark_cancelled(&mut job); return; }
                        if control.paused.load(Ordering::SeqCst) { wait_while_paused(&control); }
                        thread::sleep(Duration::from_millis(100));
                    }
                }
                Err(error) => {
                    job.state = TransferState::Failed; job.error = Some(error.user_message()); job.speed_bps = 0.0; job.eta_seconds = None;
                    let _ = self.db.save_transfer(&job); return;
                }
            }
        }
    }

    fn execute_once(&self, job: &mut TransferJob, control: Arc<TransferControl>) -> Result<()> {
        let source = self.factory.build(&job.source)?;
        let destination = self.factory.build(&job.destination)?;
        job.strategy = resolve_strategy(source.as_ref(), destination.as_ref(), &job.source, &job.destination);
        job.state = TransferState::Connecting;
        self.db.save_transfer(job)?;
        if control.cancelled.load(Ordering::SeqCst) { return Err(StorError::Cancelled); }
        wait_while_paused(&control);

        if job.strategy == TransferStrategy::ServerSide {
            job.state = TransferState::Transferring; self.db.save_transfer(job)?;
            let started = Instant::now();
            source.server_side_copy(&job.source_path, &job.destination_path)?;
            job.transferred_bytes = job.total_bytes;
            job.average_speed_bps = if started.elapsed().as_secs_f64() > 0.0 { job.total_bytes as f64 / started.elapsed().as_secs_f64() } else { 0.0 };
            return self.verify_if_enabled(job, source.as_ref(), destination.as_ref());
        }

        job.state = TransferState::Transferring;
        let can_resume = destination.capabilities().resumable_upload && job.destination.kind != ProviderKind::GoogleDrive;
        let offset = if can_resume { job.transferred_bytes.min(job.total_bytes) } else { 0 };
        if offset == 0 { job.transferred_bytes = 0; }
        self.db.save_transfer(job)?;
        self.stream_between(source.as_ref(), destination.as_ref(), job, control, offset)?;
        self.verify_if_enabled(job, source.as_ref(), destination.as_ref())
    }

    fn stream_between(&self, source: &dyn StorageProvider, destination: &dyn StorageProvider, job: &mut TransferJob, control: Arc<TransferControl>, offset: u64) -> Result<()> {
        let settings = self.db.get_settings()?;
        let chunks = ((settings.buffer_size_mi_b.max(1) * 1024 * 1024) / (256 * 1024)).clamp(2, 512);
        let (tx, rx) = bounded::<Vec<u8>>(chunks);
        let progress = Arc::new(Progress::new(self.db.clone(), job.clone(), offset));
        let source_path = job.source_path.clone();
        let destination_path = job.destination_path.clone();
        let remaining = job.total_bytes.saturating_sub(offset);
        let mut src_result: Result<()> = Ok(());
        let mut dst_result: Result<()> = Ok(());
        thread::scope(|scope| {
            let src_control = control.clone();
            let src_progress = progress.clone();
            let src = scope.spawn(|| {
                let mut writer = PipeWriter { tx, control: src_control, progress: src_progress };
                source.download_to(&source_path, &mut writer, offset)
            });
            let dst_control = control.clone();
            let dst = scope.spawn(|| {
                let mut reader = PipeReader::new(rx, dst_control);
                destination.upload_from(&destination_path, &mut reader, Some(remaining), offset)
            });
            src_result = src.join().unwrap_or_else(|_| Err(StorError::Internal("worker de download entrou em panic".into())));
            dst_result = dst.join().unwrap_or_else(|_| Err(StorError::Internal("worker de upload entrou em panic".into())));
        });
        src_result?; dst_result?;
        *job = self.db.get_transfer(&job.id)?;
        Ok(())
    }

    fn verify_if_enabled(&self, job: &mut TransferJob, source: &dyn StorageProvider, destination: &dyn StorageProvider) -> Result<()> {
        if !self.db.get_settings()?.verify_after_transfer { return Ok(()); }
        job.state = TransferState::Verifying; job.speed_bps = 0.0; job.eta_seconds = None; self.db.save_transfer(job)?;
        let source_hash = source.checksum(&job.source_path)?;
        let destination_hash = destination.checksum(&job.destination_path)?;
        if let (Some(a), Some(b)) = (source_hash, destination_hash) {
            let a_kind = a.split_once(':').map(|x| x.0); let b_kind = b.split_once(':').map(|x| x.0);
            if a_kind == b_kind && a != b { return Err(StorError::Validation("checksum de origem e destino não confere".into())); }
        }
        Ok(())
    }

    fn mark_completed(&self, job: &mut TransferJob) -> Result<()> {
        let fresh = self.db.get_transfer(&job.id).unwrap_or_else(|_| job.clone());
        *job = fresh;
        job.state = TransferState::Completed; job.transferred_bytes = job.total_bytes; job.speed_bps = 0.0; job.eta_seconds = None; job.error = None;
        self.db.save_transfer(job)?;
        let source = self.factory.build(&job.source).ok().map(|p| p.label()).unwrap_or_else(|| job.source.kind.as_str().to_string());
        let destination = self.factory.build(&job.destination).ok().map(|p| p.label()).unwrap_or_else(|| job.destination.kind.as_str().to_string());
        self.db.add_history(&HistoryEntry { id: Uuid::new_v4().to_string(), transfer_id: job.id.clone(), file_name: job.file_name.clone(), source_label: source, destination_label: destination, size: job.total_bytes, completed_at: chrono::Utc::now().timestamp(), average_speed_bps: job.average_speed_bps, strategy: job.strategy })
    }

    fn mark_cancelled(&self, job: &mut TransferJob) -> Result<()> {
        job.state = TransferState::Cancelled; job.speed_bps = 0.0; job.eta_seconds = None; self.db.save_transfer(job)
    }
}

fn resolve_strategy(source: &dyn StorageProvider, destination: &dyn StorageProvider, source_ref: &ProviderRef, destination_ref: &ProviderRef) -> TransferStrategy {
    let same_provider = source_ref.kind == destination_ref.kind && source_ref.connection_id == destination_ref.connection_id;
    if same_provider && source.capabilities().server_side_copy && destination.capabilities().server_side_copy {
        TransferStrategy::ServerSide
    } else if source.capabilities().download && destination.capabilities().upload {
        TransferStrategy::DirectStream
    } else {
        TransferStrategy::LocalRelay
    }
}

fn wait_while_paused(control: &TransferControl) {
    while control.paused.load(Ordering::SeqCst) && !control.cancelled.load(Ordering::SeqCst) { thread::sleep(Duration::from_millis(100)); }
}

struct Progress {
    db: Arc<Database>,
    job: Mutex<TransferJob>,
    base: u64,
    bytes: AtomicU64,
    started: Instant,
    last_flush_ms: AtomicU64,
}

impl Progress {
    fn new(db: Arc<Database>, job: TransferJob, base: u64) -> Self { Self { db, job: Mutex::new(job), base, bytes: AtomicU64::new(0), started: Instant::now(), last_flush_ms: AtomicU64::new(0) } }
    fn add(&self, count: u64) {
        let bytes = self.bytes.fetch_add(count, Ordering::Relaxed) + count;
        let elapsed_ms = self.started.elapsed().as_millis() as u64;
        let last = self.last_flush_ms.load(Ordering::Relaxed);
        if elapsed_ms.saturating_sub(last) < 250 && self.base + bytes < self.job.lock().total_bytes { return; }
        self.last_flush_ms.store(elapsed_ms, Ordering::Relaxed);
        let mut job = self.job.lock();
        let elapsed = self.started.elapsed().as_secs_f64().max(0.001);
        job.transferred_bytes = (self.base + bytes).min(job.total_bytes);
        job.speed_bps = bytes as f64 / elapsed;
        job.average_speed_bps = job.speed_bps;
        let remaining = job.total_bytes.saturating_sub(job.transferred_bytes);
        job.eta_seconds = if job.speed_bps > 0.0 { Some(remaining as f64 / job.speed_bps) } else { None };
        let _ = self.db.save_transfer(&job);
    }
}

struct PipeWriter { tx: Sender<Vec<u8>>, control: Arc<TransferControl>, progress: Arc<Progress> }
impl Write for PipeWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.control.cancelled.load(Ordering::SeqCst) { return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "cancelled")); }
        wait_while_paused(&self.control);
        self.tx.send(buf.to_vec()).map_err(|_| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "destination closed"))?;
        self.progress.add(buf.len() as u64);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}

struct PipeReader { rx: Receiver<Vec<u8>>, current: Cursor<Vec<u8>>, control: Arc<TransferControl> }
impl PipeReader { fn new(rx: Receiver<Vec<u8>>, control: Arc<TransferControl>) -> Self { Self { rx, current: Cursor::new(Vec::new()), control } } }
impl Read for PipeReader {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        if self.control.cancelled.load(Ordering::SeqCst) { return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "cancelled")); }
        wait_while_paused(&self.control);
        loop {
            let read = self.current.read(out)?;
            if read > 0 { return Ok(read); }
            match self.rx.recv() {
                Ok(chunk) => self.current = Cursor::new(chunk),
                Err(_) => return Ok(0),
            }
        }
    }
}

trait ErrorUserMessage { fn user_message(&self) -> String; }
impl ErrorUserMessage for StorError {
    fn user_message(&self) -> String {
        match self {
            StorError::Sftp(message) => format!("Falha na conexão SFTP: {message}"),
            StorError::Ftp(message) => format!("Falha na conexão FTP/FTPS: {message}"),
            StorError::GoogleDrive(message) => format!("Google Drive: {message}"),
            StorError::Http(message) => format!("Falha de rede: {message}"),
            StorError::Validation(message) => message.clone(),
            StorError::Cancelled => "Transferência cancelada".into(),
            other => other.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CapabilitySet, FileEntry};
    struct Fake { server: bool }
    impl StorageProvider for Fake {
        fn kind(&self) -> ProviderKind { ProviderKind::Local }
        fn label(&self) -> String { "fake".into() }
        fn capabilities(&self) -> CapabilitySet { CapabilitySet { download: true, upload: true, server_side_copy: self.server, ..Default::default() } }
        fn list(&self,_:&str)->Result<Vec<FileEntry>>{Ok(vec![])} fn stat(&self,_:&str)->Result<FileEntry>{unreachable!()}
        fn download_to(&self,_:&str,_:&mut dyn Write,_:u64)->Result<()>{Ok(())} fn upload_from(&self,_:&str,_:&mut dyn Read,_:Option<u64>,_:u64)->Result<()>{Ok(())}
        fn delete(&self,_:&str,_:bool)->Result<()>{Ok(())} fn mkdir(&self,_:&str)->Result<()>{Ok(())} fn rename(&self,_:&str,_:&str)->Result<()>{Ok(())}
    }
    #[test] fn strategy_prefers_native_copy() {
        let a=Fake{server:true}; let b=Fake{server:true}; let reference=ProviderRef{kind:ProviderKind::Local,connection_id:None};
        assert_eq!(resolve_strategy(&a,&b,&reference,&reference),TransferStrategy::ServerSide);
    }
    #[test] fn strategy_streams_across_providers() {
        let a=Fake{server:false}; let b=Fake{server:false};
        let left=ProviderRef{kind:ProviderKind::Local,connection_id:None}; let right=ProviderRef{kind:ProviderKind::Sftp,connection_id:Some("x".into())};
        assert_eq!(resolve_strategy(&a,&b,&left,&right),TransferStrategy::DirectStream);
    }
}
