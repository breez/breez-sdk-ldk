use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use bitcoin::bip32::{ChildNumber, Xpriv};
use bitcoin::secp256k1::{PublicKey, Secp256k1};
use hex::ToHex;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rand::distributions::Alphanumeric;
use rand::Rng;
use sdk_common::prelude::Network;
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use vss_client_ng::client::VssClient;
use vss_client_ng::error::VssError;
use vss_client_ng::headers::sigs_auth::SigsAuthProvider;
use vss_client_ng::util::retry::{
    ExponentialBackoffRetryPolicy, FilteredRetryPolicy, JitteredRetryPolicy,
    MaxAttemptsRetryPolicy, MaxTotalDelayRetryPolicy, RetryPolicy,
};

use crate::ldk::store::{PreviousHolder, VssStore};
use crate::node_api::NodeResult;
use crate::persist::error::PersistError;
use crate::Config;

pub(crate) type CustomRetryPolicy = FilteredRetryPolicy<
    JitteredRetryPolicy<
        MaxTotalDelayRetryPolicy<MaxAttemptsRetryPolicy<ExponentialBackoffRetryPolicy<VssError>>>,
    >,
    Box<dyn Fn(&VssError) -> bool + 'static + Send + Sync>,
>;

pub(crate) type LockingStore = crate::ldk::store::LockingStore<VssStore<CustomRetryPolicy>>;
pub(crate) type MirroringStore = crate::ldk::store::MirroringStore<Arc<LockingStore>, LockingStore>;

const VSS_HARDENED_CHILD_INDEX: u32 = 877;
const API_KEY_HEADER: &str = "X-Api-Key";
const USER_PUBKEY_HEADER: &str = "X-Pubkey";

pub(crate) fn build_vss_store(
    config: &Config,
    seed: &[u8],
    store_id: &str,
) -> NodeResult<VssStore<CustomRetryPolicy>> {
    let secp = Secp256k1::new();
    let bitcoin_network: bitcoin::Network = config.network.into();
    let xprv = Xpriv::new_master(bitcoin_network, seed)?.derive_priv(
        &secp,
        &[ChildNumber::Hardened {
            index: VSS_HARDENED_CHILD_INDEX,
        }],
    )?;
    let private_key = xprv.private_key;
    let pubkey = PublicKey::from_secret_key(&secp, &private_key);
    let pubkey_hex = pubkey.serialize().encode_hex::<String>();

    let vss_seed = xprv.private_key.secret_bytes();
    let store_id = match config.network {
        Network::Regtest => {
            // Regtest instance of VSS does not implement authentication,
            // that is why the pubkey is used to avoid collisions.
            format!("{pubkey_hex}/{store_id}")
        }
        _ => store_id.to_string(),
    };

    let retry_policy = ExponentialBackoffRetryPolicy::new(Duration::from_secs(1))
        .with_max_attempts(10)
        .with_max_total_delay(Duration::from_secs(40))
        .with_max_jitter(Duration::from_millis(10))
        .skip_retry_on_error(Box::new(|e: &VssError| {
            matches!(
                e,
                VssError::NoSuchKeyError(..)
                    | VssError::InvalidRequestError(..)
                    | VssError::ConflictError(..)
            )
        }) as _);

    let api_key = config.api_key.clone().unwrap_or_default();
    let headers = HashMap::from([
        (API_KEY_HEADER.to_string(), api_key),
        (USER_PUBKEY_HEADER.to_string(), pubkey_hex),
    ]);
    let header_provider = SigsAuthProvider::new(private_key, headers);
    let header_provider = Arc::new(header_provider);

    let vss_client =
        VssClient::new_with_headers(config.vss_url.clone(), retry_policy, header_provider);
    Ok(VssStore::new(vss_client, store_id, vss_seed))
}

pub(crate) async fn build_mirroring_store(
    working_dir: &str,
    vss_store: VssStore<CustomRetryPolicy>,
    remote_lock_shutdown_rx: mpsc::Receiver<()>,
) -> NodeResult<MirroringStore> {
    let (locking_store, previous_holder) =
        build_locking_store(working_dir, vss_store, remote_lock_shutdown_rx).await?;

    let sqlite_file_path = Path::new(working_dir).join("ldk_node_storage.sql");
    let manager = SqliteConnectionManager::file(sqlite_file_path);
    let pool = Pool::new(manager)
        .map_err(|e| PersistError::Sql(format!("Failed to create sqlite connection pool: {e}")))?;
    MirroringStore::new(Handle::current(), pool, locking_store, previous_holder)
        .await
        .map_err(Into::into)
}

async fn build_locking_store(
    working_dir: &str,
    vss_store: VssStore<CustomRetryPolicy>,
    remote_lock_shutdown_rx: mpsc::Receiver<()>,
) -> NodeResult<(Arc<LockingStore>, PreviousHolder)> {
    let instance_id = read_or_generate_instance_id(working_dir)?;
    let (locking_store, previous_holder) = LockingStore::new(instance_id, vss_store)
        .await
        .map_err(|e| PersistError::Generic(format!("Failed to build locking store: {e}")))?;
    let locking_store = Arc::new(locking_store);
    tokio::task::spawn(start_refreshing(
        Arc::clone(&locking_store),
        remote_lock_shutdown_rx,
    ));
    Ok((locking_store, previous_holder))
}

fn read_or_generate_instance_id(working_dir: &str) -> Result<String, PersistError> {
    let filepath = Path::new(working_dir).join("instance_id");
    match fs::read_to_string(&filepath) {
        Ok(instance_id) => Ok(instance_id.trim().to_string()),
        Err(e) if e.kind() == ErrorKind::NotFound => {
            let instance_id = generate_instance_id();
            fs::write(&filepath, &instance_id).map_err(|e| {
                PersistError::Generic(format!(
                    "Failed to create file {}: {e}",
                    filepath.to_string_lossy()
                ))
            })?;
            Ok(instance_id)
        }
        Err(e) => Err(PersistError::Generic(format!(
            "Failed to read file {}: {e}",
            filepath.to_string_lossy()
        ))),
    }
}

fn generate_instance_id() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(8)
        .map(char::from)
        .collect()
}

async fn start_refreshing(locking_store: Arc<LockingStore>, mut shutdown_rx: mpsc::Receiver<()>) {
    loop {
        let duration = match locking_store.refresh_lock().await {
            Ok(until) => {
                trace!("Remote lock was refreshed");
                until.duration_since(SystemTime::now()).unwrap_or_default()
            }
            Err(e) => {
                warn!("Failed to refresh remote lock: {e:?}");
                Duration::from_secs(5)
            }
        };
        tokio::select! {
            biased; // Prioritise shutdown event.
            _ = shutdown_rx.recv() => break,
            _ = tokio::time::sleep(duration) => (),
        }
    }

    info!("Releasing remote lock");
    match locking_store.unlock().await {
        Ok(()) => info!("Remote lock was released"),
        Err(e) => error!("Failed to release remote lock: {e}"),
    };
    // Explicitly drop the receiver to let the sender know we are done with releasing the lock.
    drop(shutdown_rx);
}
