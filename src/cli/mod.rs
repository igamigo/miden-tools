//! CLI definitions using clap.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use miden_client::note::NoteType;

mod execute;

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
        /// Network to use when validating (falls back to config)
        #[arg(long, value_enum)]
        network: Option<Network>,
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
        /// Network to query (falls back to config)
        #[arg(long, value_enum)]
        network: Option<Network>,
        /// Custom endpoint (protocol://host[:port]) when --network custom
        #[arg(long)]
        endpoint: Option<String>,
    },
    /// Fetch a block header by number
    Block {
        /// Block number (decimal or 0x-hex)
        block_num: String,
        /// Network to query (falls back to config)
        #[arg(long, value_enum)]
        network: Option<Network>,
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
        /// Network to query (falls back to config)
        #[arg(long, value_enum)]
        network: Option<Network>,
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
        /// Network to query (falls back to config)
        #[arg(long, value_enum)]
        network: Option<Network>,
        /// Custom endpoint (protocol://host[:port]) when --network custom
        #[arg(long)]
        endpoint: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum StoreCommand {
    /// Print the default store path for this platform
    Path,
    /// Print store summary and statistics
    Info {
        /// Path to the sqlite3 store file (falls back to config)
        #[arg(long, value_name = "path")]
        store: Option<PathBuf>,
    },
    /// Account-related store commands
    Account {
        #[command(subcommand)]
        command: StoreAccountCommand,
    },
    /// Note-related store commands
    Note {
        #[command(subcommand)]
        command: StoreNoteCommand,
    },
    /// Tag-related store commands
    Tag {
        #[command(subcommand)]
        command: StoreTagCommand,
    },
    /// Transaction-related store commands
    Tx {
        #[command(subcommand)]
        command: StoreTxCommand,
    },
    /// Interactive store browser
    Tui {
        /// Path to the sqlite3 store file (falls back to config)
        #[arg(long, value_name = "path")]
        store: Option<PathBuf>,
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
pub enum StoreAccountCommand {
    /// Get account details by ID, commitment, or nonce
    Get {
        /// Path to the sqlite3 store file (falls back to config)
        #[arg(long, value_name = "path")]
        store: Option<PathBuf>,
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
    /// List tracked accounts from a local store
    List {
        /// Path to the sqlite3 store file (falls back to config)
        #[arg(long, value_name = "path")]
        store: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
pub enum StoreNoteCommand {
    /// Get note details by ID
    Get {
        /// Note id (0x-hex)
        note_id: String,
        /// Path to the sqlite3 store file (falls back to config)
        #[arg(long, value_name = "path")]
        store: Option<PathBuf>,
    },
    /// List notes from a local store
    List {
        /// Path to the sqlite3 store file (falls back to config)
        #[arg(long, value_name = "path")]
        store: Option<PathBuf>,
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
pub enum StoreTagCommand {
    /// List tracked note tags from a local store
    List {
        /// Path to the sqlite3 store file (falls back to config)
        #[arg(long, value_name = "path")]
        store: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
pub enum StoreTxCommand {
    /// Inspect a transaction from a local store by its id
    Inspect {
        /// Transaction id (0x-hex)
        tx_id: String,
        /// Path to the sqlite3 store file (falls back to config)
        #[arg(long, value_name = "path")]
        store: Option<PathBuf>,
        /// Print extended transaction details
        #[arg(long, default_value_t = false)]
        verbose: bool,
    },
    /// List transactions from a local store
    List {
        /// Path to the sqlite3 store file (falls back to config)
        #[arg(long, value_name = "path")]
        store: Option<PathBuf>,
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
    pub(crate) fn into_note_type(self) -> NoteType {
        match self {
            NoteTypeFilter::Public => NoteType::Public,
            NoteTypeFilter::Private => NoteType::Private,
            NoteTypeFilter::Encrypted => NoteType::Encrypted,
        }
    }
}
