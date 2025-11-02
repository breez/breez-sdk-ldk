use log::warn;
use serde::{Deserialize, Serialize};
use strum_macros::Display;

/// The different supported bitcoin networks
#[derive(Clone, Copy, Debug, Display, Eq, PartialEq, Serialize, Deserialize)]
pub enum Network {
    /// Mainnet
    Bitcoin,
    Testnet,
    Signet,
    Regtest,
}

impl From<bitcoin::Network> for Network {
    fn from(network: bitcoin::Network) -> Self {
        #[allow(unreachable_patterns)]
        match network {
            bitcoin::Network::Bitcoin => Network::Bitcoin,
            bitcoin::Network::Testnet | bitcoin::Network::Testnet4 => Network::Testnet,
            bitcoin::Network::Signet => Network::Signet,
            bitcoin::Network::Regtest => Network::Regtest,
            other => {
                warn!("Unknown network: {other:?}");
                Network::Bitcoin
            }
        }
    }
}

impl From<Network> for bitcoin::Network {
    fn from(network: Network) -> Self {
        match network {
            Network::Bitcoin => bitcoin::Network::Bitcoin,
            Network::Testnet => bitcoin::Network::Testnet,
            Network::Signet => bitcoin::Network::Signet,
            Network::Regtest => bitcoin::Network::Regtest,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoltzSwapperUrls {
    pub boltz_url: String,
    pub proxy_url: String,
}
