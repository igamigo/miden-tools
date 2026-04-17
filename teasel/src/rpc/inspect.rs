use std::{collections::BTreeSet, fs, path::PathBuf};

use anyhow::{Context, Result, anyhow};
use miden_client::{
    Word,
    account::{AccountFile, AccountHeader},
    note::{NoteHeader, NoteId, NoteInclusionProof, Nullifier},
    notes::NoteFile,
    rpc::{Endpoint, GrpcClient, NodeRpcClient},
    utils::Deserializable,
};
use miden_crypto::merkle::SparseMerklePath;
use miden_protocol::block::BlockNumber;
use tokio::runtime::Runtime;

use crate::render::note::{
    format_note_tag, render_assets, render_attachment, render_well_known_inputs,
    well_known_label_from_root,
};
use crate::util::net::DEFAULT_TIMEOUT_MS;

pub(crate) fn inspect_note(
    note_id: NoteId,
    endpoint: Endpoint,
    save: Option<PathBuf>,
) -> Result<()> {
    let rt = Runtime::new()?;
    rt.block_on(async move {
        let rpc = GrpcClient::new(&endpoint, DEFAULT_TIMEOUT_MS);
        match rpc.get_notes_by_id(&[note_id]).await {
            Ok(notes) => {
                if notes.is_empty() {
                    println!("Note {note_id} not found on {endpoint}");
                    return Ok(());
                }
                if let Some(save_path) = save {
                    if notes.len() > 1 {
                        println!(
                            "Warning: received {} notes for {note_id}; using the first",
                            notes.len()
                        );
                    }
                    let (note_file, warning) = match &notes[0] {
                        miden_client::rpc::domain::note::FetchedNote::Public(note, proof) => {
                            (NoteFile::NoteWithProof(note.clone(), proof.clone()), None)
                        }
                        miden_client::rpc::domain::note::FetchedNote::Private(header, _proof) => (
                            NoteFile::NoteId(header.id()),
                            Some("note is private; saved NoteId-only NoteFile"),
                        ),
                    };
                    note_file.write(&save_path).with_context(|| {
                        format!("failed to write note file to {}", save_path.display())
                    })?;
                    println!("Saved NoteFile to {}", save_path.display());
                    if let Some(message) = warning {
                        println!("- warning: {}", message);
                    }
                }
                for note in notes {
                    render_fetched_note(&note);
                }
            }
            Err(err) => {
                println!("Failed to fetch note {note_id} from {endpoint}: {err}");
            }
        }
        Ok(())
    })
}

/// Inspect a serialized note or account file and optionally validate note data against a node.
pub(crate) fn inspect(file_path: PathBuf, endpoint: Option<Endpoint>, verbose: bool) -> Result<()> {
    let bytes =
        fs::read(&file_path).with_context(|| format!("failed to read {}", file_path.display()))?;

    if let Ok(note_file) = NoteFile::read_from_bytes(&bytes) {
        println!("Inspecting {} as NoteFile", file_path.display());
        render_note_file(&note_file);

        if let Some(endpoint) = endpoint {
            run_note_validation(&note_file, endpoint)?;
        }
        return Ok(());
    }

    match AccountFile::read_from_bytes(&bytes) {
        Ok(account_file) => {
            println!("Inspecting {} as AccountFile", file_path.display());
            render_account_file(&account_file, verbose);
            if let Some(endpoint) = endpoint {
                run_account_validation(&account_file, endpoint)?;
            }
            Ok(())
        }
        Err(account_err) => Err(anyhow!(
            "Failed to deserialize {} as note or account data\n  account error: {account_err}",
            file_path.display()
        )),
    }
}

fn render_note_file(note_file: &NoteFile) {
    match note_file {
        NoteFile::NoteId(note_id) => {
            println!("- variant: NoteId");
            println!("- note id: {note_id}");
        }
        NoteFile::NoteDetails {
            details,
            after_block_num,
            tag,
        } => {
            let script_root = details.script().root();
            let note_id = details.id();
            let inputs = details.storage().items();

            let script_label = match well_known_label_from_root(&script_root) {
                Some(label) => format!("{script_root} ({label})"),
                None => script_root.to_string(),
            };

            println!("- variant: NoteDetails");
            println!("- note id: {note_id}");
            render_assets(details.assets());
            println!("- script root: {script_label}");
            println!("- after block: {}", after_block_num.as_u32());
            println!(
                "- tag: {}",
                tag.map(format_note_tag).unwrap_or_else(|| "n/a".into())
            );
            render_well_known_inputs(&script_root, inputs, "- ", "  ");
        }
        NoteFile::NoteWithProof(note, proof) => {
            let metadata = note.metadata();
            let location = proof.location();
            let script_root = note.script().root();
            let script_label = match well_known_label_from_root(&script_root) {
                Some(label) => format!("{script_root} ({label})"),
                None => script_root.to_string(),
            };

            println!("- variant: NoteWithProof");
            println!("- note id: {}", note.id());
            println!("- sender: {}", metadata.sender());
            println!("- type: {:?}", metadata.note_type());
            println!("- tag: {}", format_note_tag(metadata.tag()));
            render_attachment(metadata.attachment(), "- ");
            render_assets(note.assets());
            println!("- script root: {script_label}");
            println!("- created in block: {}", location.block_num().as_u32());
            println!(
                "- node index in block: {}",
                location.block_note_tree_index()
            );
            render_well_known_inputs(&script_root, note.storage().items(), "- ", "  ");
        }
    }
}

fn render_account_file(account_file: &AccountFile, verbose: bool) {
    let auth_keys = account_file.auth_secret_keys.len();
    let account = &account_file.account;
    let slots = account.storage().slots();

    println!("- account id: {}", account.id());
    println!("- account type: {:?}", account.account_type());
    println!("- nonce: {}", account.nonce());
    println!("- storage slots: {}", slots.len());
    println!("- auth keys: {auth_keys}");
    println!(
        "- is public: {}",
        if account.is_public() { "yes" } else { "no" }
    );

    if verbose {
        use miden_protocol::account::StorageSlotContent;

        println!();
        println!("Storage slots:");
        for (idx, slot) in slots.iter().enumerate() {
            let name = slot.name();
            match slot.content() {
                StorageSlotContent::Value(word) => {
                    println!("  [{idx}] \"{name}\" (Value)");
                    println!("       {}", word.to_hex());
                }
                StorageSlotContent::Map(map) => {
                    let entry_count = map.entries().count();
                    println!(
                        "  [{idx}] \"{name}\" (Map, root={}, entries={})",
                        map.root().to_hex(),
                        entry_count
                    );
                    for (key, value) in map.entries() {
                        println!("       {} -> {}", key, value.to_hex());
                    }
                }
            }
        }

        println!();
        crate::render::account::render_account(account, true);
    }
}

fn run_note_validation(note_file: &NoteFile, endpoint: Endpoint) -> Result<()> {
    let rt = Runtime::new()?;
    rt.block_on(validate_note(note_file, endpoint))
}

fn run_account_validation(account_file: &AccountFile, endpoint: Endpoint) -> Result<()> {
    let rt = Runtime::new()?;
    rt.block_on(validate_account(account_file, endpoint))
}

async fn validate_account(account_file: &AccountFile, endpoint: Endpoint) -> Result<()> {
    let account = &account_file.account;
    let account_id = account.id();
    let local_header = AccountHeader::from(account);

    let rpc = GrpcClient::new(&endpoint, DEFAULT_TIMEOUT_MS);

    println!();
    println!("Validation (network: {}):", endpoint);

    match rpc.get_account_details(account_id).await {
        Ok(fetched) => {
            let on_chain_commitment = fetched.commitment();
            let local_commitment = local_header.to_commitment();

            match fetched {
                miden_client::rpc::domain::account::FetchedAccount::Public(
                    on_chain_account,
                    summary,
                ) => {
                    println!(
                        "- account exists: yes (last block: {})",
                        summary.last_block_num
                    );

                    let on_chain_header = AccountHeader::from(on_chain_account.as_ref());

                    // Overall commitment comparison
                    if local_commitment == on_chain_commitment {
                        println!("- commitment: match");
                    } else {
                        println!("- commitment: mismatch");
                        println!("    local:    {}", local_commitment);
                        println!("    on-chain: {}", on_chain_commitment);
                    }

                    // Nonce comparison (staleness detection)
                    let local_nonce = local_header.nonce();
                    let on_chain_nonce = on_chain_header.nonce();
                    let local_nonce_val = local_nonce.as_canonical_u64();
                    let on_chain_nonce_val = on_chain_nonce.as_canonical_u64();
                    if local_nonce_val == on_chain_nonce_val {
                        println!("- nonce: {} (in sync)", local_nonce);
                    } else if on_chain_nonce_val > local_nonce_val {
                        let diff = on_chain_nonce_val - local_nonce_val;
                        println!(
                            "- nonce: local={}, on-chain={} (stale by {})",
                            local_nonce, on_chain_nonce, diff
                        );
                    } else {
                        println!(
                            "- nonce: local={}, on-chain={} (local has uncommitted state)",
                            local_nonce, on_chain_nonce
                        );
                    }

                    // Vault commitment
                    let local_vault = local_header.vault_root();
                    let on_chain_vault = on_chain_header.vault_root();
                    if local_vault == on_chain_vault {
                        println!("- vault commitment: match");
                    } else {
                        println!("- vault commitment: mismatch");
                    }

                    // Storage commitment
                    let local_storage = local_header.storage_commitment();
                    let on_chain_storage = on_chain_header.storage_commitment();
                    if local_storage == on_chain_storage {
                        println!("- storage commitment: match");
                    } else {
                        println!("- storage commitment: mismatch");
                    }

                    // Code commitment
                    let local_code = local_header.code_commitment();
                    let on_chain_code = on_chain_header.code_commitment();
                    if local_code == on_chain_code {
                        println!("- code commitment: match");
                    } else {
                        println!("- code commitment: mismatch");
                    }
                }
                miden_client::rpc::domain::account::FetchedAccount::Private(_, summary) => {
                    println!(
                        "- account exists: yes (last block: {})",
                        summary.last_block_num
                    );

                    // Can only compare overall commitment for private accounts
                    if local_commitment == on_chain_commitment {
                        println!("- commitment: match");
                    } else {
                        println!("- commitment: mismatch");
                        println!("    local:    {}", local_commitment);
                        println!("    on-chain: {}", on_chain_commitment);
                    }

                    println!("- detailed comparison unavailable (on-chain account is private)");
                }
            }
        }
        Err(err) => {
            println!("- account exists: no");
            println!("- error: {err}");
        }
    }

    Ok(())
}

async fn validate_note(note_file: &NoteFile, endpoint: Endpoint) -> Result<()> {
    let local_nullifier = note_nullifier(note_file);

    let rpc = GrpcClient::new(&endpoint, DEFAULT_TIMEOUT_MS);

    println!();
    println!("Validation (network: {}):", endpoint);

    match note_file {
        NoteFile::NoteWithProof(note, proof) => {
            println!("- validation path: local inclusion proof (block header check)");
            verify_inclusion_with_header(&rpc, note.commitment(), proof, "local").await?;
        }
        NoteFile::NoteDetails {
            details,
            after_block_num,
            tag,
        } => {
            if let Some(tag) = tag {
                println!("- validation path: sync_notes by tag");
                let mut tags = BTreeSet::new();
                tags.insert(*tag);
                let note_id = details.id();
                let mut cursor = *after_block_num;
                loop {
                    match rpc.sync_notes(cursor, None, &tags).await {
                        Ok(info) => {
                            let mut found = false;
                            for block in &info.blocks {
                                if let Some(committed) = block.notes.get(&note_id) {
                                    let proof = committed.inclusion_proof();
                                    let location = proof.location();
                                    let header = match committed.metadata() {
                                        Some(m) => NoteHeader::new(note_id, m.clone()),
                                        None => {
                                            println!(
                                                "- note {note_id} found but metadata incomplete"
                                            );
                                            found = true;
                                            break;
                                        }
                                    };
                                    verify_inclusion_with_root(
                                        header.to_commitment(),
                                        location.block_note_tree_index(),
                                        proof.note_path(),
                                        block.block_header.note_root(),
                                        "sync",
                                    )?;
                                    found = true;
                                    break;
                                }
                            }
                            if found {
                                break;
                            }

                            if info.block_to == info.chain_tip {
                                println!(
                                    "- note {note_id} not found (chain tip {})",
                                    info.chain_tip.as_u32()
                                );
                                break;
                            }

                            cursor = info.block_to;
                        }
                        Err(err) => {
                            println!("- failed to sync notes by tag: {err}");
                            break;
                        }
                    }
                }
            } else {
                println!("- validation path: no note tag available to sync");
                println!("- no note tag available to sync notes");
            }
        }
        NoteFile::NoteId(note_id) => {
            println!("- validation path: get_notes_by_id");
            match rpc.get_notes_by_id(&[*note_id]).await {
                Ok(notes) => {
                    if notes.is_empty() {
                        println!("- note {note_id} not found on node");
                    } else {
                        for fetched in notes {
                            let header = NoteHeader::new(fetched.id(), fetched.metadata().clone());
                            verify_inclusion_with_header(
                                &rpc,
                                header.to_commitment(),
                                fetched.inclusion_proof(),
                                "node",
                            )
                            .await?;
                        }
                    }
                }
                Err(err) => {
                    println!("- failed to fetch note {note_id} from node: {err}");
                }
            }
        }
    }

    if let Some(nullifier) = local_nullifier {
        let mut nullifiers = std::collections::BTreeSet::new();
        nullifiers.insert(nullifier);
        match rpc
            .get_nullifier_commit_heights(nullifiers, BlockNumber::GENESIS)
            .await
        {
            Ok(heights) => {
                if let Some(height) = heights.get(&nullifier).copied().flatten() {
                    println!(
                        "- nullifier {} is spent (committed at block {})",
                        nullifier,
                        height.as_u32()
                    );
                } else {
                    println!("- nullifier {} not found (unspent)", nullifier);
                }
            }
            Err(err) => println!("- failed to check nullifier {}: {err}", nullifier),
        }
    } else {
        println!("- no nullifier available for this note variant");
    }

    Ok(())
}

// UTILS
// ================================================================================================

fn note_nullifier(note_file: &NoteFile) -> Option<Nullifier> {
    match note_file {
        NoteFile::NoteId(_) => None,
        NoteFile::NoteDetails { details, .. } => Some(details.nullifier()),
        NoteFile::NoteWithProof(note, _) => Some(note.nullifier()),
    }
}

async fn verify_inclusion_with_header(
    rpc: &GrpcClient,
    note_commitment: Word,
    proof: &NoteInclusionProof,
    label: &str,
) -> Result<()> {
    let location = proof.location();
    let block_num = location.block_num();

    match rpc.get_block_header_by_number(Some(block_num), false).await {
        Ok((header, _)) => {
            verify_inclusion_with_root(
                note_commitment,
                proof.location().block_note_tree_index(),
                proof.note_path(),
                header.note_root(),
                label,
            )?;
        }
        Err(err) => println!(
            "- {label} inclusion proof: failed to fetch block header {}: {err}",
            block_num.as_u32()
        ),
    }

    Ok(())
}

fn verify_inclusion_with_root(
    note_commitment: Word,
    note_index: u16,
    path: &SparseMerklePath,
    root: Word,
    label: &str,
) -> Result<()> {
    let result = path.verify(note_index as u64, note_commitment, &root);
    match result {
        Ok(()) => println!("- {label} inclusion proof: ok (index {note_index})"),
        Err(err) => println!("- {label} inclusion proof: failed (index {note_index}): {err}"),
    }
    Ok(())
}

fn render_fetched_note(fetched: &miden_client::rpc::domain::note::FetchedNote) {
    let metadata = fetched.metadata();
    let inclusion = fetched.inclusion_proof().location();

    println!("Note {}:", fetched.id());
    println!("- sender: {}", metadata.sender());
    println!("- type: {:?}", metadata.note_type());
    println!("- tag: {}", format_note_tag(metadata.tag()));
    println!("- included in block: {}", inclusion.block_num().as_u32());
    println!(
        "- node index in block: {}",
        inclusion.block_note_tree_index()
    );

    match fetched {
        miden_client::rpc::domain::note::FetchedNote::Public(note, _) => {
            render_assets(note.assets());
            let script_root = note.script().root();
            let script_label = match well_known_label_from_root(&script_root) {
                Some(label) => format!("{script_root} ({label})"),
                None => script_root.to_string(),
            };
            println!("- script root: {script_label}");
            render_well_known_inputs(&script_root, note.storage().items(), "- ", "  ");
            render_attachment(metadata.attachment(), "- ");
        }
        miden_client::rpc::domain::note::FetchedNote::Private(..) => {
            println!("- visibility: private (details not available)");
        }
    }
}
