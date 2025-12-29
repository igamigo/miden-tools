use anyhow::{Context, Result};
use miden_client::store::{Store, TransactionFilter};
use miden_client::transaction::{OutputNote, TransactionRecord};
use miden_client_sqlite_store::SqliteStore;
use tokio::runtime::Runtime;

use crate::render::note::{render_well_known_inputs, well_known_label_from_root};

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
            render_transaction(&tx, verbose);
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

fn render_transaction(tx: &TransactionRecord, verbose: bool) {
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
    }
}
