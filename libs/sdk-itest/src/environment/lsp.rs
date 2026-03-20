use std::str::FromStr;

use anyhow::{Error, Result};
use bitcoin::secp256k1::PublicKey;
use bitcoin::{Address, Network};
use reqwest::{Client, Method};
use serde::{Deserialize, Serialize};
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

#[derive(Serialize, Default)]
struct Bolt12OfferRequest {
    amount_msat: Option<u64>,
    description: Option<String>,
    expiry_secs: Option<u32>,
    quantity: Option<u64>,
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
        self.request(Method::GET, "getid", None)
            .await?
            .parse()
            .map_err(Error::msg)
    }

    pub async fn get_new_address(&self) -> Result<Address> {
        let address = self.request(Method::POST, "newaddr", None).await?;
        Ok(Address::from_str(&address)?.require_network(Network::Regtest)?)
    }

    pub async fn get_balance(&self, sync: bool) -> Result<Balance> {
        if sync {
            self.request(Method::POST, "sync", None).await?;
        }
        let balance = self.request(Method::GET, "balance", None).await?;
        serde_json::from_str(&balance).map_err(Error::msg)
    }

    pub async fn get_offer(&self, amount_msat: Option<u64>) -> Result<String> {
        let request = Bolt12OfferRequest {
            amount_msat,
            ..Default::default()
        };
        let request = serde_json::to_vec(&request)?;
        self.request(Method::POST, "newoffer", Some(request)).await
    }

    async fn request(
        &self,
        method: Method,
        command: &str,
        body: Option<Vec<u8>>,
    ) -> Result<String> {
        let mut request = self.client.request(
            method,
            format!("{}/{command}", self.api.external_endpoint()),
        );
        if let Some(body) = body {
            request = request.header(reqwest::header::CONTENT_TYPE, "application/json");
            request = request.body(body);
        }
        let response = request.send().await?.error_for_status()?.text().await?;
        Ok(response)
    }
}
