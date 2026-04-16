use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use miden_protocol::{
    account::StorageSlotType,
    transaction::{InputNote, TransactionInputs},
    utils::serde::{Deserializable, Serializable},
};

const PROOF_SIZE_LIMIT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone)]
struct SizeItem {
    label: String,
    bytes: usize,
}

pub(crate) fn inspect(file_path: PathBuf, top: usize) -> Result<()> {
    let bytes =
        fs::read(&file_path).with_context(|| format!("failed to read {}", file_path.display()))?;
    let tx_inputs = TransactionInputs::read_from_bytes(&bytes)
        .with_context(|| format!("failed to deserialize {}", file_path.display()))?;

    println!("Inspecting {} as TransactionInputs", file_path.display());
    render_tx_inputs_stats(&tx_inputs, bytes.len(), top.max(1));

    Ok(())
}

fn render_tx_inputs_stats(tx_inputs: &TransactionInputs, file_size: usize, top: usize) {
    let serialized_size = serialized_len(tx_inputs);
    let limit_delta = serialized_size as i64 - PROOF_SIZE_LIMIT_BYTES as i64;

    println!(
        "- total serialized size: {}",
        bytes_with_unit(serialized_size)
    );
    println!(
        "- proof size limit: {}",
        bytes_with_unit(PROOF_SIZE_LIMIT_BYTES)
    );
    if limit_delta > 0 {
        println!(
            "- limit status: exceeds by {}",
            bytes_with_unit(limit_delta as usize)
        );
    } else {
        println!(
            "- limit status: under by {}",
            bytes_with_unit((-limit_delta) as usize)
        );
    }
    println!("- file size on disk: {}", bytes_with_unit(file_size));
    if file_size != serialized_size {
        println!(
            "- warning: file size differs from serialized size by {}",
            bytes_with_unit(file_size.abs_diff(serialized_size))
        );
    }

    let account_size = serialized_len(tx_inputs.account());
    let block_header_size = serialized_len(tx_inputs.block_header());
    let blockchain_size = serialized_len(tx_inputs.blockchain());
    let input_notes_size = serialized_len(tx_inputs.input_notes());
    let tx_args_size = serialized_len(tx_inputs.tx_args());
    let advice_inputs_size = serialized_len(tx_inputs.advice_inputs());
    let foreign_account_code_size = serialized_len(&tx_inputs.foreign_account_code().to_vec());
    let foreign_slot_names_size = serialized_len(tx_inputs.foreign_account_slot_names());

    let mut top_level = vec![
        SizeItem {
            label: "advice_inputs".into(),
            bytes: advice_inputs_size,
        },
        SizeItem {
            label: "tx_args".into(),
            bytes: tx_args_size,
        },
        SizeItem {
            label: "input_notes".into(),
            bytes: input_notes_size,
        },
        SizeItem {
            label: "account".into(),
            bytes: account_size,
        },
        SizeItem {
            label: "blockchain (MMR + tracked headers)".into(),
            bytes: blockchain_size,
        },
        SizeItem {
            label: "block_header".into(),
            bytes: block_header_size,
        },
        SizeItem {
            label: "foreign_account_code".into(),
            bytes: foreign_account_code_size,
        },
        SizeItem {
            label: "foreign_account_slot_names".into(),
            bytes: foreign_slot_names_size,
        },
    ];
    render_ranked_sizes(
        "Top-level size breakdown",
        &mut top_level,
        serialized_size,
        top,
    );

    render_account_stats(tx_inputs, account_size);
    render_blockchain_stats(tx_inputs, block_header_size, blockchain_size);
    render_input_notes_stats(tx_inputs, input_notes_size, top);
    render_tx_args_stats(tx_inputs, tx_args_size);
    render_advice_stats(tx_inputs, advice_inputs_size, top);
    render_foreign_data_stats(tx_inputs, foreign_account_code_size, top);
}

fn render_account_stats(tx_inputs: &TransactionInputs, account_size: usize) {
    let account = tx_inputs.account();
    let storage_header = account.storage().header();

    let mut map_slots = 0usize;
    let mut value_slots = 0usize;
    for slot in storage_header.slots() {
        match slot.slot_type() {
            StorageSlotType::Map => map_slots += 1,
            StorageSlotType::Value => value_slots += 1,
        }
    }

    let tracked_storage_maps = account.storage().maps().count();
    let tracked_storage_map_entries: usize = account
        .storage()
        .maps()
        .map(|map| map.entries().count())
        .sum();
    let tracked_storage_map_leaves: usize = account
        .storage()
        .maps()
        .map(|map| map.leaves().count())
        .sum();
    let tracked_storage_map_inner_nodes: usize = account
        .storage()
        .maps()
        .map(|map| map.inner_nodes().count())
        .sum();
    let storage_inner_nodes = account.storage().inner_nodes().count();
    let storage_leaves = account.storage().leaves().count();

    let vault_leaves = account.vault().leaves().count();
    let vault_inner_nodes = account.vault().inner_nodes().count();

    println!();
    println!("Account:");
    println!("- serialized size: {}", bytes_with_unit(account_size));
    println!("- id: {}", account.id());
    println!("- account type: {:?}", account.id().account_type());
    println!("- storage mode: {}", account.id().storage_mode());
    println!("- nonce: {}", account.nonce());
    println!("- is new: {}", yes_no(account.is_new()));
    println!("- has public state: {}", yes_no(account.has_public_state()));
    println!("- account commitment: {}", account.to_commitment());
    println!("- initial commitment: {}", account.initial_commitment());
    println!("- code commitment: {}", account.code().commitment());
    println!("- code procedures: {}", account.code().num_procedures());
    println!("- storage commitment: {}", account.storage().commitment());
    println!(
        "- storage slots: {} (value={value_slots}, map={map_slots})",
        storage_header.num_slots()
    );
    println!("- tracked storage maps: {tracked_storage_maps}");
    println!("- tracked storage map entries: {tracked_storage_map_entries}");
    println!("- tracked storage map leaves: {tracked_storage_map_leaves}");
    println!("- tracked storage map inner nodes: {tracked_storage_map_inner_nodes}");
    println!("- storage inner nodes (all maps): {storage_inner_nodes}");
    println!("- storage leaves (all maps): {storage_leaves}");
    println!("- vault root: {}", account.vault().root());
    println!("- vault tracked leaves: {vault_leaves}");
    println!("- vault tracked inner nodes: {vault_inner_nodes}");
}

fn render_blockchain_stats(
    tx_inputs: &TransactionInputs,
    block_header_size: usize,
    blockchain_size: usize,
) {
    let block = tx_inputs.block_header();
    let blockchain = tx_inputs.blockchain();
    let peaks = blockchain.peaks();

    let mut min_tracked_block: Option<u32> = None;
    let mut max_tracked_block: Option<u32> = None;
    for header in blockchain.block_headers() {
        let block_num = header.block_num().as_u32();
        min_tracked_block = Some(min_tracked_block.map_or(block_num, |curr| curr.min(block_num)));
        max_tracked_block = Some(max_tracked_block.map_or(block_num, |curr| curr.max(block_num)));
    }

    println!();
    println!("Block / MMR:");
    println!(
        "- block_header serialized size: {}",
        bytes_with_unit(block_header_size)
    );
    println!(
        "- blockchain serialized size: {}",
        bytes_with_unit(blockchain_size)
    );
    println!("- reference block: {}", tx_inputs.ref_block().as_u32());
    println!("- block version: {}", block.version());
    println!("- block number: {}", block.block_num().as_u32());
    println!("- block epoch: {}", block.block_epoch());
    println!("- timestamp (unix seconds): {}", block.timestamp());
    println!("- block commitment: {}", block.commitment());
    println!("- chain commitment: {}", block.chain_commitment());
    println!("- account root: {}", block.account_root());
    println!("- nullifier root: {}", block.nullifier_root());
    println!("- note root: {}", block.note_root());
    println!("- tx commitment: {}", block.tx_commitment());
    println!("- tx kernel commitment: {}", block.tx_kernel_commitment());
    println!(
        "- fee native asset id: {}",
        block.fee_parameters().native_asset_id()
    );
    println!(
        "- verification base fee: {}",
        block.fee_parameters().verification_base_fee()
    );
    println!(
        "- chain length from partial blockchain: {}",
        blockchain.chain_length().as_u32()
    );
    println!(
        "- tracked block headers: {}",
        blockchain.num_tracked_blocks()
    );
    println!("- mmr leaves: {}", blockchain.mmr().num_leaves());
    println!("- mmr peaks: {}", peaks.num_peaks());
    println!(
        "- mmr tracked auth nodes: {}",
        blockchain.mmr().nodes().count()
    );

    match (min_tracked_block, max_tracked_block) {
        (Some(min), Some(max)) => println!("- tracked block range: {min}..={max}"),
        _ => println!("- tracked block range: n/a"),
    }
}

fn render_input_notes_stats(tx_inputs: &TransactionInputs, input_notes_size: usize, top: usize) {
    let input_notes = tx_inputs.input_notes();

    let mut authenticated_count = 0usize;
    let mut unauthenticated_count = 0usize;
    let mut total_note_assets = 0usize;
    let mut total_note_inputs = 0usize;
    let mut total_fungible_assets = 0usize;
    let mut total_non_fungible_assets = 0usize;
    let mut total_note_payload_size = 0usize;
    let mut total_note_proof_size = 0usize;
    let mut total_note_script_size = 0usize;
    let mut total_note_inputs_size = 0usize;
    let mut total_note_assets_size = 0usize;

    let mut note_sizes: Vec<SizeItem> = Vec::new();

    for (idx, input_note) in input_notes.iter().enumerate() {
        let note = input_note.note();
        let kind = match input_note {
            InputNote::Authenticated { .. } => {
                authenticated_count += 1;
                "authenticated"
            }
            InputNote::Unauthenticated { .. } => {
                unauthenticated_count += 1;
                "unauthenticated"
            }
        };

        let input_note_bytes = serialized_len(input_note);
        let note_payload_bytes = serialized_len(note);
        let note_proof_bytes = match input_note.proof() {
            Some(proof) => serialized_len(proof),
            None => 0,
        };
        let note_script_bytes = serialized_len(note.script());
        let note_inputs_bytes = serialized_len(note.storage());
        let note_assets_bytes = serialized_len(note.assets());

        total_note_payload_size += note_payload_bytes;
        total_note_proof_size += note_proof_bytes;
        total_note_script_size += note_script_bytes;
        total_note_inputs_size += note_inputs_bytes;
        total_note_assets_size += note_assets_bytes;

        total_note_assets += note.assets().num_assets();
        total_note_inputs += usize::from(note.storage().num_items());
        total_fungible_assets += note.assets().iter_fungible().count();
        total_non_fungible_assets += note.assets().iter_non_fungible().count();

        note_sizes.push(SizeItem {
            label: format!(
                "[{idx}] {} ({kind}, inputs={}, assets={})",
                note.id(),
                note.storage().num_items(),
                note.assets().num_assets()
            ),
            bytes: input_note_bytes,
        });
    }

    println!();
    println!("Input notes:");
    println!("- serialized size: {}", bytes_with_unit(input_notes_size));
    println!(
        "- count: {} (authenticated={}, unauthenticated={})",
        input_notes.num_notes(),
        authenticated_count,
        unauthenticated_count
    );
    println!("- notes commitment: {}", input_notes.commitment());
    println!("- total note inputs (felts): {total_note_inputs}");
    println!("- total note assets: {total_note_assets}");
    println!(
        "- asset mix: fungible={}, non-fungible={}",
        total_fungible_assets, total_non_fungible_assets
    );
    println!(
        "- payload vs proof bytes: payload={}, proof={}",
        bytes_with_unit(total_note_payload_size),
        bytes_with_unit(total_note_proof_size)
    );
    println!(
        "- payload internals: script={}, inputs={}, assets={}",
        bytes_with_unit(total_note_script_size),
        bytes_with_unit(total_note_inputs_size),
        bytes_with_unit(total_note_assets_size)
    );

    render_ranked_sizes(
        "Largest input notes",
        &mut note_sizes,
        input_notes_size,
        top,
    );
}

fn render_tx_args_stats(tx_inputs: &TransactionInputs, tx_args_size: usize) {
    let tx_args = tx_inputs.tx_args();
    let tx_script_size = tx_args.tx_script().map(serialized_len).unwrap_or(0);
    let tx_args_advice_size = serialized_len(tx_args.advice_inputs());
    let tx_args_advice_map_entries = tx_args.advice_inputs().map.len();
    let tx_args_advice_store_nodes = tx_args.advice_inputs().store.num_internal_nodes();
    let duplicated_advice = tx_args.advice_inputs() == tx_inputs.advice_inputs();

    println!();
    println!("Transaction args:");
    println!("- serialized size: {}", bytes_with_unit(tx_args_size));
    println!(
        "- tx script present: {}",
        yes_no(tx_args.tx_script().is_some())
    );
    if let Some(script) = tx_args.tx_script() {
        println!("- tx script root: {}", script.root());
        println!(
            "- tx script serialized size: {}",
            bytes_with_unit(tx_script_size)
        );
    }
    println!("- tx script args: {}", tx_args.tx_script_args());
    println!("- auth args: {}", tx_args.auth_args());
    println!(
        "- embedded tx_args.advice_inputs size: {}",
        bytes_with_unit(tx_args_advice_size)
    );
    println!("- embedded tx_args.advice map entries: {tx_args_advice_map_entries}");
    println!("- embedded tx_args.advice store nodes: {tx_args_advice_store_nodes}");
    println!(
        "- tx_args.advice_inputs equals top-level advice_inputs: {}",
        yes_no(duplicated_advice)
    );
}

fn render_advice_stats(tx_inputs: &TransactionInputs, advice_inputs_size: usize, top: usize) {
    let advice = tx_inputs.advice_inputs();
    let advice_stack_size = serialized_len(&advice.stack);
    let advice_map_size = serialized_len(&advice.map);
    let advice_store_size = serialized_len(&advice.store);

    let advice_map_elements: usize = advice.map.iter().map(|(_, values)| values.len()).sum();
    let advice_map_value_payload = advice_map_elements * 8;

    let mut largest_map_entries: Vec<SizeItem> = advice
        .map
        .iter()
        .map(|(key, values)| {
            let key_size = serialized_len(key);
            let values_size = serialized_len(&values.to_vec());
            SizeItem {
                label: format!("{key} ({} felts)", values.len()),
                bytes: key_size + values_size,
            }
        })
        .collect();

    let mut advice_parts = vec![
        SizeItem {
            label: "stack".into(),
            bytes: advice_stack_size,
        },
        SizeItem {
            label: "map".into(),
            bytes: advice_map_size,
        },
        SizeItem {
            label: "merkle store".into(),
            bytes: advice_store_size,
        },
    ];

    println!();
    println!("Advice inputs:");
    println!("- serialized size: {}", bytes_with_unit(advice_inputs_size));
    println!(
        "- stack len: {} elements ({})",
        advice.stack.len(),
        bytes_with_unit(advice_stack_size)
    );
    println!(
        "- map entries: {} (total mapped felts: {}, value payload: {})",
        advice.map.len(),
        advice_map_elements,
        bytes_with_unit(advice_map_value_payload)
    );
    println!(
        "- merkle store internal nodes: {} ({})",
        advice.store.num_internal_nodes(),
        bytes_with_unit(advice_store_size)
    );

    render_ranked_sizes(
        "Advice inputs breakdown",
        &mut advice_parts,
        advice_inputs_size,
        top,
    );
    render_ranked_sizes(
        "Largest advice map entries",
        &mut largest_map_entries,
        advice_map_size,
        top,
    );
}

fn render_foreign_data_stats(
    tx_inputs: &TransactionInputs,
    foreign_account_code_size: usize,
    top: usize,
) {
    let mut largest_foreign_codes: Vec<SizeItem> = tx_inputs
        .foreign_account_code()
        .iter()
        .enumerate()
        .map(|(idx, code)| SizeItem {
            label: format!(
                "[{idx}] commitment={} procedures={}",
                code.commitment(),
                code.num_procedures()
            ),
            bytes: serialized_len(code),
        })
        .collect();

    let mut largest_foreign_slot_names: Vec<SizeItem> = tx_inputs
        .foreign_account_slot_names()
        .iter()
        .map(|(slot_id, slot_name)| SizeItem {
            label: format!("{slot_id} -> {slot_name}"),
            bytes: serialized_len(slot_id) + serialized_len(slot_name),
        })
        .collect();

    println!();
    println!("Foreign account / witness data:");
    println!(
        "- foreign account code count: {} ({})",
        tx_inputs.foreign_account_code().len(),
        bytes_with_unit(foreign_account_code_size)
    );
    println!(
        "- foreign account slot names: {} ({})",
        tx_inputs.foreign_account_slot_names().len(),
        bytes_with_unit(serialized_len(tx_inputs.foreign_account_slot_names()))
    );

    render_ranked_sizes(
        "Largest foreign account code entries",
        &mut largest_foreign_codes,
        foreign_account_code_size,
        top,
    );
    render_ranked_sizes(
        "Largest foreign slot-name entries",
        &mut largest_foreign_slot_names,
        serialized_len(tx_inputs.foreign_account_slot_names()),
        top,
    );
}

fn render_ranked_sizes(title: &str, items: &mut [SizeItem], total: usize, top: usize) {
    println!();
    println!("{title}:");

    if items.is_empty() {
        println!("- none");
        return;
    }

    items.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.label.cmp(&b.label)));
    let shown = items.len().min(top);
    for (idx, item) in items.iter().take(shown).enumerate() {
        println!(
            "- {}. {} => {} ({:.2}%)",
            idx + 1,
            item.label,
            bytes_with_unit(item.bytes),
            pct(item.bytes, total)
        );
    }
    if items.len() > shown {
        println!("- ... {} more entries", items.len() - shown);
    }
}

fn serialized_len<T: Serializable + ?Sized>(value: &T) -> usize {
    value.to_bytes().len()
}

fn bytes_with_unit(bytes: usize) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;

    if bytes as f64 >= MIB {
        format!("{bytes} B ({:.2} MiB)", bytes as f64 / MIB)
    } else if bytes as f64 >= KIB {
        format!("{bytes} B ({:.2} KiB)", bytes as f64 / KIB)
    } else {
        format!("{bytes} B")
    }
}

fn pct(bytes: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        (bytes as f64) * 100.0 / (total as f64)
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
