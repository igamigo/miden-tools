//! CLI command execution logic.

use anyhow::{Result, anyhow};
use miden_client::note::NoteTag;

use crate::{
    rpc, store,
    util::{config, net, parse},
};

use super::{
    Cli, Command, NoteTypeFilter, ParseCommand, RpcCommand, StoreAccountCommand, StoreCommand,
    StoreNoteCommand, StoreTagCommand, StoreTxCommand,
};

impl Cli {
    pub fn execute(self) -> Result<()> {
        match self.command {
            Command::Inspect {
                file_path,
                validate,
                network,
                endpoint,
            } => {
                let endpoint = if validate {
                    Some(config::resolve_endpoint_with_fallback(network, endpoint)?)
                } else {
                    None
                };
                rpc::inspect::inspect(file_path, endpoint)
            }
            Command::Rpc { command } => execute_rpc(command),
            Command::Store { command } => execute_store(command),
            Command::Parse { command } => execute_parse(command),
        }
    }
}

fn execute_rpc(command: RpcCommand) -> Result<()> {
    match command {
        RpcCommand::Status { network, endpoint } => {
            let endpoint = config::resolve_endpoint_with_fallback(network, endpoint)?;
            rpc::status::rpc_status(endpoint)
        }
        RpcCommand::Block {
            block_num,
            network,
            endpoint,
        } => {
            let block_num = parse::block_number(&block_num)?;
            let endpoint = config::resolve_endpoint_with_fallback(network, endpoint)?;
            rpc::status::rpc_block(endpoint, block_num)
        }
        RpcCommand::Note {
            note_id,
            save,
            network,
            endpoint,
        } => {
            let note_id = parse::note_id(&note_id)?;
            let endpoint = config::resolve_endpoint_with_fallback(network, endpoint)?;
            rpc::inspect::inspect_note(note_id, endpoint, save)
        }
        RpcCommand::Account {
            account,
            verbose,
            network,
            endpoint,
        } => {
            let (account_id, address_network_hint) = parse::account_id(&account)?;
            let selected_network_id = network.clone().and_then(net::network_id_for_cli_network);
            let endpoint = config::resolve_endpoint_with_fallback(network, endpoint)?;
            rpc::account::inspect_account(
                account_id,
                address_network_hint,
                selected_network_id,
                verbose,
                endpoint,
            )
        }
    }
}

fn execute_store(command: StoreCommand) -> Result<()> {
    match command {
        StoreCommand::Path => store::inspect::print_default_store_path(),
        StoreCommand::Info { store } => {
            let store = config::resolve_store_path(store)?;
            store::inspect::inspect_store(store)
        }
        StoreCommand::Account { command } => match command {
            StoreAccountCommand::Get {
                store,
                account,
                commitment,
                nonce,
            } => {
                let store = config::resolve_store_path(store)?;
                let query = match (account, commitment, nonce) {
                    (Some(account), None, None) => {
                        let (account_id, _) = parse::account_id(&account)?;
                        store::account::StoreAccountQuery::AccountId(account_id)
                    }
                    (None, Some(commitment), None) => {
                        store::account::StoreAccountQuery::Commitment(commitment)
                    }
                    (None, None, Some(nonce)) => store::account::StoreAccountQuery::Nonce(nonce),
                    _ => {
                        return Err(anyhow!(
                            "provide exactly one of --account, --commitment, or --nonce"
                        ));
                    }
                };
                store::account::inspect_store_account(store, query)
            }
            StoreAccountCommand::List { store } => {
                let store = config::resolve_store_path(store)?;
                store::account::list_store_accounts(store)
            }
        },
        StoreCommand::Note { command } => match command {
            StoreNoteCommand::Get { store, note_id } => {
                let store = config::resolve_store_path(store)?;
                let note_id = parse::note_id(&note_id)?;
                store::note::inspect_store_note(store, note_id)
            }
            StoreNoteCommand::List {
                store,
                input,
                output,
                state,
                tag,
                note_type,
            } => {
                let store = config::resolve_store_path(store)?;
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
                let note_type = note_type.map(NoteTypeFilter::into_note_type);
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
        StoreCommand::Tag { command } => match command {
            StoreTagCommand::List { store } => {
                let store = config::resolve_store_path(store)?;
                store::tags::list_store_tags(store)
            }
        },
        StoreCommand::Tx { command } => match command {
            StoreTxCommand::Inspect {
                tx_id,
                store,
                verbose,
            } => {
                let store = config::resolve_store_path(store)?;
                let tx_id = parse::transaction_id(&tx_id)?;
                rpc::tx::inspect_transaction(store, tx_id, verbose)
            }
            StoreTxCommand::List { store } => {
                let store = config::resolve_store_path(store)?;
                rpc::tx::list_transactions(store)
            }
        },
        StoreCommand::Tui { store } => {
            let store = config::resolve_store_path(store)?;
            store::tui::run_store_tui(store)
        }
    }
}

fn execute_parse(command: ParseCommand) -> Result<()> {
    use miden_client::address::{Address, AddressId};

    match command {
        ParseCommand::Word { values } => {
            let word = parse::word(&values)?;
            rpc::word::build_word(word)
        }
        ParseCommand::AccountId { account, network } => {
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
                if let Some(expected) = selected_network_id.clone()
                    && expected != address_network
                {
                    println!(
                        "- warning: address network {address_network} does not match selected {expected}"
                    );
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
            println!("- binary: {:032b}", raw);

            // Check if this looks like an account target tag
            // Account target tags use the N most significant bits of an account ID prefix
            // Default is 14 bits, meaning the lower 18 bits should be zero
            let trailing_zeros = raw.trailing_zeros();
            if trailing_zeros >= 18 {
                let significant_bits = 32 - trailing_zeros;
                println!(
                    "- likely account target: yes ({} high bits set, {} low bits zero)",
                    significant_bits, trailing_zeros
                );
                // The high bits would match account IDs with this prefix
                let prefix_bits = raw >> (32 - 16); // Show as 16-bit prefix
                println!(
                    "- matches account prefixes starting with: 0x{:04x}...",
                    prefix_bits
                );
            } else if trailing_zeros >= 16 {
                println!(
                    "- possible account target: yes ({} low bits zero, default is 18)",
                    trailing_zeros
                );
            } else {
                println!("- likely account target: no (use case tag or custom structure)");
            }

            Ok(())
        }
        ParseCommand::Address { address, network } => {
            let selected_network_id = network.clone().and_then(net::network_id_for_cli_network);
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

                if let Some(expected) = selected_network_id
                    && expected != network_id
                {
                    println!(
                        "- warning: address network {} does not match selected {}",
                        network_id, expected
                    );
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
    }
}
