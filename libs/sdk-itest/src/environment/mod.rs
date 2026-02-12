mod bitcoind;
mod cln;
mod container;
mod esplora;
mod lnd;
mod log;
mod lsp;
mod mempool;
mod rgs;
mod vss;

use std::path::PathBuf;

use anyhow::Result;
use bitcoin::Amount;
use bitcoin::secp256k1::PublicKey;
use bitcoind::Bitcoind;
use cln::Cln;
use esplora::Esplora;
pub use lnd::Lnd;
use lsp::Lsp;
use mempool::Mempool;
use rand::Rng;
use rgs::Rgs;
use testcontainers::{ContainerAsync, Image};
use testdir::testdir;
use tokio::sync::OnceCell;
use tokio::try_join;
use tracing::{info, instrument};
use vss::Vss;

use crate::wait_for;

#[derive(Clone, Debug)]
pub struct EnvironmentId {
    id: String,
    working_dir: PathBuf,
}

impl Default for EnvironmentId {
    fn default() -> Self {
        Self::new()
    }
}

impl EnvironmentId {
    pub fn new() -> Self {
        let id: u32 = rand::rng().random_range(0..0xFFFFFFFF);
        let id = hex::encode(id.to_le_bytes());
        let mut working_dir = testdir!();
        working_dir.push(id.clone());
        Self { id, working_dir }
    }

    pub fn network_name(&self) -> String {
        format!("network-{}", self.id)
    }

    pub fn working_dir(&self) -> &PathBuf {
        &self.working_dir
    }
}

impl std::fmt::Display for EnvironmentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.id)
    }
}

#[derive(Default, Clone)]
pub struct Cert {
    pub ca_pem: Vec<u8>,
    pub client_cert: Vec<u8>,
    pub client_key: Vec<u8>,
}

#[derive(Default, Clone)]
pub struct ApiCredentials {
    pub host: String,
    pub port: u16,
    pub external_port: u16,
    pub path: String,
    pub username: String,
    pub password: String,
    pub cert: Cert,
}

impl ApiCredentials {
    pub async fn from_container<I>(container: &ContainerAsync<I>, port: u16) -> Result<Self>
    where
        I: Image,
    {
        let host = container.get_bridge_ip_address().await?.to_string();
        let external_port = container.get_host_port_ipv4(port).await?;

        Ok(Self {
            host,
            port,
            external_port,
            ..Default::default()
        })
    }

    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub fn endpoint(&self) -> String {
        format!("http://{}{}", self.address(), self.path)
    }

    pub fn external_address(&self) -> String {
        format!("127.0.0.1:{}", self.external_port)
    }

    pub fn external_endpoint(&self) -> String {
        format!("http://{}{}", self.external_address(), self.path)
    }
}

#[derive(Default)]
pub struct Environment {
    environmnet_id: EnvironmentId,
    bitcoind: OnceCell<Bitcoind>,
    esplora: OnceCell<Esplora>,
    mempool: OnceCell<Mempool>,
    vss: OnceCell<Vss>,
    lsp: OnceCell<Lsp>,
    lnd: OnceCell<Lnd>,
    cln: OnceCell<Cln>,
    channel: OnceCell<()>,
    cln_channel: OnceCell<()>,
    rgs: OnceCell<Rgs>,
}

impl Environment {
    #[instrument(skip(self))]
    pub async fn bitcoind_api(&self) -> Result<&ApiCredentials> {
        Ok(&self.bitcoind().await?.api)
    }

    #[instrument(skip(self))]
    async fn bitcoind_rest_api(&self) -> Result<&ApiCredentials> {
        Ok(&self.bitcoind().await?.rest_api)
    }

    #[instrument(skip(self))]
    pub async fn esplora_api(&self) -> Result<&ApiCredentials> {
        let esplora = self
            .esplora
            .get_or_try_init(|| async {
                info!("Initializing esplora");
                let bitcoind_api = self.bitcoind_api().await?;
                let result = Esplora::new(&self.environmnet_id, bitcoind_api).await;
                log_result(result, "Esplora")
            })
            .await?;

        Ok(&esplora.api)
    }

    #[instrument(skip(self))]
    pub async fn mempool_api(&self) -> Result<&ApiCredentials> {
        let mempool = self
            .mempool
            .get_or_try_init(|| async {
                info!("Initializing mempool");
                let result = Mempool::new(
                    &self.environmnet_id,
                    self.bitcoind_api(),
                    self.esplora_api(),
                )
                .await;
                log_result(result, "Mempool")
            })
            .await?;
        Ok(&mempool.api)
    }

    #[instrument(skip(self))]
    pub async fn vss_api(&self) -> Result<&ApiCredentials> {
        let vss = self
            .vss
            .get_or_try_init(|| async {
                info!("Initializing VSS");
                log_result(Vss::new(&self.environmnet_id).await, "VSS")
            })
            .await?;
        Ok(&vss.api)
    }

    #[instrument(skip(self))]
    pub async fn lsp_external_address(&self) -> Result<String> {
        let lsp = self.lsp().await?;
        let pubkey = lsp.get_node_id().await?;
        let address = lsp.lightning_api.external_address();
        Ok(format!("{pubkey}@{address}"))
    }

    #[instrument(skip(self))]
    async fn lsp_address(&self) -> Result<String> {
        let lsp = self.lsp().await?;
        let pubkey = lsp.get_node_id().await?;
        let address = lsp.lightning_api.address();
        Ok(format!("{pubkey}@{address}"))
    }

    #[instrument(skip(self))]
    pub async fn lnd(&self) -> Result<&Lnd> {
        self.lnd.get_or_try_init(|| self.init_lnd()).await
    }

    #[instrument(skip(self))]
    pub async fn lnd_with_channel(&self) -> Result<&Lnd> {
        self.channel.get_or_try_init(|| self.open_channel()).await?;
        self.lnd().await
    }

    #[instrument(skip(self))]
    pub async fn cln(&self) -> Result<&Cln> {
        self.cln.get_or_try_init(|| self.init_cln()).await
    }

    #[instrument(skip(self))]
    pub async fn cln_with_channel(&self) -> Result<&Cln> {
        self.cln_channel
            .get_or_try_init(|| self.cln_open_channel())
            .await?;
        self.cln().await
    }

    #[instrument(skip(self))]
    pub async fn rgs(&self) -> Result<&ApiCredentials> {
        let rgs = self
            .rgs
            .get_or_try_init(|| async {
                info!("Initializing RGS");
                let bitcoind_rest_api = self.bitcoind_rest_api();
                let lsp_address = self.lsp_address();
                let lnd = self.lnd_with_channel();
                let result =
                    Rgs::new(&self.environmnet_id, bitcoind_rest_api, lsp_address, lnd).await;
                log_result(result, "RGS")
            })
            .await?;
        Ok(&rgs.api)
    }

    #[instrument(skip(self))]
    async fn init_lnd(&self) -> Result<Lnd> {
        info!("Initializing LND");
        let bitcoind_api = self.bitcoind_api().await?;
        let bitcoind_zmq_block = &self.bitcoind().await?.zmq_block;
        let bitcoind_zmq_tx = &self.bitcoind().await?.zmq_tx;
        let result = Lnd::new(
            &self.environmnet_id,
            bitcoind_api,
            bitcoind_zmq_block,
            bitcoind_zmq_tx,
        )
        .await;
        log_result(result, "LND")
    }

    #[instrument(skip(self))]
    async fn init_cln(&self) -> Result<Cln> {
        info!("Initializing CLN");
        let bitcoind_api = self.bitcoind_api().await?;
        let result = Cln::new(&self.environmnet_id, bitcoind_api).await;
        log_result(result, "CLN")
    }

    #[instrument(skip(self))]
    async fn open_channel(&self) -> Result<()> {
        info!("Opening LND -> LSP channel...");
        let (bitcoind, lsp, lnd) = try_join!(self.bitcoind(), self.lsp(), self.lnd())?;

        let amount = Amount::ONE_BTC;
        let address = lsp.get_new_address().await?;
        bitcoind.fund_address(&address, amount).await?;

        let address = lnd.get_new_address().await?;
        bitcoind.fund_address(&address, amount).await?;

        bitcoind.generate_blocks(1).await?;
        info!("Waiting for LSP to see on-chain funds...");
        wait_for!(lsp.get_balance(true).await?.spendable_onchain_sats >= amount.to_sat());

        let lsp_id = lsp.get_node_id().await?;
        let lsp_address = lsp.lightning_api.address();
        let funding_amount = amount / 2;
        let push_amount = funding_amount / 2;
        lnd.open_channel(lsp_id, lsp_address, funding_amount, push_amount)
            .await?;
        bitcoind.generate_blocks(6).await?;
        info!("Waiting for LND to see the channel active...");
        wait_for!(!lnd.list_active_channels(&lsp_id).await?.is_empty());

        info!("LND -> LSP channel opened successfully");
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn cln_open_channel(&self) -> Result<()> {
        info!("Opening CLN -> LND channel...");
        let (bitcoind, cln, lnd) = try_join!(self.bitcoind(), self.cln(), self.lnd())?;

        let amount = Amount::ONE_BTC;
        let address = cln.get_new_address().await?;
        bitcoind.fund_address(&address, amount).await?;
        bitcoind.generate_blocks(1).await?;
        info!("Waiting for CLN to see on-chain funds...");
        wait_for!(cln.spendable_onchain_sats().await? >= amount.to_sat());

        let lnd_id: PublicKey = lnd.get_id().await?.parse()?;
        let lnd_address = lnd.lightning_api.address();
        let funding_amount = amount / 2;
        let push_amount = funding_amount / 2;
        cln.open_channel(lnd_id, lnd_address, funding_amount, push_amount)
            .await?;
        bitcoind.generate_blocks(6).await?;
        info!("Waiting for CLN to see the channel active...");
        wait_for!(cln.has_active_channel(&lnd_id).await?);

        info!("CLN -> LND channel opened successfully");
        Ok(())
    }

    #[instrument(skip(self))]
    async fn lsp(&self) -> Result<&Lsp> {
        self.lsp
            .get_or_try_init(|| async {
                info!("Initializing LSP");
                let esplora_api = self.esplora_api().await?;
                let result = Lsp::new(&self.environmnet_id, esplora_api).await;
                log_result(result, "LSP")
            })
            .await
    }

    #[instrument(skip(self))]
    async fn bitcoind(&self) -> Result<&Bitcoind> {
        let bitcoind = self
            .bitcoind
            .get_or_try_init(|| async {
                info!("Initializing bitcoind");
                let result = Bitcoind::new(&self.environmnet_id).await;
                let mut bitcoind = log_result(result, "Bitcoind")?;
                if let Err(e) = bitcoind.initialize().await {
                    info!("Bitcoind initialization failed: {e}");
                    return Err(e);
                }
                Ok(bitcoind)
            })
            .await?;
        Ok(bitcoind)
    }
}

fn log_result<T>(result: Result<T>, service: &str) -> Result<T> {
    match &result {
        Ok(_) => info!("{service} started successfully"),
        Err(e) => info!("{service} failed: {e}"),
    }
    result
}
