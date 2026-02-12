use std::path::Path;

use anyhow::{Error, Result};
use testcontainers::core::ExecCommand;
use testcontainers::{ContainerAsync, GenericImage};
use tonic::async_trait;

#[async_trait]
pub trait ContainerExt {
    async fn read_file(&self, source: &str) -> Result<Vec<u8>>;
    async fn copy_file(&self, source: &str, destination: &Path) -> Result<()>;
}

#[async_trait]
impl ContainerExt for ContainerAsync<GenericImage> {
    async fn read_file(&self, source: &str) -> Result<Vec<u8>> {
        self.exec(ExecCommand::new(["cat", source]))
            .await?
            .stdout_to_vec()
            .await
            .map_err(Error::msg)
    }

    async fn copy_file(&self, source: &str, destination: &Path) -> Result<()> {
        let file = self.read_file(source).await?;
        tokio::fs::write(&destination, file).await?;
        Ok(())
    }
}
