use core::convert::TryInto;
use std::collections::HashMap;
use std::sync::Arc;

use bitcoin::io::{Error, ErrorKind};
use ldk_node::lightning::util::persist::{KVStore as KVStoreAsync, KVStoreSync};
use ldk_node::lightning_types::payment::{PaymentHash, PaymentPreimage};
use ldk_node::DynStore;

use crate::error::{SdkError, SdkResult};
use crate::ldk::utils::Hex;
use crate::persist::payment_store::PaymentStore;
use crate::{LnPaymentInfo, LnUrlInfo};

pub(crate) const BREEZ_NS: &str = "breez";
pub(crate) const PREIMAGES_NS: &str = "preimages";
pub(crate) const LN_INFOS_NS: &str = "ln_infos";
pub(crate) const LNURL_INFOS_NS: &str = "lnurl_infos";

pub(crate) type KVStore = Arc<DynStore>;

#[derive(Clone)]
pub(crate) struct Store {
    kv_store: KVStore,
}

impl Store {
    pub(crate) fn new(kv_store: KVStore) -> Self {
        Self { kv_store }
    }

    pub(crate) fn store_preimage(
        &self,
        hash: &PaymentHash,
        preimage: &PaymentPreimage,
    ) -> Result<(), Error> {
        KVStoreSync::write(
            self.kv_store.as_ref(),
            BREEZ_NS,
            PREIMAGES_NS,
            &hash.to_hex(),
            preimage.0.to_vec(),
        )
    }

    pub(crate) fn load_preimage(&self, hash: &PaymentHash) -> Result<PaymentPreimage, Error> {
        let preimage = KVStoreSync::read(
            self.kv_store.as_ref(),
            BREEZ_NS,
            PREIMAGES_NS,
            &hash.to_hex(),
        )?;
        match preimage.as_slice().try_into() {
            Ok(preimage) => Ok(PaymentPreimage(preimage)),
            Err(err) => Err(Error::new(ErrorKind::InvalidData, err)),
        }
    }
}

#[tonic::async_trait]
impl PaymentStore for Store {
    async fn set_ln_info(&self, payment_id: &str, info: &LnPaymentInfo) -> SdkResult<()> {
        let info = serde_json::to_vec(info)?;
        KVStoreAsync::write(
            self.kv_store.as_ref(),
            BREEZ_NS,
            LN_INFOS_NS,
            payment_id,
            info,
        )
        .await
        .map_err(Into::into)
    }

    async fn set_lnurl_info(&self, payment_id: &str, info: &LnUrlInfo) -> SdkResult<()> {
        let info = serde_json::to_vec(info)?;
        KVStoreAsync::write(
            self.kv_store.as_ref(),
            BREEZ_NS,
            LNURL_INFOS_NS,
            payment_id,
            info,
        )
        .await
        .map_err(Into::into)
    }

    async fn get_ln_info(&self, payment_ids: &[&str]) -> SdkResult<HashMap<String, LnPaymentInfo>> {
        let mut infos = HashMap::new();
        for payment_id in payment_ids {
            match KVStoreAsync::read(self.kv_store.as_ref(), BREEZ_NS, LN_INFOS_NS, payment_id)
                .await
            {
                Ok(raw) => {
                    let info = serde_json::from_slice::<LnPaymentInfo>(&raw)?;
                    infos.insert((*payment_id).to_string(), info);
                }
                Err(err) if err.kind() == ErrorKind::NotFound => {}
                Err(err) => return Err(SdkError::generic(&err.to_string())),
            }
        }
        Ok(infos)
    }

    async fn get_lnurl_info(&self, payment_ids: &[&str]) -> SdkResult<HashMap<String, LnUrlInfo>> {
        let mut infos = HashMap::new();
        for payment_id in payment_ids {
            match KVStoreAsync::read(self.kv_store.as_ref(), BREEZ_NS, LNURL_INFOS_NS, payment_id)
                .await
            {
                Ok(raw) => {
                    let info = serde_json::from_slice::<LnUrlInfo>(&raw)?;
                    infos.insert((*payment_id).to_string(), info);
                }
                Err(err) if err.kind() == ErrorKind::NotFound => {}
                Err(err) => return Err(SdkError::generic(&err.to_string())),
            }
        }
        Ok(infos)
    }
}

impl From<Error> for SdkError {
    fn from(err: Error) -> Self {
        SdkError::Generic {
            err: err.to_string(),
        }
    }
}
