use anyhow::Result;
use futures::{Future, TryFutureExt};
use testcontainers::core::WaitFor;
use testcontainers::core::wait::HttpWaitStrategy;
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};
use testcontainers_modules::mariadb::Mariadb;
use tokio::try_join;

use crate::environment::log::TracingConsumer;
use crate::environment::{ApiCredentials, EnvironmentId};

const IMAGE_NAME: &str = "mempool/backend";
const IMAGE_TAG: &str = "v3.0.1";
const RPC_PORT: u16 = 8999;

pub struct Mempool {
    pub api: ApiCredentials,
    _container: ContainerAsync<GenericImage>,
    _mariadb: ContainerAsync<Mariadb>,
}

impl Mempool {
    pub async fn new(
        environment_id: &EnvironmentId,
        bitcoind_api: impl Future<Output = Result<&ApiCredentials>>,
        esplora_api: impl Future<Output = Result<&ApiCredentials>>,
    ) -> Result<Self> {
        tracing::info!("Starting mariadb initialization");
        let mariadb = Mariadb::default()
            .with_env_var("MARIADB_DATABASE", "mempool")
            .with_env_var("MARIADB_PASSWORD", "mempool")
            .with_env_var("MARIADB_USER", "mempool")
            .with_network(environment_id.network_name())
            .with_log_consumer(TracingConsumer::new("mempool-db"))
            .start()
            .map_err(anyhow::Error::msg);

        let (mariadb, bitcoind_api, esplora_api) = try_join!(mariadb, bitcoind_api, esplora_api)?;

        let mariadb_ip = mariadb.get_bridge_ip_address().await?.to_string();
        let container = GenericImage::new(IMAGE_NAME, IMAGE_TAG)
            .with_exposed_port(RPC_PORT.into())
            .with_wait_for(WaitFor::Http(Box::new(
                HttpWaitStrategy::new("/api/v1/blocks/tip/height")
                    .with_port(RPC_PORT.into())
                    .with_expected_status_code(200u16),
            )))
            .with_network(environment_id.network_name())
            .with_log_consumer(TracingConsumer::new("mempool"))
            .with_env_var("CORE_RPC_HOST", bitcoind_api.host.clone())
            .with_env_var("CORE_RPC_PASSWORD", bitcoind_api.password.clone())
            .with_env_var("CORE_RPC_PORT", bitcoind_api.port.to_string())
            .with_env_var("CORE_RPC_USERNAME", bitcoind_api.username.clone())
            .with_env_var("DATABASE_HOST", mariadb_ip)
            .with_env_var("ESPLORA_REST_API_URL", esplora_api.endpoint())
            .with_env_var("MEMPOOL_BACKEND", "esplora")
            .with_env_var("MEMPOOL_POOLS_JSON_URL", "http://localhost:9")
            .start()
            .await?;

        let mut api = ApiCredentials::from_container(&container, RPC_PORT).await?;
        api.path = "/api/v1".to_string();

        Ok(Self {
            api,
            _container: container,
            _mariadb: mariadb,
        })
    }
}
