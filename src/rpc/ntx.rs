use std::sync::Arc;

use anyhow::{Result, anyhow};
use miden_client::{
    Client, ExecutionOptions, Felt,
    account::AccountId,
    crypto::RpoRandomCoin,
    keystore::FilesystemKeyStore,
    note::NoteId,
    notes::NoteFile,
    rpc::{Endpoint, GrpcClient, NodeRpcClient},
    store::Store,
};
use miden_client_sqlite_store::SqliteStore;
use miden_protocol::transaction::TransactionArgs;
use miden_tx::{NoteConsumptionChecker, TransactionExecutor};
use tokio::runtime::Runtime;

use super::data_store::NtxDataStore;
use crate::util::net::DEFAULT_TIMEOUT_MS;

pub(crate) fn debug_ntx(
    account_id: AccountId,
    note_ids: Vec<NoteId>,
    endpoint: Endpoint,
    verbose: bool,
) -> Result<()> {
    let rt = Runtime::new()?;
    rt.block_on(async move {
        let rpc = GrpcClient::new(&endpoint, DEFAULT_TIMEOUT_MS);

        // ── 1. Fetch notes from the network ──────────────────────────────
        println!("Fetching {} note(s) from {}...", note_ids.len(), endpoint);
        let fetched = rpc
            .get_notes_by_id(&note_ids)
            .await
            .map_err(|e| anyhow!("failed to fetch notes: {e}"))?;

        if fetched.is_empty() {
            return Err(anyhow!("no notes found on the network"));
        }

        let mut notes = Vec::new();
        let mut note_files = Vec::new();
        for f in &fetched {
            match f {
                miden_client::rpc::domain::note::FetchedNote::Public(note, proof) => {
                    notes.push(note.clone());
                    note_files.push(NoteFile::NoteWithProof(note.clone(), proof.clone()));
                }
                miden_client::rpc::domain::note::FetchedNote::Private(header, _) => {
                    println!("  note {} is private, skipping", header.id());
                }
            }
        }

        if notes.is_empty() {
            return Err(anyhow!(
                "all fetched notes are private; cannot test execution"
            ));
        }

        println!("Fetched {} public note(s)", notes.len());

        // Check attachments for network account targeting
        for note in &notes {
            let attachment = note.metadata().attachment();
            match miden_standards::note::NetworkAccountTarget::try_from(attachment) {
                Ok(target) => {
                    let target_id = target.target_id();
                    let matches = target_id == account_id;
                    println!(
                        "  note {} targets network account {} (hint: {:?}){}",
                        note.id(),
                        target_id,
                        target.execution_hint(),
                        if matches { "" } else { " ← MISMATCH" },
                    );
                }
                Err(_) => {
                    let kind = attachment.attachment_kind();
                    if !kind.is_none() {
                        println!(
                            "  note {} has non-standard attachment (scheme={}, kind={:?})",
                            note.id(),
                            attachment.attachment_scheme().as_u32(),
                            kind,
                        );
                    }
                }
            }
        }

        // ── 2. Create temp client ────────────────────────────────────────
        let tmp_dir = std::env::temp_dir().join(format!("distaff-ntx-{}", std::process::id()));
        std::fs::create_dir_all(&tmp_dir)?;
        let store_path = tmp_dir.join("store.sqlite3");

        let store: Arc<dyn Store> = Arc::new(
            SqliteStore::new(store_path)
                .await
                .map_err(|e| anyhow!("failed to create temp store: {e}"))?,
        );

        let coin_seed: [u64; 4] = [42, 0, 0, 0];
        let rng: miden_client::ClientRngBox =
            Box::new(RpoRandomCoin::new(coin_seed.map(Felt::new).into()));

        let rpc_client: Arc<dyn NodeRpcClient> =
            Arc::new(GrpcClient::new(&endpoint, DEFAULT_TIMEOUT_MS));

        let exec_options = ExecutionOptions::new(
            Some(miden_client::MAX_TX_EXECUTION_CYCLES),
            miden_client::MIN_TX_EXECUTION_CYCLES,
            false,
            false,
        )
        .map_err(|e| anyhow!("invalid execution options: {e}"))?;

        let mut client: Client<FilesystemKeyStore> = Client::new(
            rpc_client,
            rng,
            store.clone(),
            None,
            exec_options,
            None,
            None,
            None,
            None,
        )
        .await
        .map_err(|e| anyhow!("failed to create client: {e}"))?;

        // ── 3. Sync ─────────────────────────────────────────────────────
        let sync = client
            .sync_state()
            .await
            .map_err(|e| anyhow!("sync failed: {e}"))?;
        if verbose {
            println!("Synced client to block {}", sync.block_num.as_u32());
        }

        // ── 4. Import account ────────────────────────────────────────────
        if verbose {
            println!("Importing account {}...", account_id);
        }
        client
            .import_account_by_id(account_id)
            .await
            .map_err(|e| anyhow!("failed to import account: {e}"))?;

        // ── 5. Import notes ─────────────────────────────────────────────
        client
            .import_notes(&note_files)
            .await
            .map_err(|e| anyhow!("failed to import notes: {e}"))?;

        // ── 6. Consumption check via NoteConsumptionChecker ─────────────
        // TODO: Replace with Client::check_note_consumability when available in v0.14.
        println!();
        println!("Consumption check:");

        let rpc_for_ds: Arc<dyn NodeRpcClient> =
            Arc::new(GrpcClient::new(&endpoint, DEFAULT_TIMEOUT_MS));
        let data_store = NtxDataStore::new(store.clone(), rpc_for_ds);

        // Load account code into the MAST store
        let account_record = store
            .get_account(account_id)
            .await
            .map_err(|e| anyhow!("failed to get account: {e}"))?
            .ok_or_else(|| anyhow!("account not found in store after import"))?;
        let account: miden_protocol::account::Account = account_record
            .try_into()
            .map_err(|_| anyhow!("failed to convert account record"))?;
        data_store.mast_store().load_account_code(account.code());

        // Persist and load note scripts
        let note_scripts: Vec<_> = notes.iter().map(|n| n.script().clone()).collect();
        store
            .upsert_note_scripts(&note_scripts)
            .await
            .map_err(|e| anyhow!("failed to upsert note scripts: {e}"))?;
        for note in &notes {
            data_store.mast_store().insert(note.script().mast().clone());
        }

        let executor: TransactionExecutor<'_, '_, _, ()> = TransactionExecutor::new(&data_store)
            .with_options(exec_options)
            .map_err(|e| anyhow!("failed to create executor: {e}"))?;
        let checker = NoteConsumptionChecker::new(&executor);

        let result = checker
            .check_notes_consumability(
                account_id,
                sync.block_num,
                notes.clone(),
                TransactionArgs::default(),
            )
            .await;

        match result {
            Ok(info) => {
                for note in &info.successful {
                    println!("  note {}: consumable", note.id());
                }
                for failed in &info.failed {
                    println!("  note {}: FAILED", failed.note.id());
                    println!("    {}", failed.error);
                }
            }
            Err(err) => {
                println!("  checker error: {err}");
            }
        }

        let _ = std::fs::remove_dir_all(&tmp_dir);

        Ok(())
    })
}
