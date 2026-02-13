use anyhow::{Error, Result};
use futures::TryFutureExt;
use testcontainers::core::WaitFor;
use testcontainers::core::wait::LogWaitStrategy;
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};
use testcontainers_modules::postgres::Postgres;
use tokio::try_join;

use crate::environment::log::LogConsumer;
use crate::environment::{ApiCredentials, EnvironmentId};

const IMAGE_NAME: &str = "swapd";
const IMAGE_TAG: &str = "latest";
const RPC_PORT: u16 = 8011;

pub struct Swapd {
    pub api: ApiCredentials,
    _container: ContainerAsync<GenericImage>,
    _postgres: ContainerAsync<Postgres>,
}

impl Swapd {
    pub async fn new(
        environment_id: &EnvironmentId,
        bitcoind_api: impl Future<Output = Result<&ApiCredentials>>,
        lnd_grpc_api: impl Future<Output = Result<&ApiCredentials>>,
    ) -> Result<Self> {
        let postgres = Postgres::default()
            .with_tag("16")
            .with_network(environment_id.network_name())
            .with_log_consumer(LogConsumer::new("swapd-postgres"))
            .start()
            .map_err(Error::msg);
        let (postgres, bitcoind_api, lnd_grpc_api) =
            try_join!(postgres, bitcoind_api, lnd_grpc_api)?;
        let postgres_host = postgres.get_bridge_ip_address().await?.to_string();

        let container = GenericImage::new(IMAGE_NAME, IMAGE_TAG)
            .with_exposed_port(RPC_PORT.into())
            .with_wait_for(WaitFor::Log(LogWaitStrategy::stdout("swapd started")))
            .with_network(environment_id.network_name())
            .with_log_consumer(LogConsumer::new("swapd"))
            .with_copy_to(
                "/data/lnd/admin.macaroon",
                hex::encode(&lnd_grpc_api.macaroon).into_bytes(),
            )
            .with_env_var("NO_COLOR", "1")
            .with_cmd([
				"--auto-migrate",
				"--chain-poll-interval-seconds=5",
				format!("--lnd-grpc-address={}", lnd_grpc_api.endpoint()).as_str(),
				"--lnd-grpc-macaroon=/data/lnd/admin.macaroon",
				"--log-level=swapd=debug,info",
				"--network=regtest",
				format!("--address=0.0.0.0:{RPC_PORT}").as_str(),
				format!("--db-url=postgresql://postgres:postgres@{postgres_host}/postgres?sslmode=disable").as_str(),
				format!("--bitcoind-rpc-address={}", bitcoind_api.endpoint()).as_str(),
				format!("--bitcoind-rpc-user={}", bitcoind_api.username).as_str(),
				format!("--bitcoind-rpc-password={}", bitcoind_api.password).as_str(),
			])
            .start()
            .await?;

        let api = ApiCredentials::from_container(&container, RPC_PORT).await?;

        Ok(Self {
            api,
            _container: container,
            _postgres: postgres,
        })
    }
}
