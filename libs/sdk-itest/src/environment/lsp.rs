use std::str::FromStr;

use anyhow::{Error, Result};
use bitcoin::secp256k1::PublicKey;
use bitcoin::{Address, Network};
use reqwest::{Client, Method};
use serde::Deserialize;
use testcontainers::core::WaitFor;
use testcontainers::core::wait::HttpWaitStrategy;
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};

use crate::environment::log::LogConsumer;
use crate::environment::{ApiCredentials, EnvironmentId};

const LIGHTNING_PORT: u16 = 9735;
const RPC_PORT: u16 = 9736;

pub struct Lsp {
    pub lightning_api: ApiCredentials,
    api: ApiCredentials,
    client: Client,
    _container: ContainerAsync<GenericImage>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
pub struct Balance {
    pub total_onchain_sats: u64,
    pub spendable_onchain_sats: u64,
    pub lightning_sats: u64,
}

impl Lsp {
    pub async fn new(environment_id: &EnvironmentId, esplora_api: &ApiCredentials) -> Result<Self> {
        let container = GenericImage::new("lsps2-server", "latest")
            .with_exposed_port(LIGHTNING_PORT.into())
            .with_exposed_port(RPC_PORT.into())
            .with_wait_for(WaitFor::Http(Box::new(
                HttpWaitStrategy::new("/getid")
                    .with_port(RPC_PORT.into())
                    .with_expected_status_code(200u16),
            )))
            .with_network(environment_id.network_name())
            .with_log_consumer(LogConsumer::new("lsps2-server"))
            .with_env_var("ESPLORA_URL", esplora_api.endpoint())
            .with_env_var("LISTENING_ADDRESS", format!("0.0.0.0:{LIGHTNING_PORT}"))
            .with_env_var("NETWORK", "regtest")
            .with_env_var("RPC_LISTEN_ADDRESS", format!("0.0.0.0:{RPC_PORT}"))
            .with_env_var("STORAGE_PATH", "/data")
            .with_env_var(
                "MNEMONIC",
                "hip liar they despair head rookie act fresh long joy power orient",
            )
            .start()
            .await?;
        let api = ApiCredentials::from_container(&container, RPC_PORT).await?;
        let lightning_api = ApiCredentials::from_container(&container, LIGHTNING_PORT).await?;
        let client = Client::new();
        Ok(Self {
            lightning_api,
            api,
            client,
            _container: container,
        })
    }

    pub async fn get_node_id(&self) -> Result<PublicKey> {
        self.request(Method::GET, "getid")
            .await?
            .parse()
            .map_err(Error::msg)
    }

    pub async fn get_new_address(&self) -> Result<Address> {
        let address = self.request(Method::POST, "newaddr").await?;
        Ok(Address::from_str(&address)?.require_network(Network::Regtest)?)
    }

    pub async fn get_balance(&self, sync: bool) -> Result<Balance> {
        if sync {
            self.request(Method::POST, "sync").await?;
        }
        let balance = self.request(Method::GET, "balance").await?;
        serde_json::from_str(&balance).map_err(Error::msg)
    }

    async fn request(&self, method: Method, command: &str) -> Result<String> {
        let response = self
            .client
            .request(
                method,
                format!("{}/{command}", self.api.external_endpoint()),
            )
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        Ok(response)
    }
}
