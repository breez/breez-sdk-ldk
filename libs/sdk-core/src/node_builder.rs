use std::sync::Arc;

use crate::backup::BackupTransport;
use crate::ldk::{Ldk, LdkBackupTransport};
use crate::models::{Config, LspAPI};
use crate::node_api::{NodeAPI, NodeResult};
use crate::persist::db::SqliteStorage;

pub struct NodeImpls {
    pub node: Arc<dyn NodeAPI>,
    pub backup_transport: Arc<dyn BackupTransport>,
    pub lsp: Option<Arc<dyn LspAPI>>,
}

#[allow(unused_variables)]
pub async fn build_node(
    config: Config,
    seed: Vec<u8>,
    restore_only: Option<bool>,
    persister: Arc<SqliteStorage>,
) -> NodeResult<NodeImpls> {
    let backup_transport = Arc::new(LdkBackupTransport::new(&config, &seed)?);
    let ldk = Ldk::build(config, &seed, restore_only).await?;
    let ldk = Arc::new(ldk);
    let lsp: Option<Arc<dyn LspAPI>> = Some(ldk.clone());
    Ok(NodeImpls {
        node: ldk,
        backup_transport,
        lsp,
    })
}
