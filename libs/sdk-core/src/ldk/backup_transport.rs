use vss_client::error::VssError;
use vss_client::util::retry::{ExponentialBackoffRetryPolicy, MaxAttemptsRetryPolicy};

use crate::backup::{BackupState, BackupTransport};
use crate::error::{SdkError, SdkResult};
use crate::ldk::store::{VersionedStore, VssStore};
use crate::ldk::store_builder;
use crate::Config;

pub(crate) struct LdkBackupTransport {
    store: VssStore<MaxAttemptsRetryPolicy<ExponentialBackoffRetryPolicy<VssError>>>,
}

impl LdkBackupTransport {
    const KEY: &str = "backup";

    pub fn new(config: &Config, seed: &[u8]) -> Self {
        let store = store_builder::build_vss_store(config, seed, "backups");
        Self { store }
    }
}

#[tonic::async_trait]
impl BackupTransport for LdkBackupTransport {
    async fn pull(&self) -> SdkResult<Option<BackupState>> {
        debug!("Pulling backup");
        match self.store.get(Self::KEY.to_string()).await {
            Ok(Some((data, version))) => Ok(Some(BackupState {
                generation: version as u64,
                data,
            })),
            Ok(None) => Ok(None),
            Err(e) => Err(SdkError::generic(&e.to_string())),
        }
    }

    async fn push(&self, version: Option<u64>, hex: Vec<u8>) -> SdkResult<u64> {
        debug!("Pushing backup with version {version:?}");
        let version = version.unwrap_or_default() as i64;
        match self.store.put(Self::KEY.to_string(), hex, version).await {
            Ok(()) => Ok((version + 1) as u64),
            Err(e) => Err(SdkError::generic(&e.to_string())),
        }
    }
}
