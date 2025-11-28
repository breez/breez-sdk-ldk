use std::str::FromStr;
use std::time::Duration;

use anyhow::{Error, Result, anyhow, bail, ensure};
use bitcoin::address::NetworkUnchecked;
use bitcoin::{Address, Amount, Denomination, Network, Txid};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use testcontainers::core::WaitFor;
use testcontainers::core::wait::LogWaitStrategy;
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};
use tracing::info;

use crate::environment::log::TracingConsumer;
use crate::environment::{ApiCredentials, EnvironmentId};

const BITCOIND_VERSION: &str = "v28.0";
const BITCOIND_DOCKER_IMAGE: &str = "lncm/bitcoind";
const RPC_USER: &str = "rpcuser";
const RPC_PASSWORD: &str = "rpcpassword";
const RPC_PORT: u16 = 8332;
const ZMQPUBRAWBLOCK_RPC_PORT: u16 = 28332;
const ZMQPUBRAWTX_RPC_PORT: u16 = 28333;
const DEFAULT_MINING_ADDRESS: &str = "bcrt1qs758ursh4q9z627kt3pp5yysm78ddny6txaqgw";

pub struct Bitcoind {
    pub api: ApiCredentials,
    pub rest_api: ApiCredentials,
    pub zmq_block: ApiCredentials,
    pub zmq_tx: ApiCredentials,
    mining_address: Address,
    client: Client,
    _container: ContainerAsync<GenericImage>,
}

#[derive(Serialize, Deserialize, Debug)]
struct RpcResponse<T> {
    result: Option<T>,
    error: Option<Value>,
}

#[derive(Deserialize)]
struct ListUnspentEntry {
    amount: f64,
}

impl Bitcoind {
    pub async fn new(environment_id: &EnvironmentId) -> Result<Self> {
        let container = GenericImage::new(BITCOIND_DOCKER_IMAGE, BITCOIND_VERSION)
            .with_exposed_port(RPC_PORT.into())
            .with_exposed_port(ZMQPUBRAWBLOCK_RPC_PORT.into())
            .with_exposed_port(ZMQPUBRAWTX_RPC_PORT.into())
            .with_wait_for(WaitFor::Log(LogWaitStrategy::stdout(
                "init message: Done loading",
            )))
            .with_network(environment_id.network_name())
            .with_log_consumer(TracingConsumer::new("bitcoind"))
            .with_cmd([
                "-regtest",
                "-server",
                "-logtimestamps",
                "-nolisten",
                "-addresstype=bech32",
                "-txindex",
                "-fallbackfee=0.00000253",
                "-debug=mempool",
                "-debug=rpc",
                format!("-rpcport={RPC_PORT}").as_str(),
                format!("-rpcuser={RPC_USER}").as_str(),
                format!("-rpcpassword={RPC_PASSWORD}").as_str(),
                format!("-zmqpubrawblock=tcp://0.0.0.0:{ZMQPUBRAWBLOCK_RPC_PORT}").as_str(),
                format!("-zmqpubrawtx=tcp://0.0.0.0:{ZMQPUBRAWTX_RPC_PORT}").as_str(),
                "-rpcbind=0.0.0.0",
                "-rpcallowip=0.0.0.0/0",
                "-rest",
            ])
            .start()
            .await?;

        info!("Bitcoind container running");
        let mut api = ApiCredentials::from_container(&container, RPC_PORT).await?;
        api.username = RPC_USER.to_string();
        api.password = RPC_PASSWORD.to_string();
        let mut rest_api = ApiCredentials::from_container(&container, RPC_PORT).await?;
        rest_api.path = "/rest".to_string();
        let zmq_block = ApiCredentials::from_container(&container, ZMQPUBRAWBLOCK_RPC_PORT).await?;
        let zmq_tx = ApiCredentials::from_container(&container, ZMQPUBRAWTX_RPC_PORT).await?;
        // Create instance with RPC URL
        let instance = Self {
            mining_address: Address::from_str(DEFAULT_MINING_ADDRESS)?
                .require_network(Network::Regtest)?,
            api,
            rest_api,
            zmq_block,
            zmq_tx,
            client: Client::new(),
            _container: container,
        };

        info!("Created bitcoind container. Ensure wallet created.");

        // Wait for RPC to be available and create wallet using the RPC API
        instance.ensure_wallet_created().await?;

        info!("Bitcoin wallet is created.");
        Ok(instance)
    }

    async fn ensure_wallet_created(&self) -> Result<()> {
        // Try to create wallet with retries
        let max_retries = 10;
        let mut retries = 0;
        let mut last_error = None;

        while retries < max_retries {
            match self.create_wallet_rpc().await {
                Ok(_) => {
                    info!("Successfully created or confirmed bitcoin wallet");
                    return Ok(());
                }
                Err(e) => {
                    retries += 1;
                    info!("Failed to create wallet (retry {retries}/{max_retries}): {e}");
                    last_error = Some(e);
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
        }

        Err(last_error.unwrap_or(anyhow!("Failed to create wallet after retries")))
    }

    async fn create_wallet_rpc(&self) -> Result<()> {
        self.rpc_call::<Value>("createwallet", &[json!("default")])
            .await?;
        Ok(())
    }

    pub async fn initialize(&mut self) -> Result<()> {
        // Create a new address from bitcoind's internal wallet
        let new_address = self.get_new_address().await?;
        info!("Created new mining address: {new_address}");
        // Update the mining address
        self.mining_address = new_address;

        // Generate some blocks to mature the coinbase
        self.generate_blocks(101).await?;
        info!("Generated 101 blocks for bitcoind");
        Ok(())
    }

    pub async fn get_new_address(&self) -> Result<Address> {
        self.rpc_call::<String>("getnewaddress", &[json!("mining"), json!("bech32")])
            .await?
            .parse::<Address<NetworkUnchecked>>()?
            .require_network(Network::Regtest)
            .map_err(Error::msg)
    }

    pub async fn generate_blocks(&self, count: u32) -> Result<Vec<String>> {
        let address = self.mining_address.to_string();
        self.rpc_call::<Vec<String>>("generatetoaddress", &[json!(count), json!(address)])
            .await
    }

    pub async fn fund_address(&self, address: &Address, amount: Amount) -> Result<Txid> {
        let amount = amount.to_string_in(Denomination::Bitcoin);
        self.rpc_call::<String>(
            "sendtoaddress",
            &[json!(address.to_string()), json!(amount)],
        )
        .await?
        .parse()
        .map_err(Error::msg)
    }

    pub async fn get_address_balance(&self, address: &Address) -> Result<Amount> {
        let entries = self
            .rpc_call::<Vec<ListUnspentEntry>>(
                "listunspent",
                &[json!(0), json!(9999999), json!([address.to_string()])],
            )
            .await?;

        let balance = entries
            .iter()
            .map(|entry| Amount::from_btc(entry.amount).map_err(Error::msg))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .sum();

        Ok(balance)
    }

    async fn rpc_call<T: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        params: &[Value],
    ) -> Result<T> {
        let request = json!({
            "jsonrpc": "1.0",
            "id": "rust-client",
            "method": method,
            "params": params,
        });

        let response = self
            .client
            .post(self.api.external_endpoint())
            .basic_auth(&self.api.username, Some(&self.api.password))
            .json(&request)
            .send()
            .await?;

        ensure!(
            response.status().is_success(),
            "bitcoind returned error status: {}",
            response.status()
        );

        let response: RpcResponse<T> = response.json().await?;
        match (response.result, response.error) {
            (Some(result), None) => Ok(result),
            (None, Some(error)) => bail!("RPC error: {error:?}"),
            _ => bail!("Invalid RPC response"),
        }
    }
}
