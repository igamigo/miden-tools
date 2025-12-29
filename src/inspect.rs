use std::{collections::BTreeSet, fs, path::PathBuf};

use anyhow::{Context, Result, anyhow};
use miden_client::{
    BlockNumber, Felt, Word,
    account::{AccountFile, AccountId},
    asset::Asset,
    note::{NoteAssets, NoteId, NoteInclusionProof, NoteTag, NoteType, Nullifier, WellKnownNote},
    notes::NoteFile,
    rpc::{Endpoint, GrpcClient, NodeRpcClient},
    utils::Deserializable,
};
use tokio::runtime::Runtime;

use crate::net::DEFAULT_TIMEOUT_MS;

pub(crate) fn inspect_note(note_id: NoteId, endpoint: Endpoint) -> Result<()> {
    let rt = Runtime::new()?;
    rt.block_on(async move {
        let rpc = GrpcClient::new(&endpoint, DEFAULT_TIMEOUT_MS);
        match rpc.get_notes_by_id(&[note_id]).await {
            Ok(notes) => {
                if notes.is_empty() {
                    println!("Note {note_id} not found on {endpoint}");
                    return Ok(());
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
pub(crate) fn inspect(file_path: PathBuf, endpoint: Option<Endpoint>) -> Result<()> {
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
            render_account_file(&account_file);
            if endpoint.is_some() {
                println!("- validation skipped for account files");
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
            let inputs = details.inputs().values();

            let is_p2id = script_root == WellKnownNote::P2ID.script_root();
            let script_label = if is_p2id {
                format!("{script_root} (P2ID)")
            } else {
                script_root.to_string()
            };
            let p2id_target = if is_p2id {
                extract_account_id_from_inputs(inputs)
            } else {
                None
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
            if let Some(target) = p2id_target {
                println!("- target account (P2ID): {target}");
            }
        }
        NoteFile::NoteWithProof(note, proof) => {
            let metadata = note.metadata();
            let location = proof.location();
            let script_root = note.script().root();
            let is_p2id = script_root == WellKnownNote::P2ID.script_root();
            let script_label = if is_p2id {
                format!("{script_root} (P2ID)")
            } else {
                script_root.to_string()
            };
            let p2id_target = if is_p2id {
                extract_account_id_from_inputs(note.inputs().values())
            } else {
                None
            };

            println!("- variant: NoteWithProof");
            println!("- note id: {}", note.id());
            println!("- sender: {}", metadata.sender());
            println!("- type: {:?}", metadata.note_type());
            println!("- tag: {}", format_note_tag(metadata.tag()));
            render_assets(note.assets());
            println!("- script root: {script_label}");
            println!("- created in block: {}", location.block_num().as_u32());
            println!("- node index in block: {}", location.node_index_in_block());
            if let Some(target) = p2id_target {
                println!("- target account (P2ID): {target}");
            }
        }
    }
}

fn render_account_file(account_file: &AccountFile) {
    let auth_keys = account_file.auth_secret_keys.len();
    let account = &account_file.account;
    let storage_slots = account.storage().slots().len();

    println!("- account id: {}", account.id());
    println!("- account type: {:?}", account.account_type());
    println!("- nonce: {}", account.nonce());
    println!("- storage slots: {storage_slots}");
    println!("- auth keys: {auth_keys}");
    println!(
        "- is public: {}",
        if account.is_public() { "yes" } else { "no" }
    );
}

fn extract_account_id_from_inputs(inputs: &[Felt]) -> Option<AccountId> {
    let account_inputs: [Felt; 2] = inputs.get(0..2)?.try_into().ok()?;
    AccountId::try_from([account_inputs[1], account_inputs[0]]).ok()
}

fn format_note_tag(tag: NoteTag) -> String {
    let raw: u32 = tag.into();
    let execution = tag.execution_mode();
    let target = if tag.is_single_target() {
        "single-target"
    } else {
        "use-case"
    };
    let note_types = if tag.validate(NoteType::Private).is_ok() {
        "any note type"
    } else {
        "public only"
    };

    format!("0x{raw:08x} (mode: {execution:?}, target: {target}, {note_types})")
}

fn render_assets(assets: &NoteAssets) {
    if assets.is_empty() {
        println!("- assets: 0");
        return;
    }

    println!("- assets: {}", assets.num_assets());
    println!("- asset details:");
    for (idx, asset) in assets.iter().enumerate() {
        println!("  [{idx}] {}", format_asset(asset));
    }
}

fn format_asset(asset: &Asset) -> String {
    match asset {
        Asset::Fungible(f) => format!("fungible amount={} faucet={}", f.amount(), f.faucet_id()),
        Asset::NonFungible(nf) => format!(
            "non-fungible faucet-prefix={} value={:?}",
            nf.faucet_id_prefix(),
            nf
        ),
    }
}

fn run_note_validation(note_file: &NoteFile, endpoint: Endpoint) -> Result<()> {
    let rt = Runtime::new()?;
    rt.block_on(validate_note(note_file, endpoint))
}

async fn validate_note(note_file: &NoteFile, endpoint: Endpoint) -> Result<()> {
    let local_nullifier = note_nullifier(note_file);

    let rpc = GrpcClient::new(&endpoint, DEFAULT_TIMEOUT_MS);

    println!("Validation (network: {}):", endpoint);

    match note_file {
        NoteFile::NoteWithProof(note, proof) => {
            verify_inclusion_with_header(&rpc, note.id(), proof, "local").await?;
        }
        NoteFile::NoteDetails {
            details,
            after_block_num,
            tag,
        } => {
            if let Some(tag) = tag {
                let mut tags = BTreeSet::new();
                tags.insert(*tag);
                let note_id = details.id();
                let mut cursor = *after_block_num;
                loop {
                    match rpc.sync_notes(cursor, None, &tags).await {
                        Ok(info) => {
                            if let Some(committed) =
                                info.notes.iter().find(|note| note.note_id() == &note_id)
                            {
                                verify_inclusion_with_root(
                                    note_id,
                                    committed.note_index(),
                                    committed.inclusion_path(),
                                    info.block_header.note_root(),
                                    "sync",
                                )?;
                                break;
                            }

                            let block_num = info.block_header.block_num();
                            if block_num == info.chain_tip {
                                println!(
                                    "- note {note_id} not found (chain tip {})",
                                    info.chain_tip.as_u32()
                                );
                                break;
                            }

                            cursor = block_num;
                        }
                        Err(err) => {
                            println!("- failed to sync notes by tag: {err}");
                            break;
                        }
                    }
                }
            } else {
                println!("- no note tag available to sync notes");
            }
        }
        NoteFile::NoteId(note_id) => {
            match rpc.get_notes_by_id(&[*note_id]).await {
                Ok(notes) => {
                    if notes.is_empty() {
                        println!("- note {note_id} not found on node");
                    } else {
                        for fetched in notes {
                            verify_inclusion_with_header(
                                &rpc,
                                fetched.id(),
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
        match rpc
            .get_nullifier_commit_height(&nullifier, BlockNumber::GENESIS)
            .await
        {
            Ok(Some(height)) => println!(
                "- nullifier {} is spent (committed at block {})",
                nullifier,
                height.as_u32()
            ),
            Ok(None) => println!(
                "- nullifier {} not found (unspent or not yet known)",
                nullifier
            ),
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
    note_id: NoteId,
    proof: &NoteInclusionProof,
    label: &str,
) -> Result<()> {
    let location = proof.location();
    let block_num = location.block_num();
    let note_index = location.node_index_in_block() as u64;
    let note_value: Word = note_id.into();

    match rpc.get_block_header_by_number(Some(block_num), false).await {
        Ok((header, _)) => {
            verify_inclusion_with_root(
                note_id,
                proof.location().node_index_in_block(),
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
    note_id: NoteId,
    note_index: u16,
    path: &miden_client::crypto::merkle::SparseMerklePath,
    root: Word,
    label: &str,
) -> Result<()> {
    let note_value: Word = note_id.into();
    let result = path.verify(note_index as u64, note_value, &root);
    match result {
        Ok(()) => println!(
            "- {label} inclusion proof: ok (index {note_index})"
        ),
        Err(err) => println!(
            "- {label} inclusion proof: failed (index {note_index}): {err}"
        ),
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
    println!("- node index in block: {}", inclusion.node_index_in_block());

    match fetched {
        miden_client::rpc::domain::note::FetchedNote::Public(note, _) => {
            render_assets(note.assets());
            println!("- script root: {}", note.script().root());
        }
        miden_client::rpc::domain::note::FetchedNote::Private(..) => {
            println!("- visibility: private (details not available)");
        }
    }
}
