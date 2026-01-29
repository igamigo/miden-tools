use std::collections::HashMap;

use anyhow::{Context, Result};
use miden_client::note::{NoteId, Nullifier};
use miden_client::store::{InputNoteRecord, NoteFilter, OutputNoteRecord, Store, TransactionFilter};
use miden_client::transaction::{OutputNote, TransactionRecord};
use miden_client_sqlite_store::SqliteStore;
use tokio::runtime::Runtime;

use crate::render::note::{render_well_known_inputs, well_known_label_from_root};
use crate::store::note::{render_input_note, render_output_note};

pub(crate) fn inspect_transaction(
    store_path: std::path::PathBuf,
    tx_id: miden_client::transaction::TransactionId,
    verbose: bool,
) -> Result<()> {
    let rt = Runtime::new()?;
    rt.block_on(async move {
        let store = SqliteStore::new(store_path.clone())
            .await
            .with_context(|| format!("failed to open store at {}", store_path.display()))?;

        let transactions = store
            .get_transactions(TransactionFilter::Ids(vec![tx_id]))
            .await?;

        if transactions.is_empty() {
            println!("Transaction {tx_id} not found in store");
            return Ok(());
        }

        let mut first = true;
        for tx in transactions {
            if !first {
                println!();
            }
            first = false;
            let notes = if verbose {
                Some(load_transaction_notes(&store, &tx.details).await?)
            } else {
                None
            };
            render_transaction(&tx, verbose, notes.as_ref());
        }

        Ok(())
    })
}

pub(crate) fn list_transactions(store_path: std::path::PathBuf) -> Result<()> {
    let rt = Runtime::new()?;
    rt.block_on(async move {
        let store = SqliteStore::new(store_path.clone())
            .await
            .with_context(|| format!("failed to open store at {}", store_path.display()))?;

        let transactions = store.get_transactions(TransactionFilter::All).await?;
        if transactions.is_empty() {
            println!("No transactions found");
            return Ok(());
        }

        println!("Transactions:");
        for tx in transactions {
            println!("- {} ({})", tx.id, tx.status);
        }

        Ok(())
    })
}

fn render_transaction(tx: &TransactionRecord, verbose: bool, notes: Option<&TransactionNotes>) {
    let details = &tx.details;
    println!("Transaction {}:", tx.id);
    println!("- status: {}", tx.status);
    println!("- account id: {}", details.account_id);
    println!("- block num: {}", details.block_num.as_u32());
    println!(
        "- submission height: {}",
        details.submission_height.as_u32()
    );
    println!(
        "- expiration block: {}",
        details.expiration_block_num.as_u32()
    );
    println!(
        "- input nullifiers: {}",
        details.input_note_nullifiers.len()
    );
    println!("- output notes: {}", details.output_notes.num_notes());

    if verbose {
        println!(
            "- init account state: {}",
            details.init_account_state.to_hex()
        );
        println!(
            "- final account state: {}",
            details.final_account_state.to_hex()
        );
        println!(
            "- output notes commitment: {}",
            details.output_notes.commitment()
        );
        if !details.input_note_nullifiers.is_empty() {
            println!("- input nullifier list:");
            for (idx, nullifier) in details.input_note_nullifiers.iter().enumerate() {
                println!("  [{idx}] {}", nullifier.to_hex());
            }
        }
        if !details.output_notes.is_empty() {
            println!("- output notes:");
            for (idx, note) in details.output_notes.iter().enumerate() {
                let kind = match note {
                    OutputNote::Full(_) => "full",
                    OutputNote::Partial(_) => "partial",
                    OutputNote::Header(_) => "header",
                };
                println!("  [{idx}] {} ({kind})", note.id());
                if let Some(recipient) = note.recipient() {
                    let script_root = recipient.script().root();
                    let script_label = match well_known_label_from_root(&script_root) {
                        Some(label) => format!("{script_root} ({label})"),
                        None => script_root.to_string(),
                    };
                    println!("    script root: {script_label}");
                    render_well_known_inputs(
                        &script_root,
                        recipient.inputs().values(),
                        "    ",
                        "      ",
                    );
                }
            }
        }
        if let Some(notes) = notes {
            render_transaction_notes(notes);
        }
    }
}

struct TransactionNotes {
    input_notes: Vec<(Nullifier, Option<InputNoteRecord>)>,
    output_notes: Vec<(NoteId, Option<OutputNoteRecord>)>,
}

async fn load_transaction_notes(
    store: &SqliteStore,
    details: &miden_client::transaction::TransactionDetails,
) -> Result<TransactionNotes> {
    let input_nullifiers: Vec<Nullifier> = details
        .input_note_nullifiers
        .iter()
        .copied()
        .map(Nullifier::from)
        .collect();
    let input_notes = if input_nullifiers.is_empty() {
        Vec::new()
    } else {
        store
            .get_input_notes(NoteFilter::Nullifiers(input_nullifiers.clone()))
            .await?
    };

    let mut input_by_nullifier: HashMap<Nullifier, InputNoteRecord> = HashMap::new();
    for note in input_notes {
        input_by_nullifier.insert(note.nullifier(), note);
    }

    let input_notes = input_nullifiers
        .iter()
        .map(|nullifier| (*nullifier, input_by_nullifier.remove(nullifier)))
        .collect();

    let output_note_ids: Vec<NoteId> = details
        .output_notes
        .iter()
        .map(|note| note.id())
        .collect();
    let output_notes = if output_note_ids.is_empty() {
        Vec::new()
    } else {
        store
            .get_output_notes(NoteFilter::List(output_note_ids.clone()))
            .await?
    };

    let mut output_by_id: HashMap<NoteId, OutputNoteRecord> = HashMap::new();
    for note in output_notes {
        output_by_id.insert(note.id(), note);
    }

    let output_notes = output_note_ids
        .iter()
        .map(|note_id| (*note_id, output_by_id.remove(note_id)))
        .collect();

    Ok(TransactionNotes {
        input_notes,
        output_notes,
    })
}

fn render_transaction_notes(notes: &TransactionNotes) {
    let has_input = !notes.input_notes.is_empty();
    let has_output = !notes.output_notes.is_empty();
    if !has_input && !has_output {
        return;
    }

    println!();
    if has_input {
        render_input_notes(&notes.input_notes);
        if has_output {
            println!();
        }
    }
    if has_output {
        render_output_notes(&notes.output_notes);
    }
}

fn render_input_notes(notes: &[(Nullifier, Option<InputNoteRecord>)]) {
    if notes.is_empty() {
        return;
    }

    println!("Input notes (store):");
    let mut printed_any = false;
    for (nullifier, note) in notes {
        match note {
            Some(note) => {
                if printed_any {
                    println!();
                }
                render_input_note(note);
                printed_any = true;
            }
            None => {
                println!("- missing input note for nullifier {nullifier}");
                printed_any = true;
            }
        }
    }
}

fn render_output_notes(notes: &[(NoteId, Option<OutputNoteRecord>)]) {
    if notes.is_empty() {
        return;
    }

    println!("Output notes (store):");
    let mut printed_any = false;
    for (note_id, note) in notes {
        match note {
            Some(note) => {
                if printed_any {
                    println!();
                }
                render_output_note(note);
                printed_any = true;
            }
            None => {
                println!("- missing output note for id {note_id}");
                printed_any = true;
            }
        }
    }
}
