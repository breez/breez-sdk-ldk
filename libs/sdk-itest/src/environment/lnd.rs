use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{Result, bail};
use bitcoin::secp256k1::PublicKey;
use bitcoin::{Address, Amount, Network};
use testcontainers::core::{ExecCommand, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};
use tokio::sync::Mutex;
use tonic_lnd::Client;
use tonic_lnd::lnrpc::{
    AddressType, ConnectPeerRequest, GetInfoRequest, Invoice, LightningAddress,
    ListChannelsRequest, NewAddressRequest, OpenChannelRequest, SendRequest,
};

use crate::environment::log::TracingConsumer;
use crate::environment::{ApiCredentials, EnvironmentId};

const IMAGE_NAME: &str = "lightninglabs/lnd";
const IMAGE_TAG: &str = "v0.19.3-beta";
const RPC_PORT: u16 = 10009;

pub struct Lnd {
    pub container: ContainerAsync<GenericImage>,
    client: Mutex<Client>,
}

impl Lnd {
    pub async fn new(
        environment_id: &EnvironmentId,
        bitcoind_api: &ApiCredentials,
        bitcoind_zmq_block: &ApiCredentials,
        bitcoind_zmq_tx: &ApiCredentials,
    ) -> Result<Self> {
        let container = GenericImage::new(IMAGE_NAME, IMAGE_TAG)
            .with_exposed_port(RPC_PORT.into())
            .with_wait_for(WaitFor::message_on_stdout("Server listening on"))
            .with_network(environment_id.network_name())
            .with_log_consumer(TracingConsumer::new("lnd"))
            .with_cmd([
                "--bitcoin.regtest",
                "--bitcoin.node=bitcoind",
                format!("--bitcoind.rpcuser={}", bitcoind_api.username).as_str(),
                format!("--bitcoind.rpcpass={}", bitcoind_api.password).as_str(),
                format!("--bitcoind.rpchost={}", bitcoind_api.address()).as_str(),
                format!(
                    "--bitcoind.zmqpubrawblock=tcp://{}",
                    bitcoind_zmq_block.address()
                )
                .as_str(),
                format!("--bitcoind.zmqpubrawtx=tcp://{}", bitcoind_zmq_tx.address()).as_str(),
                "--noseedbackup",
                "--lnddir=/root/.lnd",
                "--tlscertpath=/root/.lnd/tls.cert",
                "--tlskeypath=/root/.lnd/tls.key",
                "--rpclisten=0.0.0.0:10009",
                "--accept-keysend",
                "--accept-amp",
                "--protocol.wumbo-channels",
                "--tlsextradomain=localhost",
            ])
            .start()
            .await?;

        let working_dir = environment_id.working_dir().join("lnd");
        std::fs::create_dir_all(&working_dir)?;
        let cert_path = working_dir.join("tls.cert");
        let macaroon_path = working_dir.join("admin.macaroon");
        copy_files(&container, "/root/.lnd/tls.cert", &cert_path).await?;
        copy_files(
            &container,
            "/root/.lnd/data/chain/bitcoin/regtest/admin.macaroon",
            &macaroon_path,
        )
        .await?;
        let port = container.get_host_port_ipv4(RPC_PORT).await?;
        let endpoint = format!("https://localhost:{port}");
        let client = tonic_lnd::connect(endpoint, &cert_path, &macaroon_path).await?;

        Ok(Self {
            container,
            client: Mutex::new(client),
        })
    }

    pub async fn get_id(&self) -> Result<String> {
        let mut client = self.client.lock().await;
        let info = client.lightning().get_info(GetInfoRequest {}).await?;
        Ok(info.into_inner().identity_pubkey)
    }

    pub async fn get_new_address(&self) -> Result<Address> {
        let mut client = self.client.lock().await;
        let address = client
            .lightning()
            .new_address(NewAddressRequest {
                account: "".to_string(),
                r#type: AddressType::UnusedWitnessPubkeyHash.into(),
            })
            .await?;
        Address::from_str(&address.into_inner().address)?
            .require_network(Network::Regtest)
            .map_err(anyhow::Error::msg)
    }

    pub async fn open_channel(
        &self,
        peer: PublicKey,
        address: String,
        funding_amount: Amount,
        push_amount: Amount,
    ) -> Result<()> {
        let mut client = self.client.lock().await;

        let addr = LightningAddress {
            pubkey: peer.to_string(),
            host: address,
        };

        match client
            .lightning()
            .connect_peer(ConnectPeerRequest {
                addr: Some(addr),
                perm: false,
                timeout: 0,
            })
            .await
        {
            Ok(_) => (),
            Err(status)
                if status
                    .message()
                    .to_lowercase()
                    .contains("already connected") => {}
            Err(status) => bail!(status),
        }

        client
            .lightning()
            .open_channel_sync(OpenChannelRequest {
                node_pubkey: peer.serialize().to_vec(),
                local_funding_amount: funding_amount.to_sat() as i64,
                push_sat: push_amount.to_sat() as i64,
                ..Default::default()
            })
            .await?;
        Ok(())
    }

    pub async fn list_active_channels(&self, peer: &PublicKey) -> Result<Vec<String>> {
        let mut client = self.client.lock().await;
        let request = ListChannelsRequest {
            active_only: true,
            peer: peer.serialize().to_vec(),
            ..Default::default()
        };
        let channels = client
            .lightning()
            .list_channels(request)
            .await?
            .into_inner()
            .channels
            .into_iter()
            .map(|c| c.channel_point)
            .collect();
        Ok(channels)
    }

    pub async fn pay(&self, payment_request: String) -> Result<()> {
        let mut client = self.client.lock().await;
        let resp = client
            .lightning()
            .send_payment_sync(SendRequest {
                payment_request,
                ..Default::default()
            })
            .await?;
        let res = resp.into_inner();
        if !res.payment_error.is_empty() {
            bail!(res.payment_error);
        }
        Ok(())
    }

    pub async fn receive(&self, amount: &Amount) -> Result<String> {
        let mut client = self.client.lock().await;
        let resp = client
            .lightning()
            .add_invoice(Invoice {
                value_msat: (amount.to_sat() * 1000) as i64,
                memo: "LND".to_string(),
                ..Default::default()
            })
            .await?;
        Ok(resp.into_inner().payment_request)
    }
}

async fn copy_files(
    container: &ContainerAsync<GenericImage>,
    source: &str,
    destination: &PathBuf,
) -> Result<()> {
    let content = container
        .exec(ExecCommand::new(["cat", source]))
        .await?
        .stdout_to_vec()
        .await?;
    tokio::fs::write(&destination, content).await?;
    Ok(())
}
