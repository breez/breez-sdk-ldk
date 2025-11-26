use std::sync::Arc;

use ldk_node::lightning::io::ErrorKind;
use ldk_node::lightning::util::persist::KVStoreSync;
use ldk_node::DynStore;

use crate::node_api::NodeError;

const PRIMARY_NS: &str = "breez";
const SECONDARY_NS: &str = "restore_state";
const KEY_INITIALIZED: &str = "initialized";
const VALUE_INITIALIZED: &[u8] = b"1";

/// Tracks whether an LDK node instance has persisted state using the configured KV store.
pub(crate) struct RestoreStateTracker {
    kv_store: Arc<DynStore>,
}

impl RestoreStateTracker {
    pub(crate) fn new(kv_store: Arc<DynStore>) -> Self {
        Self { kv_store }
    }

    pub(crate) fn is_initialized(&self) -> Result<bool, NodeError> {
        match KVStoreSync::read(
            self.kv_store.as_ref(),
            PRIMARY_NS,
            SECONDARY_NS,
            KEY_INITIALIZED,
        ) {
            Ok(value) => Ok(value == VALUE_INITIALIZED),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(false),
            Err(err) => Err(NodeError::Generic(format!(
                "Failed to read restore state marker: {err}"
            ))),
        }
    }

    pub(crate) fn mark_initialized(&self) -> Result<(), NodeError> {
        KVStoreSync::write(
            self.kv_store.as_ref(),
            PRIMARY_NS,
            SECONDARY_NS,
            KEY_INITIALIZED,
            VALUE_INITIALIZED.to_vec(),
        )
        .map_err(|err| NodeError::Generic(format!("Failed to write restore state marker: {err}")))
    }
}
