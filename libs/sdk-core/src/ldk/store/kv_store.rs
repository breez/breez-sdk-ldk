use core::convert::TryInto;
use std::sync::Arc;

use bitcoin::io::{Error, ErrorKind};
use ldk_node::lightning::util::persist::KVStoreSync;
use ldk_node::lightning_types::payment::{PaymentHash, PaymentPreimage};
use ldk_node::DynStore;

use crate::ldk::utils::Hex;

pub(crate) const BREEZ_NS: &str = "breez";
pub(crate) const BOLT11_NS: &str = "bolt11";
pub(crate) const PREIMAGES_NS: &str = "preimages";

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

    pub(crate) fn store_bolt11(&self, hash: &str, bolt11: String) -> Result<(), Error> {
        KVStoreSync::write(
            self.kv_store.as_ref(),
            BREEZ_NS,
            BOLT11_NS,
            hash,
            bolt11.into_bytes(),
        )
    }

    pub(crate) fn load_bolt11(&self, hash: &PaymentHash) -> Result<Option<String>, Error> {
        match KVStoreSync::read(self.kv_store.as_ref(), BREEZ_NS, BOLT11_NS, &hash.to_hex()) {
            Ok(bolt11) => Ok(Some(String::from_utf8_lossy(&bolt11).into_owned())),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }
}
