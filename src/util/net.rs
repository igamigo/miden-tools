use anyhow::{Result, anyhow};
use miden_client::{address::NetworkId, rpc::Endpoint};

use crate::{cli::Network, util::parse};

pub(crate) const DEFAULT_TIMEOUT_MS: u64 = 10_000;

/// Resolve an RPC endpoint based on the CLI network selection and optional override.
pub(crate) fn resolve_endpoint(
    network: Network,
    custom_endpoint: Option<String>,
) -> Result<Endpoint> {
    match network {
        Network::Testnet => Ok(Endpoint::testnet()),
        Network::Devnet => Ok(Endpoint::devnet()),
        Network::Local => Ok(Endpoint::localhost()),
        Network::Custom => {
            let raw = custom_endpoint
                .ok_or_else(|| anyhow!("--endpoint is required for custom network"))?;

            parse::endpoint_parameter(raw.as_str())
        }
    }
}

pub(crate) fn network_id_for_cli_network(network: Network) -> Option<NetworkId> {
    match network {
        Network::Testnet => Some(NetworkId::Testnet),
        Network::Devnet => Some(NetworkId::Devnet),
        Network::Local | Network::Custom => None,
    }
}
