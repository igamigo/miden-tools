//! Configuration utilities using miden-client-cli's CliConfig.

use std::path::PathBuf;

use anyhow::{Context, Result};
use miden_client::rpc::Endpoint;
use miden_client_cli::config::CliConfig;

/// Cached configuration loaded from the system.
static CONFIG: std::sync::OnceLock<Option<CliConfig>> = std::sync::OnceLock::new();

/// Load the CLI config from the system (local .miden/ first, then global ~/.miden/).
/// Returns None if no config file is found.
fn get_config() -> Option<&'static CliConfig> {
    CONFIG
        .get_or_init(|| CliConfig::from_system().ok())
        .as_ref()
}

/// Get the store path from config, or None if not configured.
pub(crate) fn default_store_path() -> Option<PathBuf> {
    get_config().map(|c| c.store_filepath.clone())
}

/// Get the RPC endpoint from config, or None if not configured.
pub(crate) fn default_endpoint() -> Option<Endpoint> {
    get_config().map(|c| Endpoint::from(&c.rpc.endpoint))
}

/// Resolve store path: use provided path, or fall back to config default.
pub(crate) fn resolve_store_path(provided: Option<PathBuf>) -> Result<PathBuf> {
    provided.or_else(default_store_path).ok_or_else(|| {
        anyhow::anyhow!(
            "no store path provided and no config found\n  \
                hint: Use --store <path> or create a config at .miden/miden-client.toml"
        )
    })
}

/// Resolve endpoint: use provided endpoint/network, or fall back to config default.
pub(crate) fn resolve_endpoint_with_fallback(
    network: Option<crate::cli::Network>,
    custom_endpoint: Option<String>,
) -> Result<Endpoint> {
    use crate::cli::Network;
    use crate::util::parse;

    // If network is explicitly specified, use the standard resolution
    if let Some(network) = network {
        match network {
            Network::Testnet => return Ok(Endpoint::testnet()),
            Network::Devnet => return Ok(Endpoint::devnet()),
            Network::Local => return Ok(Endpoint::localhost()),
            Network::Custom => {
                let raw = custom_endpoint.ok_or_else(|| {
                    anyhow::anyhow!("--endpoint is required when using --network custom")
                })?;
                return parse::endpoint_parameter(raw.as_str()).context("invalid endpoint format");
            }
        }
    }

    // If custom endpoint is provided without network, use it
    if let Some(raw) = custom_endpoint {
        return parse::endpoint_parameter(raw.as_str()).context("invalid endpoint format");
    }

    // Fall back to config
    default_endpoint().ok_or_else(|| {
        anyhow::anyhow!(
            "no endpoint provided and no config found\n  \
            hint: Use --network <network> or create a config at .miden/miden-client.toml"
        )
    })
}
