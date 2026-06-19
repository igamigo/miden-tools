use anyhow::Result;
use miden_client::{
    account::AccountId,
    address::NetworkId,
    rpc::{Endpoint, GrpcClient, NodeRpcClient},
};
use tokio::runtime::Runtime;

use crate::render::account::render_account;
use crate::util::net::DEFAULT_TIMEOUT_MS;

/// Fetch and display account details for an on-chain account id or bech32 address.
pub(crate) fn inspect_account(
    account_id: AccountId,
    address_network_hint: Option<NetworkId>,
    selected_network_id: Option<NetworkId>,
    verbose: bool,
    endpoint: Endpoint,
) -> Result<()> {
    let rt = Runtime::new()?;

    rt.block_on(async move {
        let rpc = GrpcClient::new(&endpoint, DEFAULT_TIMEOUT_MS);
        match rpc.get_account_details(account_id).await {
            Ok(fetched) => {
                println!("Account {account_id}:");
                if let Some(address_network) = address_network_hint {
                    if let Some(expected) = selected_network_id {
                        if expected != address_network {
                            println!(
                                "- warning: address network {address_network} does not match selected {expected}"
                            );
                        } else {
                            println!("- address network: {address_network}");
                        }
                    } else {
                        println!("- address network: {address_network}");
                    }
                }

                // 0.15: `get_account_details` returns `Some(Account)` only for accounts with public
                // state; private accounts (or those without on-chain state) return `None`.
                match fetched {
                    Some(account) => {
                        println!("- commitment: {}", account.to_commitment());
                        render_account(&account, verbose);
                    },
                    None => {
                        if verbose {
                            println!("- type: private (state not available)");
                        } else {
                            println!("- header unavailable (private account)");
                        }
                    },
                }
            },
            Err(err) => {
                println!("Failed to fetch account {account_id}: {err}");
            },
        }
        Ok(())
    })
}
