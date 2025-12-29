use std::path::PathBuf;

use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand, ValueEnum};
use miden_client::address::Address;

use crate::{
    account, inspect, net, parse, rpc_tools, store_account, store_inspect, store_note, tx_inspect,
    word,
};
#[cfg(feature = "tui")]
use crate::store_tui;

#[derive(Debug, Parser)]
#[command(
    name = "distaff",
    about = "Lightweight helpers around miden-client",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Inspect a note or account file and print a short summary
    File {
        /// Path to a serialized `NoteFile` or `AccountFile`
        file_path: PathBuf,
        /// Validate the note against a node (fetch inclusion info, nullifier status)
        #[arg(long, default_value_t = false)]
        validate: bool,
        /// Network to use when validating
        #[arg(long, value_enum, default_value = "testnet")]
        network: Network,
        /// Custom endpoint (protocol://host[:port]) when --network custom
        #[arg(long)]
        endpoint: Option<String>,
    },
    /// RPC-related commands
    Rpc {
        #[command(subcommand)]
        command: RpcCommand,
    },
    /// Store-related commands
    Store {
        #[command(subcommand)]
        command: StoreCommand,
    },
    /// Build a word from one 0x-hex word or four felts (decimal or 0x-hex)
    Word {
        /// Provide one 0x-prefixed 32-byte word or four felts (decimal or 0x-hex)
        #[arg(num_args = 1..=4, value_name = "value")]
        values: Vec<String>,
    },
    /// Resolve an account ID from a bech32 address or 0x-hex account id
    AccountId {
        /// Account address (bech32) or account ID (0x-hex)
        account: String,
        /// Network to use when encoding an address from an account ID
        #[arg(long, value_enum)]
        network: Option<Network>,
    },
}

#[derive(Debug, Subcommand)]
pub enum RpcCommand {
    /// Query basic status information from the node
    Status {
        /// Network to query
        #[arg(long, value_enum, default_value = "testnet")]
        network: Network,
        /// Custom endpoint (protocol://host[:port]) when --network custom
        #[arg(long)]
        endpoint: Option<String>,
    },
    /// Inspect a note on the node by its note id
    Note {
        /// Note id (0x-hex)
        note_id: String,
        /// Network to query
        #[arg(long, value_enum, default_value = "testnet")]
        network: Network,
        /// Custom endpoint (protocol://host[:port]) when --network custom
        #[arg(long)]
        endpoint: Option<String>,
    },
    /// Fetch and display account details from a node
    Account {
        /// Account address (bech32) or account ID (0x-hex); network hints are derived when possible
        account: String,
        /// Print extended account details
        #[arg(long, default_value_t = false)]
        verbose: bool,
        /// Network to query
        #[arg(long, value_enum, default_value = "testnet")]
        network: Network,
        /// Custom endpoint (protocol://host[:port]) when --network custom
        #[arg(long)]
        endpoint: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum StoreCommand {
    /// Inspect a SQLite store and print summary statistics
    Inspect {
        /// Path to the sqlite3 store file
        #[arg(long, value_name = "path")]
        store: PathBuf,
    },
    /// Inspect account records in a local store
    Account {
        /// Path to the sqlite3 store file
        #[arg(long, value_name = "path")]
        store: PathBuf,
        /// Account address (bech32) or account ID (0x-hex)
        #[arg(long)]
        account: Option<String>,
        /// Account commitment (0x-hex)
        #[arg(long)]
        commitment: Option<String>,
        /// Account nonce (decimal)
        #[arg(long)]
        nonce: Option<u64>,
    },
    /// Inspect a note in a local store by its id
    Note {
        /// Note id (0x-hex)
        note_id: String,
        /// Path to the sqlite3 store file
        #[arg(long, value_name = "path")]
        store: PathBuf,
    },
    /// Transaction-related store commands
    Tx {
        #[command(subcommand)]
        command: StoreTxCommand,
    },
    /// Interactive store browser (requires --features tui)
    #[cfg(feature = "tui")]
    Tui {
        /// Path to the sqlite3 store file
        #[arg(long, value_name = "path")]
        store: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub enum StoreTxCommand {
    /// Inspect a transaction from a local store by its id
    Inspect {
        /// Transaction id (0x-hex)
        tx_id: String,
        /// Path to the sqlite3 store file
        #[arg(long, value_name = "path")]
        store: PathBuf,
        /// Print extended transaction details
        #[arg(long, default_value_t = false)]
        verbose: bool,
    },
    /// List transactions from a local store
    List {
        /// Path to the sqlite3 store file
        #[arg(long, value_name = "path")]
        store: PathBuf,
    },
}

#[derive(Debug, Clone, ValueEnum)]
pub enum Network {
    Testnet,
    Devnet,
    Local,
    Custom,
}

impl Cli {
    pub fn execute(self) -> Result<()> {
        match self.command {
            Command::File {
                file_path,
                validate,
                network,
                endpoint,
            } => {
                let endpoint = if validate {
                    Some(net::resolve_endpoint(network, endpoint)?)
                } else {
                    None
                };
                inspect::inspect(file_path, endpoint)
            }
            Command::Rpc { command } => match command {
                RpcCommand::Status { network, endpoint } => {
                    let endpoint = net::resolve_endpoint(network, endpoint)?;
                    rpc_tools::rpc_status(endpoint)
                }
                RpcCommand::Note {
                    note_id,
                    network,
                    endpoint,
                } => {
                    let note_id = parse::note_id(&note_id)?;
                    let endpoint = net::resolve_endpoint(network, endpoint)?;
                    inspect::inspect_note(note_id, endpoint)
                }
                RpcCommand::Account {
                    account,
                    verbose,
                    network,
                    endpoint,
                } => {
                    let (account_id, address_network_hint) = parse::account_id(&account)?;
                    let selected_network_id = net::network_id_for_cli_network(network.clone());
                    let endpoint = net::resolve_endpoint(network, endpoint)?;
                    account::inspect_account(
                        account_id,
                        address_network_hint,
                        selected_network_id,
                        verbose,
                        endpoint,
                    )
                }
            },
            Command::Store { command } => match command {
                StoreCommand::Inspect { store } => store_inspect::inspect_store(store),
                StoreCommand::Account {
                    store,
                    account,
                    commitment,
                    nonce,
                } => {
                    let query = match (account, commitment, nonce) {
                        (Some(account), None, None) => {
                            let (account_id, _) = parse::account_id(&account)?;
                            store_account::StoreAccountQuery::AccountId(account_id)
                        }
                        (None, Some(commitment), None) => {
                            store_account::StoreAccountQuery::Commitment(commitment)
                        }
                        (None, None, Some(nonce)) => store_account::StoreAccountQuery::Nonce(nonce),
                        _ => {
                            return Err(anyhow!(
                                "provide exactly one of --account, --commitment, or --nonce"
                            ));
                        }
                    };
                    store_account::inspect_store_account(store, query)
                }
                StoreCommand::Note { store, note_id } => {
                    let note_id = parse::note_id(&note_id)?;
                    store_note::inspect_store_note(store, note_id)
                }
                StoreCommand::Tx { command } => match command {
                    StoreTxCommand::Inspect {
                        tx_id,
                        store,
                        verbose,
                    } => {
                        let tx_id = parse::transaction_id(&tx_id)?;
                        tx_inspect::inspect_transaction(store, tx_id, verbose)
                    }
                    StoreTxCommand::List { store } => tx_inspect::list_transactions(store),
                },
                #[cfg(feature = "tui")]
                StoreCommand::Tui { store } => store_tui::run_store_tui(store),
            },
            Command::Word { values } => {
                let word = parse::word(&values)?;
                word::build_word(word)
            }
            Command::AccountId { account, network } => {
                let decoded_address = Address::decode(&account).ok();
                let (account_id, network_hint) = parse::account_id(&account)?;
                let selected_network_id = network.clone().and_then(net::network_id_for_cli_network);

                println!("Account ID: {}", account_id);
                println!("- account id (hex): {}", account_id.to_hex());
                println!("- account type: {:?}", account_id.account_type());
                println!("- storage mode: {}", account_id.storage_mode());
                println!(
                    "- public state: {}",
                    if account_id.has_public_state() {
                        "yes"
                    } else {
                        "no"
                    }
                );
                println!("- account ID version: {:?}", account_id.version());

                if let Some((address_network, address)) = decoded_address {
                    if let Some(expected) = selected_network_id.clone() {
                        if expected != address_network {
                            println!(
                                "- warning: address network {address_network} does not match selected {expected}"
                            );
                        }
                    }
                    println!("- address: {}", address.encode(address_network));
                } else if let Some(network_id) = selected_network_id {
                    let address = Address::new(account_id);
                    println!(
                        "- address ({network_id}): {}",
                        address.encode(network_id.clone())
                    );
                }

                if let Some(network_id) = network_hint {
                    println!("- address network: {}", network_id);
                }

                Ok(())
            }
        }
    }
}
