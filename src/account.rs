use anyhow::Result;
use miden_client::{
    account::AccountId,
    address::NetworkId,
    rpc::{Endpoint, GrpcClient, NodeRpcClient},
};
use tokio::runtime::Runtime;

use crate::net::DEFAULT_TIMEOUT_MS;
use crate::render::account::{render_account_header, render_public_account};

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
                println!("- commitment: {}", fetched.commitment());

                match fetched {
                    miden_client::rpc::domain::account::FetchedAccount::Private(_, summary) => {
                        println!("- latest block: {}", summary.last_block_num);
                        if verbose {
                            println!("- type: private (state not available)");
                        } else {
                            println!("- header unavailable (private account)");
                        }
                    },
                    miden_client::rpc::domain::account::FetchedAccount::Public(account, summary) => {
                        println!("- latest block: {}", summary.last_block_num);
                        if verbose {
                            render_public_account(account.as_ref());
                        } else {
                            render_account_header(account.as_ref());
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
