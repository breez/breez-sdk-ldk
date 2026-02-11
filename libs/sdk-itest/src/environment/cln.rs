use std::str::FromStr;

use anyhow::{Error, Result, anyhow, bail};
use bitcoin::secp256k1::PublicKey;
use bitcoin::{Address, Amount, Network};
use serde_json::Value;
use testcontainers::core::{ExecCommand, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};

use crate::environment::container::ContainerExt;
use crate::environment::log::TracingConsumer;
use crate::environment::{ApiCredentials, Cert, EnvironmentId};

const CA_PEM_FILE: &str = "/data/.lightning/regtest/ca.pem";
const CLIENT_CERT_FILE: &str = "/data/.lightning/regtest/client.pem";
const CLIENT_KEY_FILE: &str = "/data/.lightning/regtest/client-key.pem";
const CLN_HOSTNAME: &str = "cln";
const GRPC_PORT: u16 = 8888;
const IMAGE_NAME: &str = "elementsproject/lightningd";
const IMAGE_TAG: &str = "v25.12.1";
const LIGHTNING_PORT: u16 = 9735;
const RPC_FILE: &str = "/tmp/lightning-rpc";

pub struct Cln {
    api: ApiCredentials,
    pub grpc_api: ApiCredentials,
    container: ContainerAsync<GenericImage>,
}

impl Cln {
    pub async fn new(
        environment_id: &EnvironmentId,
        bitcoind_api: &ApiCredentials,
    ) -> Result<Self> {
        let container = GenericImage::new(IMAGE_NAME, IMAGE_TAG)
            .with_exposed_port(LIGHTNING_PORT.into())
            .with_exposed_port(GRPC_PORT.into())
            .with_wait_for(WaitFor::message_on_stdout(
                "lightningd: Server started with public key",
            ))
            .with_network(environment_id.network_name())
            .with_hostname(CLN_HOSTNAME)
            .with_log_consumer(TracingConsumer::new("cln"))
            .with_cmd([
                "--network=regtest",
                "--alias=cln",
                "--log-level=debug",
                "--grpc-host=0.0.0.0",
                format!("--grpc-port={GRPC_PORT}").as_str(),
                format!("--addr=0.0.0.0:{LIGHTNING_PORT}").as_str(),
                format!("--rpc-file={RPC_FILE}").as_str(),
                format!("--bitcoin-rpcconnect={}", bitcoind_api.host).as_str(),
                format!("--bitcoin-rpcport={}", bitcoind_api.port).as_str(),
                format!("--bitcoin-rpcuser={}", bitcoind_api.username).as_str(),
                format!("--bitcoin-rpcpassword={}", bitcoind_api.password).as_str(),
            ])
            .start()
            .await?;

        let api = ApiCredentials::from_container(&container, LIGHTNING_PORT).await?;
        let cert = Cert {
            ca_pem: container.read_file(CA_PEM_FILE).await?,
            client_cert: container.read_file(CLIENT_CERT_FILE).await?,
            client_key: container.read_file(CLIENT_KEY_FILE).await?,
        };
        let grpc_api = ApiCredentials {
            host: CLN_HOSTNAME.to_string(),
            port: GRPC_PORT,
            cert,
            ..Default::default()
        };

        Ok(Self {
            api,
            grpc_api,
            container,
        })
    }

    pub async fn get_new_address(&self) -> Result<Address> {
        let response = self.cli_json(&["newaddr"]).await?;
        let address = response
            .get("bech32")
            .and_then(Value::as_str)
            .ok_or(anyhow!("CLN newaddr response missing bech32 address"))?;
        Address::from_str(address)?
            .require_network(Network::Regtest)
            .map_err(anyhow::Error::msg)
    }

    pub async fn has_active_channel(&self, peer: &PublicKey) -> Result<bool> {
        Ok(!self.list_active_channels(peer).await?.is_empty())
    }

    pub async fn spendable_onchain_sats(&self) -> Result<u64> {
        let response = self.cli_json(&["listfunds"]).await?;
        let outputs = response
            .get("outputs")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let mut total_sat = 0u64;
        for output in outputs {
            let status = output.get("status").and_then(Value::as_str);
            if !matches!(status, Some("confirmed") | Some("spendable")) {
                continue;
            }
            let msat = output
                .get("amount_msat")
                .and_then(Value::as_u64)
                .ok_or(anyhow!("Failed to parse amount_msat"))?;
            total_sat += msat / 1000;
            continue;
        }

        Ok(total_sat)
    }

    pub async fn get_id(&self) -> Result<String> {
        let info = self.cli_json(&["getinfo"]).await?;
        info.get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or(anyhow!("CLN getinfo response missing id"))
    }

    pub async fn open_channel(
        &self,
        peer: PublicKey,
        address: String,
        funding_amount: Amount,
        push_amount: Amount,
    ) -> Result<()> {
        self.cli_json(&["connect", &peer.to_string(), &address])
            .await?;
        self.cli_json(&[
            "fundchannel",
            "-k",
            &format!("id={peer}"),
            &format!("amount={}", funding_amount.to_sat()),
            &format!("push_msat={}", push_amount.to_sat() * 1000),
        ])
        .await?;
        Ok(())
    }

    pub async fn lightning_address(&self) -> Result<String> {
        Ok(format!("{}@{}", self.get_id().await?, self.api.address()))
    }

    async fn list_active_channels(&self, peer: &PublicKey) -> Result<Vec<String>> {
        let response = self.cli_json(&["listchannels"]).await?;
        let channels = response
            .get("channels")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let peer = peer.to_string();
        let mut active = Vec::new();
        for channel in channels {
            let destination = channel.get("destination").and_then(Value::as_str);
            let active_flag = channel.get("active").and_then(Value::as_bool);
            if !matches!(destination, Some(dest) if dest == peer) || active_flag != Some(true) {
                continue;
            }
            let scid = channel
                .get("short_channel_id")
                .and_then(Value::as_str)
                .or_else(|| channel.get("channel_id").and_then(Value::as_str));
            if let Some(id) = scid {
                active.push(id.to_string());
            }
        }
        Ok(active)
    }

    async fn cli_json(&self, args: &[&str]) -> Result<Value> {
        let stdout = self.cli(args).await?;
        let value: Value = serde_json::from_str(&stdout).map_err(Error::msg)?;
        if let Some(code) = value.get("code") {
            let message = value.get("message").unwrap_or_default();
            bail!("CLN Error [{code}]: {message}");
        }
        Ok(value)
    }

    async fn cli(&self, args: &[&str]) -> Result<String> {
        let mut cmd = Vec::with_capacity(args.len() + 2);
        cmd.push("lightning-cli".to_string());
        cmd.push(format!("--rpc-file={RPC_FILE}"));
        for arg in args {
            cmd.push((*arg).to_string());
        }
        let mut output = self.container.exec(ExecCommand::new(cmd)).await?;
        let stdout = output.stdout_to_vec().await?;
        Ok(String::from_utf8_lossy(&stdout).trim().to_string())
    }
}
