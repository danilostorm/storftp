pub mod commands;
pub mod credentials;
pub mod database;
pub mod error;
pub mod imports;
pub mod models;
pub mod providers;
pub mod security;
pub mod sync;
pub mod transfer;

use credentials::{CredentialStore, SystemCredentialStore};
use database::Database;
use providers::ProviderFactory;
use std::sync::Arc;
use tauri::Manager;
use transfer::TransferManager;

pub struct AppState {
    pub db: Arc<Database>,
    pub credentials: Arc<dyn CredentialStore>,
    pub factory: ProviderFactory,
    pub transfers: Arc<TransferManager>,
}

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db = Arc::new(
                Database::open(&data_dir.join("storftp.db"))
                    .map_err(|e| std::io::Error::other(e.to_string()))?,
            );
            let credentials: Arc<dyn CredentialStore> = Arc::new(SystemCredentialStore);
            let factory = ProviderFactory::new(db.clone(), credentials.clone());
            let transfers = TransferManager::new(db.clone(), credentials.clone());
            app.manage(AppState {
                db,
                credentials,
                factory,
                transfers,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::home_directory,
            commands::list_entries,
            commands::provider_capabilities,
            commands::list_connections,
            commands::save_connection,
            commands::delete_connection,
            commands::test_connection,
            commands::import_connections,
            commands::create_directory,
            commands::delete_entry,
            commands::rename_entry,
            commands::search_entries,
            commands::enqueue_transfer,
            commands::list_transfers,
            commands::pause_transfer,
            commands::resume_transfer,
            commands::cancel_transfer,
            commands::retry_transfer,
            commands::get_settings,
            commands::save_settings,
            commands::list_history,
            commands::google_oauth_login,
            commands::probe_sftp_profile,
            commands::probe_sftp_host,
            commands::trust_sftp_host,
            commands::compare_directories,
        ])
        .run(tauri::generate_context!())
        .expect("erro fatal ao iniciar StorFTP");
}
