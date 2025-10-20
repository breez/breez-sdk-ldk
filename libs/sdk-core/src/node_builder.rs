use std::sync::Arc;

use crate::backup::BackupTransport;
use crate::breez_services::Receiver;
use crate::ldk::{Ldk, LdkBackupTransport};
use crate::models::{Config, LspAPI};
use crate::node_api::{NodeAPI, NodeResult};
use crate::persist::db::SqliteStorage;

pub struct NodeImpls {
    pub node: Arc<dyn NodeAPI>,
    pub backup_transport: Arc<dyn BackupTransport>,
    pub lsp: Option<Arc<dyn LspAPI>>,
    pub receiver: Option<Arc<dyn Receiver>>,
}

#[allow(unused_variables)]
pub async fn build_node(
    config: Config,
    seed: Vec<u8>,
    restore_only: Option<bool>,
    persister: Arc<SqliteStorage>,
) -> NodeResult<NodeImpls> {
    let ldk = Ldk::build(config, &seed, restore_only).await?;
    let ldk = Arc::new(ldk);
    let backup_transport = Arc::new(LdkBackupTransport {});
    let lsp: Option<Arc<dyn LspAPI>> = Some(ldk.clone());
    let receiver: Option<Arc<dyn Receiver>> = Some(ldk.clone());
    Ok(NodeImpls {
        node: ldk,
        backup_transport,
        lsp,
        receiver,
    })
}
