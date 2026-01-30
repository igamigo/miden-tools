use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use miden_client::store::{
    InputNoteRecord, InputNoteState, NoteFilter, OutputNoteRecord, OutputNoteState,
    PartialBlockchainFilter, Store, TransactionFilter,
};
use miden_client::transaction::TransactionStatusVariant;
use miden_client_sqlite_store::SqliteStore;
use rusqlite::{Connection, OptionalExtension};
use tokio::runtime::Runtime;

/// Print the default store path for this platform.
pub(crate) fn print_default_store_path() -> Result<()> {
    let base = dirs::data_local_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."));

    let store_dir = base.join("miden-client");
    let store_file = store_dir.join("store.sqlite3");

    println!("Default miden-client store locations:");
    println!();
    println!("  Directory: {}", store_dir.display());
    println!("  Store file: {}", store_file.display());
    println!();

    if store_file.exists() {
        println!("  Status: found (file exists)");
        if let Ok(meta) = fs::metadata(&store_file) {
            println!("  Size: {} bytes", meta.len());
        }
    } else if store_dir.exists() {
        println!("  Status: partial (directory exists, no store file)");
    } else {
        println!("  Status: not initialized (no store found)");
    }

    Ok(())
}

pub(crate) fn inspect_store(path: PathBuf) -> Result<()> {
    let file_size = fs::metadata(&path).map(|meta| meta.len()).ok();
    let conn = Connection::open(&path)
        .with_context(|| format!("failed to open sqlite store at {}", path.display()))?;
    let rt = Runtime::new()?;
    let stats = rt.block_on(async {
        let store = SqliteStore::new(path.clone()).await?;
        collect_store_stats(&store).await
    })?;

    println!("Inspecting store: {}", path.display());
    if let Some(size) = file_size {
        println!("- file size: {} bytes", size);
    }

    print_accounts(&conn, &stats)?;
    newline();
    print_notes(&stats)?;
    newline();
    print_chain(&conn, &stats)?;
    newline();
    print_transactions(&stats)?;
    newline();
    print_storage(&conn)?;
    newline();

    Ok(())
}

fn print_accounts(conn: &Connection, stats: &StoreStats) -> Result<()> {
    let total_states = query_u64(conn, "SELECT COUNT(*) FROM accounts")?;
    let multi_state_accounts = query_u64(
        conn,
        "SELECT COUNT(*) FROM (SELECT id FROM accounts GROUP BY id HAVING COUNT(*) > 1)",
    )?;
    let max_nonce = query_u64_opt(conn, "SELECT MAX(nonce) FROM accounts")?;

    println!("Accounts:");
    println!("- total rows: {}", total_states);
    println!("- distinct account ids: {}", stats.distinct_account_ids);
    println!("- accounts with history: {}", multi_state_accounts);
    if let Some(max_nonce) = max_nonce {
        println!("- max nonce: {}", max_nonce);
    }
    println!(
        "- tracked accounts: {}",
        query_u64(conn, "SELECT COUNT(*) FROM tracked_accounts")?
    );
    println!("- addresses: {}", stats.addresses);
    println!("- states per account:");
    for (account_id, count) in query_grouped_counts(conn, "accounts", "id")? {
        println!("  {account_id}: {count}");
    }
    Ok(())
}

fn print_notes(stats: &StoreStats) -> Result<()> {
    println!("Notes:");
    println!("- input notes: {}", stats.input_notes_total);
    for (label, count) in &stats.input_note_states {
        println!("- input state {}: {}", label, count);
    }
    println!("- output notes: {}", stats.output_notes_total);
    for (label, count) in &stats.output_note_states {
        println!("- output state {}: {}", label, count);
    }
    println!("- output notes unspent: {}", stats.output_unspent);
    println!(
        "- output notes with nullifier: {}",
        stats.output_with_nullifier
    );
    println!("- note tags: {}", stats.note_tags_total);
    println!("- unique note tags: {}", stats.unique_note_tags_total);
    Ok(())
}

fn print_chain(conn: &Connection, stats: &StoreStats) -> Result<()> {
    println!("Chain:");
    let block_count = query_u64(conn, "SELECT COUNT(*) FROM block_headers")?;
    let max_block = query_u64_opt(conn, "SELECT MAX(block_num) FROM block_headers")?;

    println!("- block headers: {}", block_count);
    if let Some(max_block) = max_block {
        println!("- max block: {}", max_block);
    }
    println!(
        "- blocks with client notes: {}",
        stats.tracked_block_headers
    );
    println!("- last state sync: {}", stats.sync_height);
    println!(
        "- partial blockchain nodes: {}",
        stats.partial_blockchain_nodes
    );
    Ok(())
}

fn print_transactions(stats: &StoreStats) -> Result<()> {
    println!("Transactions:");
    println!("- total: {}", stats.transactions_total);
    for (label, count) in &stats.transaction_states {
        println!("- status {}: {}", label, count);
    }
    Ok(())
}

fn print_storage(conn: &Connection) -> Result<()> {
    println!("Storage:");
    println!(
        "- account code rows: {}",
        query_u64(conn, "SELECT COUNT(*) FROM account_code")?
    );
    println!(
        "- account storage rows: {}",
        query_u64(conn, "SELECT COUNT(*) FROM account_storage")?
    );
    println!(
        "- storage map entries: {}",
        query_u64(conn, "SELECT COUNT(*) FROM storage_map_entries")?
    );
    println!(
        "- account assets: {}",
        query_u64(conn, "SELECT COUNT(*) FROM account_assets")?
    );
    println!(
        "- foreign account code: {}",
        query_u64(conn, "SELECT COUNT(*) FROM foreign_account_code")?
    );
    Ok(())
}

fn query_u64(conn: &Connection, sql: &str) -> Result<u64> {
    let value: i64 = conn.query_row(sql, [], |row| row.get(0))?;
    Ok(value.try_into().unwrap_or(0))
}

fn query_u64_opt(conn: &Connection, sql: &str) -> Result<Option<u64>> {
    let value: Option<i64> = conn.query_row(sql, [], |row| row.get(0)).optional()?;
    Ok(value.and_then(|val| val.try_into().ok()))
}

fn query_grouped_counts(
    conn: &Connection,
    table: &str,
    column: &str,
) -> Result<Vec<(String, u64)>> {
    let sql = format!("SELECT {column}, COUNT(*) FROM {table} GROUP BY {column} ORDER BY {column}");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| {
        let key: String = row.get(0)?;
        let count: i64 = row.get(1)?;
        Ok((key, count.try_into().unwrap_or(0)))
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

struct StoreStats {
    distinct_account_ids: u64,
    addresses: u64,
    input_notes_total: u64,
    input_note_states: Vec<(&'static str, u64)>,
    output_notes_total: u64,
    output_note_states: Vec<(&'static str, u64)>,
    output_unspent: u64,
    output_with_nullifier: u64,
    tracked_block_headers: u64,
    sync_height: u64,
    partial_blockchain_nodes: u64,
    transactions_total: u64,
    transaction_states: Vec<(&'static str, u64)>,
    note_tags_total: u64,
    unique_note_tags_total: u64,
}

async fn collect_store_stats(store: &SqliteStore) -> Result<StoreStats> {
    let account_ids = store.get_account_ids().await?;
    let mut address_count: u64 = 0;
    for account_id in &account_ids {
        address_count += store.get_addresses_by_account_id(*account_id).await?.len() as u64;
    }

    let input_notes = store.get_input_notes(NoteFilter::All).await?;
    let output_notes = store.get_output_notes(NoteFilter::All).await?;

    let (input_note_states, output_note_states) = count_note_states(&input_notes, &output_notes);
    let output_unspent = output_notes
        .iter()
        .filter(|note| !note.is_consumed())
        .count() as u64;
    let output_with_nullifier = output_notes
        .iter()
        .filter(|note| note.nullifier().is_some())
        .count() as u64;

    let tracked_block_headers = store.get_tracked_block_headers().await?.len() as u64;
    let sync_height = store.get_sync_height().await?.as_u32() as u64;
    let partial_blockchain_nodes = store
        .get_partial_blockchain_nodes(PartialBlockchainFilter::All)
        .await?
        .len() as u64;

    let transactions = store.get_transactions(TransactionFilter::All).await?;
    let transaction_states = count_transaction_states(&transactions);

    let note_tags_total = store.get_note_tags().await?.len() as u64;
    let unique_note_tags_total = store.get_unique_note_tags().await?.len() as u64;

    Ok(StoreStats {
        distinct_account_ids: account_ids.len() as u64,
        addresses: address_count,
        input_notes_total: input_notes.len() as u64,
        input_note_states,
        output_notes_total: output_notes.len() as u64,
        output_note_states,
        output_unspent,
        output_with_nullifier,
        tracked_block_headers,
        sync_height,
        partial_blockchain_nodes,
        transactions_total: transactions.len() as u64,
        transaction_states,
        note_tags_total,
        unique_note_tags_total,
    })
}

fn count_note_states(
    input_notes: &[InputNoteRecord],
    output_notes: &[OutputNoteRecord],
) -> (Vec<(&'static str, u64)>, Vec<(&'static str, u64)>) {
    let mut input_counts = [0u64; 9];
    for note in input_notes {
        let idx = match note.state() {
            InputNoteState::Expected(_) => 0,
            InputNoteState::Unverified(_) => 1,
            InputNoteState::Committed(_) => 2,
            InputNoteState::Invalid(_) => 3,
            InputNoteState::ProcessingAuthenticated(_) => 4,
            InputNoteState::ProcessingUnauthenticated(_) => 5,
            InputNoteState::ConsumedAuthenticatedLocal(_) => 6,
            InputNoteState::ConsumedUnauthenticatedLocal(_) => 7,
            InputNoteState::ConsumedExternal(_) => 8,
        };
        input_counts[idx] += 1;
    }

    let mut output_counts = [0u64; 5];
    for note in output_notes {
        let idx = match note.state() {
            OutputNoteState::ExpectedPartial => 0,
            OutputNoteState::ExpectedFull { .. } => 1,
            OutputNoteState::CommittedPartial { .. } => 2,
            OutputNoteState::CommittedFull { .. } => 3,
            OutputNoteState::Consumed { .. } => 4,
        };
        output_counts[idx] += 1;
    }

    let input_labels = [
        "expected",
        "unverified",
        "committed",
        "invalid",
        "processing-authenticated",
        "processing-unauthenticated",
        "consumed-authenticated",
        "consumed-unauthenticated",
        "consumed-external",
    ];
    let output_labels = [
        "expected-partial",
        "expected-full",
        "committed-partial",
        "committed-full",
        "consumed",
    ];

    let input_states = input_labels
        .into_iter()
        .zip(input_counts.into_iter())
        .collect();
    let output_states = output_labels
        .into_iter()
        .zip(output_counts.into_iter())
        .collect();

    (input_states, output_states)
}

fn count_transaction_states(
    transactions: &[miden_client::transaction::TransactionRecord],
) -> Vec<(&'static str, u64)> {
    let mut counts = [0u64; 3];
    for tx in transactions {
        let idx = match tx.status.variant() {
            TransactionStatusVariant::Pending => 0,
            TransactionStatusVariant::Committed => 1,
            TransactionStatusVariant::Discarded => 2,
        };
        counts[idx] += 1;
    }

    let labels = ["pending", "committed", "discarded"];
    labels.into_iter().zip(counts.into_iter()).collect()
}

fn newline() {
    println!();
}
