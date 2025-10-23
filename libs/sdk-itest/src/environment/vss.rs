use anyhow::Result;
use testcontainers::core::WaitFor;
use testcontainers::core::wait::HttpWaitStrategy;
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};
use testcontainers_modules::postgres::Postgres;

use crate::environment::log::TracingConsumer;
use crate::environment::{ApiCredentials, EnvironmentId};

const IMAGE_NAME: &str = "vss";
const IMAGE_TAG: &str = "latest";
const RPC_PORT: u16 = 3080;
const CONFIG_PATH: &str = "/etc/vss-server.toml";

pub struct Vss {
    pub api: ApiCredentials,
    _container: ContainerAsync<GenericImage>,
    _postgres: ContainerAsync<Postgres>,
}

impl Vss {
    pub async fn new(environment_id: &EnvironmentId) -> Result<Self> {
        let init_sql = include_bytes!("../../docker/v0_create_vss_db.sql");
        let postgres = Postgres::default()
            .with_init_sql(init_sql.to_vec())
            .with_tag("16")
            .with_network(environment_id.network_name())
            .with_log_consumer(TracingConsumer::new("vss-postgres"))
            .start()
            .await?;
        let postgres_host = postgres.get_bridge_ip_address().await?.to_string();
        let config = include_str!("../../docker/vss-server.toml.template")
            .replace("{postgres_host}", &postgres_host);

        let container = GenericImage::new(IMAGE_NAME, IMAGE_TAG)
            .with_exposed_port(RPC_PORT.into())
            .with_wait_for(WaitFor::Http(Box::new(
                HttpWaitStrategy::new("/vss")
                    .with_port(RPC_PORT.into())
                    .with_expected_status_code(400u16),
            )))
            .with_network(environment_id.network_name())
            .with_log_consumer(TracingConsumer::new("vss-server"))
            .with_copy_to(CONFIG_PATH, config.as_bytes().to_vec())
            .with_cmd([CONFIG_PATH])
            .start()
            .await?;

        let mut api = ApiCredentials::from_container(&container, RPC_PORT).await?;
        api.path = "/vss".to_string();

        Ok(Self {
            api,
            _container: container,
            _postgres: postgres,
        })
    }
}
