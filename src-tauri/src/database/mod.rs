use crate::error::{Result, StorError};
use crate::models::{AppSettings, ConnectionProfile, HistoryEntry, ProviderKind, ProviderRef, TransferJob, TransferState, TransferStrategy};
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{Map, Value};
use std::path::Path;
use std::str::FromStr;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let database = Self { conn: Mutex::new(conn) };
        database.migrate()?;
        Ok(database)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.lock().execute_batch(r#"
            CREATE TABLE IF NOT EXISTS connections (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                kind TEXT NOT NULL,
                host TEXT,
                port INTEGER,
                username TEXT,
                initial_path TEXT,
                timeout_seconds INTEGER NOT NULL DEFAULT 30,
                keep_alive_seconds INTEGER NOT NULL DEFAULT 30,
                max_connections INTEGER NOT NULL DEFAULT 4,
                group_name TEXT,
                tags_json TEXT NOT NULL DEFAULT '[]',
                favorite INTEGER NOT NULL DEFAULT 0,
                extra_json TEXT NOT NULL DEFAULT '{}',
                created_at INTEGER NOT NULL DEFAULT (unixepoch()),
                updated_at INTEGER NOT NULL DEFAULT (unixepoch())
            );
            CREATE INDEX IF NOT EXISTS idx_connections_kind ON connections(kind);
            CREATE INDEX IF NOT EXISTS idx_connections_group ON connections(group_name);

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value_json TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS favorites (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                provider_json TEXT NOT NULL,
                path TEXT NOT NULL,
                created_at INTEGER NOT NULL DEFAULT (unixepoch())
            );

            CREATE TABLE IF NOT EXISTS transfers (
                id TEXT PRIMARY KEY,
                source_json TEXT NOT NULL,
                destination_json TEXT NOT NULL,
                source_path TEXT NOT NULL,
                destination_path TEXT NOT NULL,
                file_name TEXT NOT NULL,
                total_bytes INTEGER NOT NULL DEFAULT 0,
                transferred_bytes INTEGER NOT NULL DEFAULT 0,
                state TEXT NOT NULL,
                strategy TEXT NOT NULL,
                speed_bps REAL NOT NULL DEFAULT 0,
                average_speed_bps REAL NOT NULL DEFAULT 0,
                eta_seconds REAL,
                attempts INTEGER NOT NULL DEFAULT 0,
                max_attempts INTEGER NOT NULL DEFAULT 5,
                error TEXT,
                priority INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_transfers_state_priority ON transfers(state, priority DESC, created_at ASC);

            CREATE TABLE IF NOT EXISTS history (
                id TEXT PRIMARY KEY,
                transfer_id TEXT NOT NULL,
                file_name TEXT NOT NULL,
                source_label TEXT NOT NULL,
                destination_label TEXT NOT NULL,
                size INTEGER NOT NULL,
                completed_at INTEGER NOT NULL,
                average_speed_bps REAL NOT NULL,
                strategy TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_history_completed ON history(completed_at DESC);
        "#)?;
        Ok(())
    }

    pub fn list_connections(&self) -> Result<Vec<ConnectionProfile>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT id,name,kind,host,port,username,initial_path,timeout_seconds,keep_alive_seconds,max_connections,group_name,tags_json,favorite,extra_json FROM connections ORDER BY favorite DESC, group_name, name COLLATE NOCASE")?;
        let rows = stmt.query_map([], |row| {
            let kind_text: String = row.get(2)?;
            let tags_json: String = row.get(11)?;
            let extra_json: String = row.get(13)?;
            let kind = ProviderKind::from_str(&kind_text).map_err(|e| rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e))))?;
            Ok(ConnectionProfile {
                id: row.get(0)?, name: row.get(1)?, kind, host: row.get(3)?, port: row.get::<_, Option<u16>>(4)?, username: row.get(5)?, initial_path: row.get(6)?,
                timeout_seconds: row.get(7)?, keep_alive_seconds: row.get(8)?, max_connections: row.get(9)?, group_name: row.get(10)?,
                tags: serde_json::from_str(&tags_json).unwrap_or_default(), favorite: row.get::<_, i64>(12)? != 0,
                extra: serde_json::from_str(&extra_json).unwrap_or_else(|_| Map::new()),
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(StorError::from)
    }

    pub fn get_connection(&self, id: &str) -> Result<ConnectionProfile> {
        self.list_connections()?.into_iter().find(|profile| profile.id == id).ok_or_else(|| StorError::ConnectionNotFound(id.to_string()))
    }

    pub fn save_connection(&self, profile: &ConnectionProfile) -> Result<()> {
        profile.validate()?;
        let tags = serde_json::to_string(&profile.tags)?;
        let extra = serde_json::to_string(&profile.extra)?;
        self.conn.lock().execute(r#"
            INSERT INTO connections(id,name,kind,host,port,username,initial_path,timeout_seconds,keep_alive_seconds,max_connections,group_name,tags_json,favorite,extra_json)
            VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)
            ON CONFLICT(id) DO UPDATE SET name=excluded.name,kind=excluded.kind,host=excluded.host,port=excluded.port,username=excluded.username,initial_path=excluded.initial_path,
              timeout_seconds=excluded.timeout_seconds,keep_alive_seconds=excluded.keep_alive_seconds,max_connections=excluded.max_connections,group_name=excluded.group_name,
              tags_json=excluded.tags_json,favorite=excluded.favorite,extra_json=excluded.extra_json,updated_at=unixepoch()
        "#, params![profile.id, profile.name, profile.kind.as_str(), profile.host, profile.port, profile.username, profile.initial_path, profile.timeout_seconds, profile.keep_alive_seconds, profile.max_connections, profile.group_name, tags, profile.favorite as i64, extra])?;
        Ok(())
    }

    pub fn delete_connection(&self, id: &str) -> Result<()> {
        self.conn.lock().execute("DELETE FROM connections WHERE id=?1", [id])?;
        Ok(())
    }

    pub fn get_settings(&self) -> Result<AppSettings> {
        let conn = self.conn.lock();
        let raw: Option<String> = conn.query_row("SELECT value_json FROM settings WHERE key='app'", [], |r| r.get(0)).optional()?;
        match raw { Some(json) => Ok(serde_json::from_str(&json).unwrap_or_default()), None => Ok(AppSettings::default()) }
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<()> {
        if !(1..=32).contains(&settings.concurrent_transfers) { return Err(StorError::Validation("transferências simultâneas deve estar entre 1 e 32".into())); }
        if !(1..=128).contains(&settings.buffer_size_mi_b) { return Err(StorError::Validation("buffer deve estar entre 1 e 128 MiB".into())); }
        let json = serde_json::to_string(settings)?;
        self.conn.lock().execute("INSERT INTO settings(key,value_json) VALUES('app',?1) ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json", [json])?;
        Ok(())
    }

    pub fn save_transfer(&self, job: &TransferJob) -> Result<()> {
        self.conn.lock().execute(r#"
            INSERT INTO transfers(id,source_json,destination_json,source_path,destination_path,file_name,total_bytes,transferred_bytes,state,strategy,speed_bps,average_speed_bps,eta_seconds,attempts,max_attempts,error,priority,created_at)
            VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)
            ON CONFLICT(id) DO UPDATE SET total_bytes=excluded.total_bytes,transferred_bytes=excluded.transferred_bytes,state=excluded.state,strategy=excluded.strategy,speed_bps=excluded.speed_bps,
              average_speed_bps=excluded.average_speed_bps,eta_seconds=excluded.eta_seconds,attempts=excluded.attempts,error=excluded.error,priority=excluded.priority
        "#, params![job.id, serde_json::to_string(&job.source)?, serde_json::to_string(&job.destination)?, job.source_path, job.destination_path, job.file_name,
            job.total_bytes, job.transferred_bytes, job.state.as_str(), strategy_str(job.strategy), job.speed_bps, job.average_speed_bps, job.eta_seconds,
            job.attempts, job.max_attempts, job.error, job.priority, job.created_at])?;
        Ok(())
    }

    pub fn list_transfers(&self) -> Result<Vec<TransferJob>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT id,source_json,destination_json,source_path,destination_path,file_name,total_bytes,transferred_bytes,state,strategy,speed_bps,average_speed_bps,eta_seconds,attempts,max_attempts,error,priority,created_at FROM transfers ORDER BY CASE state WHEN 'transferring' THEN 0 WHEN 'retrying' THEN 1 WHEN 'queued' THEN 2 ELSE 3 END, priority DESC, created_at DESC LIMIT 500")?;
        let rows = stmt.query_map([], |r| {
            let source_json: String = r.get(1)?; let destination_json: String = r.get(2)?;
            let state_text: String = r.get(8)?; let strategy_text: String = r.get(9)?;
            Ok(TransferJob {
                id: r.get(0)?, source: serde_json::from_str::<ProviderRef>(&source_json).map_err(json_sql_error)?, destination: serde_json::from_str::<ProviderRef>(&destination_json).map_err(json_sql_error)?,
                source_path: r.get(3)?, destination_path: r.get(4)?, file_name: r.get(5)?, total_bytes: r.get(6)?, transferred_bytes: r.get(7)?,
                state: parse_state(&state_text), strategy: parse_strategy(&strategy_text), speed_bps: r.get(10)?, average_speed_bps: r.get(11)?, eta_seconds: r.get(12)?,
                attempts: r.get(13)?, max_attempts: r.get(14)?, error: r.get(15)?, priority: r.get(16)?, created_at: r.get(17)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(StorError::from)
    }

    pub fn get_transfer(&self, id: &str) -> Result<TransferJob> {
        self.list_transfers()?.into_iter().find(|job| job.id == id).ok_or_else(|| StorError::Validation(format!("transferência não encontrada: {id}")))
    }

    pub fn recover_incomplete_transfers(&self) -> Result<Vec<TransferJob>> {
        let mut jobs = self.list_transfers()?.into_iter().filter(|j| !matches!(j.state, TransferState::Completed | TransferState::Cancelled)).collect::<Vec<_>>();
        for job in &mut jobs { job.state = TransferState::Queued; job.speed_bps = 0.0; job.eta_seconds = None; job.error = None; self.save_transfer(job)?; }
        Ok(jobs)
    }

    pub fn add_history(&self, entry: &HistoryEntry) -> Result<()> {
        self.conn.lock().execute("INSERT OR REPLACE INTO history(id,transfer_id,file_name,source_label,destination_label,size,completed_at,average_speed_bps,strategy) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![entry.id, entry.transfer_id, entry.file_name, entry.source_label, entry.destination_label, entry.size, entry.completed_at, entry.average_speed_bps, strategy_str(entry.strategy)])?;
        Ok(())
    }

    pub fn list_history(&self, query: Option<&str>) -> Result<Vec<HistoryEntry>> {
        let conn = self.conn.lock();
        let like = format!("%{}%", query.unwrap_or(""));
        let mut stmt = conn.prepare("SELECT id,transfer_id,file_name,source_label,destination_label,size,completed_at,average_speed_bps,strategy FROM history WHERE file_name LIKE ?1 OR source_label LIKE ?1 OR destination_label LIKE ?1 ORDER BY completed_at DESC LIMIT 1000")?;
        let rows = stmt.query_map([like], |r| Ok(HistoryEntry { id:r.get(0)?,transfer_id:r.get(1)?,file_name:r.get(2)?,source_label:r.get(3)?,destination_label:r.get(4)?,size:r.get(5)?,completed_at:r.get(6)?,average_speed_bps:r.get(7)?,strategy:parse_strategy(&r.get::<_,String>(8)?) }))?;
        rows.collect::<std::result::Result<Vec<_>,_>>().map_err(StorError::from)
    }
}

fn json_sql_error(error: serde_json::Error) -> rusqlite::Error { rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error)) }
fn strategy_str(strategy: TransferStrategy) -> &'static str { match strategy { TransferStrategy::ServerSide=>"server_side",TransferStrategy::DirectStream=>"direct_stream",TransferStrategy::LocalRelay=>"local_relay" } }
fn parse_strategy(value:&str)->TransferStrategy { match value { "server_side"=>TransferStrategy::ServerSide,"local_relay"=>TransferStrategy::LocalRelay,_=>TransferStrategy::DirectStream } }
fn parse_state(value:&str)->TransferState { match value { "preparing"=>TransferState::Preparing,"connecting"=>TransferState::Connecting,"transferring"=>TransferState::Transferring,"paused"=>TransferState::Paused,"retrying"=>TransferState::Retrying,"verifying"=>TransferState::Verifying,"completed"=>TransferState::Completed,"failed"=>TransferState::Failed,"cancelled"=>TransferState::Cancelled,_=>TransferState::Queued } }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrips_settings_and_connections() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(&dir.path().join("test.db")).unwrap();
        let settings = AppSettings { concurrent_transfers: 8, ..Default::default() };
        db.save_settings(&settings).unwrap();
        assert_eq!(db.get_settings().unwrap().concurrent_transfers, 8);
    }
}
