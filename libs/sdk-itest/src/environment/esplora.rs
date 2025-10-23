use anyhow::Result;
use testcontainers::core::WaitFor;
use testcontainers::core::wait::HttpWaitStrategy;
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};

use crate::environment::log::TracingConsumer;
use crate::environment::{ApiCredentials, EnvironmentId};

const IMAGE_NAME: &str = "ghcr.io/vulpemventures/electrs";
const IMAGE_TAG: &str = "a808b51d0d9301fa82390b985c57551966001f9b";
const RPC_PORT: u16 = 30000;

pub struct Esplora {
    pub api: ApiCredentials,
    _container: ContainerAsync<GenericImage>,
}

impl Esplora {
    pub async fn new(
        environment_id: &EnvironmentId,
        bitcoind_api: &ApiCredentials,
    ) -> Result<Self> {
        let container = GenericImage::new(IMAGE_NAME, IMAGE_TAG)
            .with_exposed_port(RPC_PORT.into())
            .with_wait_for(WaitFor::Http(Box::new(
                HttpWaitStrategy::new("/blocks/tip/hash")
                    .with_port(RPC_PORT.into())
                    .with_expected_status_code(200u16),
            )))
            .with_network(environment_id.network_name())
            .with_log_consumer(TracingConsumer::new("esplora"))
            .with_cmd([
                "-vvvv",
                "--network=regtest",
                "--daemon-dir=/config",
                "--jsonrpc-import",
                format!("--daemon-rpc-addr={}", bitcoind_api.address()).as_str(),
                format!(
                    "--cookie={}:{}",
                    bitcoind_api.username, bitcoind_api.password
                )
                .as_str(),
                format!("--http-addr=0.0.0.0:{RPC_PORT}").as_str(),
            ])
            .start()
            .await?;

        let api = ApiCredentials::from_container(&container, RPC_PORT).await?;
        Ok(Self {
            api,
            _container: container,
        })
    }
}
