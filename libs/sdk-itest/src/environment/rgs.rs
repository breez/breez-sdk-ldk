use std::time::Duration;

use anyhow::Result;
use futures::TryFutureExt;
use testcontainers::core::wait::LogWaitStrategy;
use testcontainers::core::{AccessMode, Mount, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};
use testcontainers_modules::postgres::Postgres;
use tokio::try_join;

use crate::environment::log::LogConsumer;
use crate::environment::{ApiCredentials, EnvironmentId, Lnd};

const IMAGE_NAME: &str = "rgs";
const IMAGE_TAG: &str = "latest";
const RPC_PORT: u16 = 8011;

pub struct Rgs {
    pub api: ApiCredentials,
    _container: ContainerAsync<GenericImage>,
    _nginx: ContainerAsync<GenericImage>,
    _postgres: ContainerAsync<Postgres>,
}

impl Rgs {
    pub async fn new(
        environment_id: &EnvironmentId,
        bitcoind_rest_api: impl Future<Output = Result<&ApiCredentials>>,
        lsp_address: impl Future<Output = Result<String>>,
        lnd: impl Future<Output = Result<&Lnd>>,
    ) -> Result<Self> {
        let postgres = Postgres::default()
            .with_tag("16")
            .with_network(environment_id.network_name())
            .with_log_consumer(LogConsumer::new("rgs-postgres"))
            .start()
            .map_err(anyhow::Error::msg);
        let (postgres, bitcoind_rest_api, lsp_address, _lnd) =
            try_join!(postgres, bitcoind_rest_api, lsp_address, lnd)?;

        let postgres_host = postgres.get_bridge_ip_address().await?.to_string();
        let volume_name = format!("rgs-data-{environment_id}");

        let rgs_data = Mount::volume_mount(&volume_name, "/data/.rgs");
        let container = GenericImage::new(IMAGE_NAME, IMAGE_TAG)
            .with_wait_for(WaitFor::Log(LogWaitStrategy::stdout(
                "Sleeping until next snapshot capture",
            )))
            .with_network(environment_id.network_name())
            .with_log_consumer(LogConsumer::new("rgs-server"))
            .with_mount(rgs_data)
            .with_env_var("BITCOIN_REST_DOMAIN", &bitcoind_rest_api.host)
            .with_env_var("BITCOIN_REST_PATH", &bitcoind_rest_api.path)
            .with_env_var("BITCOIN_REST_PORT", bitcoind_rest_api.port.to_string())
            .with_env_var("LN_PEERS", lsp_address)
            .with_env_var("RAPID_GOSSIP_SYNC_SERVER_DB_HOST", postgres_host)
            .with_env_var("RAPID_GOSSIP_SYNC_SERVER_DB_NAME", "postgres")
            .with_env_var("RAPID_GOSSIP_SYNC_SERVER_DB_PASSWORD", "postgres")
            .with_env_var("RAPID_GOSSIP_SYNC_SERVER_DB_USER", "postgres")
            .with_env_var("RAPID_GOSSIP_SYNC_SERVER_NETWORK", "regtest")
            .with_startup_timeout(Duration::from_secs(180)) // Wating for the second channel update takes time...
            .start()
            .await?;

        let rgs_data =
            Mount::volume_mount(volume_name, "/data").with_access_mode(AccessMode::ReadOnly);
        let nginx_config = include_bytes!("../../docker/rgs-nginx.conf");
        let nginx = GenericImage::new("nginx", "latest")
            .with_exposed_port(RPC_PORT.into())
            .with_network(environment_id.network_name())
            .with_log_consumer(LogConsumer::new("rgs-nginx"))
            .with_copy_to("/etc/nginx/conf.d/default.conf", nginx_config.to_vec())
            .with_mount(rgs_data)
            .start()
            .await?;

        let mut api = ApiCredentials::from_container(&nginx, RPC_PORT).await?;
        api.path = "/v2".to_string();

        Ok(Self {
            api,
            _container: container,
            _nginx: nginx,
            _postgres: postgres,
        })
    }
}
