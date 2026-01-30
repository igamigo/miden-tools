use std::path::PathBuf;

use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand, ValueEnum};
use miden_client::address::{Address, AddressId};
use miden_client::note::{NoteTag, NoteType};

#[cfg(feature = "tui")]
use crate::store::tui as store_tui;
use crate::{
    commands::{account, inspect, rpc, tx, word},
    store,
    util::{net, parse},
};

#[derive(Debug, Parser)]
#[command(
    name = "distaff",
    about = "Lightweight helpers around miden-client",
    version = concat!(env!("CARGO_PKG_VERSION"), " (", env!("GIT_HASH"), ")")
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Inspect a note or account file and print a short summary
    Inspect {
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
    /// Parsing helpers for common formats
    Parse {
        #[command(subcommand)]
        command: ParseCommand,
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
    /// Fetch a block header by number
    Block {
        /// Block number (decimal or 0x-hex)
        block_num: String,
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
        /// Save fetched note as a NoteFile
        #[arg(long, value_name = "path")]
        save: Option<PathBuf>,
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
    /// Print the default store path for this platform
    Path,
    /// Print a condensed summary of store statistics
    Stats {
        /// Path to the sqlite3 store file
        #[arg(long, value_name = "path")]
        store: PathBuf,
    },
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
    /// Account-related store commands
    Accounts {
        #[command(subcommand)]
        command: StoreAccountsCommand,
    },
    /// Inspect a note in a local store by its id
    Note {
        /// Note id (0x-hex)
        note_id: String,
        /// Path to the sqlite3 store file
        #[arg(long, value_name = "path")]
        store: PathBuf,
    },
    /// Note-related store commands
    Notes {
        #[command(subcommand)]
        command: StoreNotesCommand,
    },
    /// Tag-related store commands
    Tags {
        #[command(subcommand)]
        command: StoreTagsCommand,
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
pub enum ParseCommand {
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
    /// Parse a note tag (decimal or 0x-hex)
    NoteTag {
        /// Note tag value (decimal or 0x-hex)
        tag: String,
    },
    /// Decode or encode a bech32 address
    Address {
        /// Bech32 address or account ID (0x-hex)
        address: String,
        /// Network to use when encoding an address from an account ID
        #[arg(long, value_enum)]
        network: Option<Network>,
    },
}

#[derive(Debug, Subcommand)]
pub enum StoreAccountsCommand {
    /// List tracked accounts from a local store
    List {
        /// Path to the sqlite3 store file
        #[arg(long, value_name = "path")]
        store: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub enum StoreNotesCommand {
    /// List notes from a local store
    List {
        /// Path to the sqlite3 store file
        #[arg(long, value_name = "path")]
        store: PathBuf,
        /// Include input notes
        #[arg(long, default_value_t = false)]
        input: bool,
        /// Include output notes
        #[arg(long, default_value_t = false)]
        output: bool,
        /// Filter by note state (comma-separated or repeated)
        #[arg(long, value_name = "state", num_args = 1.., value_delimiter = ',')]
        state: Vec<String>,
        /// Filter by note tag (decimal or 0x-hex)
        #[arg(long, value_name = "tag")]
        tag: Option<String>,
        /// Filter by note type
        #[arg(long, value_enum)]
        note_type: Option<NoteTypeFilter>,
    },
}

#[derive(Debug, Subcommand)]
pub enum StoreTagsCommand {
    /// List tracked note tags from a local store
    List {
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

#[derive(Debug, Clone, ValueEnum)]
pub enum NoteTypeFilter {
    Public,
    Private,
    Encrypted,
}

impl NoteTypeFilter {
    fn to_note_type(self) -> NoteType {
        match self {
            NoteTypeFilter::Public => NoteType::Public,
            NoteTypeFilter::Private => NoteType::Private,
            NoteTypeFilter::Encrypted => NoteType::Encrypted,
        }
    }
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
                    rpc::rpc_status(endpoint)
                }
                RpcCommand::Block {
                    block_num,
                    network,
                    endpoint,
                } => {
                    let block_num = parse::block_number(&block_num)?;
                    let endpoint = net::resolve_endpoint(network, endpoint)?;
                    rpc::rpc_block(endpoint, block_num)
                }
                RpcCommand::Note {
                    note_id,
                    save,
                    network,
                    endpoint,
                } => {
                    let note_id = parse::note_id(&note_id)?;
                    let endpoint = net::resolve_endpoint(network, endpoint)?;
                    inspect::inspect_note(note_id, endpoint, save)
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
                StoreCommand::Path => store::inspect::print_default_store_path(),
                StoreCommand::Stats { store } => store::inspect::print_store_stats(store),
                StoreCommand::Inspect { store } => store::inspect::inspect_store(store),
                StoreCommand::Account {
                    store,
                    account,
                    commitment,
                    nonce,
                } => {
                    let query = match (account, commitment, nonce) {
                        (Some(account), None, None) => {
                            let (account_id, _) = parse::account_id(&account)?;
                            store::account::StoreAccountQuery::AccountId(account_id)
                        }
                        (None, Some(commitment), None) => {
                            store::account::StoreAccountQuery::Commitment(commitment)
                        }
                        (None, None, Some(nonce)) => {
                            store::account::StoreAccountQuery::Nonce(nonce)
                        }
                        _ => {
                            return Err(anyhow!(
                                "provide exactly one of --account, --commitment, or --nonce"
                            ));
                        }
                    };
                    store::account::inspect_store_account(store, query)
                }
                StoreCommand::Accounts { command } => match command {
                    StoreAccountsCommand::List { store } => {
                        store::account::list_store_accounts(store)
                    }
                },
                StoreCommand::Note { store, note_id } => {
                    let note_id = parse::note_id(&note_id)?;
                    store::note::inspect_store_note(store, note_id)
                }
                StoreCommand::Notes { command } => match command {
                    StoreNotesCommand::List {
                        store,
                        input,
                        output,
                        state,
                        tag,
                        note_type,
                    } => {
                        let tag = match tag {
                            Some(tag) => {
                                let value = parse::u64(&tag)?;
                                let raw: u32 = value
                                    .try_into()
                                    .map_err(|_| anyhow!("note tag must fit in u32"))?;
                                Some(NoteTag::from(raw))
                            }
                            None => None,
                        };
                        let note_type = note_type.map(NoteTypeFilter::to_note_type);
                        let filters = store::note::NoteListFilters {
                            include_input: input,
                            include_output: output,
                            states: state,
                            tag,
                            note_type,
                        };
                        store::note::list_store_notes(store, filters)
                    }
                },
                StoreCommand::Tags { command } => match command {
                    StoreTagsCommand::List { store } => store::tags::list_store_tags(store),
                },
                StoreCommand::Tx { command } => match command {
                    StoreTxCommand::Inspect {
                        tx_id,
                        store,
                        verbose,
                    } => {
                        let tx_id = parse::transaction_id(&tx_id)?;
                        tx::inspect_transaction(store, tx_id, verbose)
                    }
                    StoreTxCommand::List { store } => tx::list_transactions(store),
                },
                #[cfg(feature = "tui")]
                StoreCommand::Tui { store } => store_tui::run_store_tui(store),
            },
            Command::Parse { command } => match command {
                ParseCommand::Word { values } => {
                    let word = parse::word(&values)?;
                    word::build_word(word)
                }
                ParseCommand::AccountId { account, network } => {
                    let decoded_address = Address::decode(&account).ok();
                    let (account_id, network_hint) = parse::account_id(&account)?;
                    let selected_network_id =
                        network.clone().and_then(net::network_id_for_cli_network);

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
                ParseCommand::NoteTag { tag } => {
                    let value = parse::u64(&tag)?;
                    let raw: u32 = value
                        .try_into()
                        .map_err(|_| anyhow!("note tag must fit in u32"))?;
                    let tag = NoteTag::from(raw);
                    println!("Note tag: {}", tag);
                    println!("- raw (hex): 0x{raw:08x}");
                    println!("- decoded: {}", crate::render::note::format_note_tag(tag));
                    Ok(())
                }
                ParseCommand::Address { address, network } => {
                    let selected_network_id =
                        network.clone().and_then(net::network_id_for_cli_network);
                    if let Ok((network_id, decoded_address)) = Address::decode(&address) {
                        let account_id = match decoded_address.id() {
                            AddressId::AccountId(id) => id,
                            _ => return Err(anyhow!("unsupported address type")),
                        };

                        println!("Address: {}", address);
                        println!("- network: {}", network_id);
                        println!("- account id: {}", account_id);
                        println!("- account type: {:?}", account_id.account_type());
                        println!("- storage mode: {}", account_id.storage_mode());
                        println!("- note tag length: {}", decoded_address.note_tag_len());
                        println!(
                            "- note tag: {}",
                            crate::render::note::format_note_tag(decoded_address.to_note_tag())
                        );
                        if let Some(interface) = decoded_address.interface() {
                            println!("- interface: {}", interface);
                        }
                        println!("- bech32: {}", decoded_address.encode(network_id.clone()));

                        if let Some(expected) = selected_network_id {
                            if expected != network_id {
                                println!(
                                    "- warning: address network {} does not match selected {}",
                                    network_id, expected
                                );
                            }
                        }

                        Ok(())
                    } else {
                        let (account_id, network_hint) = parse::account_id(&address)?;
                        let addr = Address::new(account_id);

                        if let Some(network_id) = selected_network_id {
                            let encoded = addr.encode(network_id.clone());
                            println!("Address: {}", encoded);
                            println!("- network: {}", network_id);
                        } else {
                            println!("Address (from account id):");
                            println!("- bech32: n/a (provide --network testnet|devnet)");
                        }
                        println!("- account id: {}", account_id);
                        println!("- account type: {:?}", account_id.account_type());
                        println!("- storage mode: {}", account_id.storage_mode());
                        println!("- note tag length: {}", addr.note_tag_len());
                        println!(
                            "- note tag: {}",
                            crate::render::note::format_note_tag(addr.to_note_tag())
                        );

                        if let Some(network_id) = network_hint {
                            println!("- address network: {}", network_id);
                        }

                        Ok(())
                    }
                }
            },
        }
    }
}
