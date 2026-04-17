use miden_client::address::NetworkId;

use crate::cli::Network;

pub(crate) const DEFAULT_TIMEOUT_MS: u64 = 10_000;

pub(crate) fn network_id_for_cli_network(network: Network) -> Option<NetworkId> {
    match network {
        Network::Testnet => Some(NetworkId::Testnet),
        Network::Devnet => Some(NetworkId::Devnet),
        Network::Local | Network::Custom => None,
    }
}
