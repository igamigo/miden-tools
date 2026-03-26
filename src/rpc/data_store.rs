//! Minimal [`DataStore`] wrapping [`Store`] + [`NodeRpcClient`] for use with
//! [`NoteConsumptionChecker`].

use std::collections::BTreeSet;
use std::sync::Arc;

use miden_client::{
    rpc::NodeRpcClient,
    store::{AccountStorageFilter, PartialBlockchainFilter, Store},
};
use miden_protocol::{
    MastForest, Word, ZERO,
    account::{
        Account, AccountId, PartialAccount, StorageMapWitness, StorageSlot, StorageSlotContent,
    },
    asset::{AssetVaultKey, AssetWitness},
    block::{BlockHeader, BlockNumber},
    crypto::merkle::{
        MerklePath,
        mmr::{InOrderIndex, PartialMmr},
    },
    note::NoteScript,
    transaction::{AccountInputs, PartialBlockchain},
    vm::FutureMaybeSend,
};
use miden_tx::{DataStore, DataStoreError, MastForestStore, TransactionMastStore};

pub struct NtxDataStore {
    store: Arc<dyn Store>,
    rpc: Arc<dyn NodeRpcClient>,
    mast_store: Arc<TransactionMastStore>,
}

impl NtxDataStore {
    pub fn new(store: Arc<dyn Store>, rpc: Arc<dyn NodeRpcClient>) -> Self {
        Self {
            store,
            rpc,
            mast_store: Arc::new(TransactionMastStore::new()),
        }
    }

    pub fn mast_store(&self) -> Arc<TransactionMastStore> {
        self.mast_store.clone()
    }
}

impl MastForestStore for NtxDataStore {
    fn get(&self, procedure_hash: &Word) -> Option<Arc<MastForest>> {
        self.mast_store.get(procedure_hash)
    }
}

impl DataStore for NtxDataStore {
    async fn get_transaction_inputs(
        &self,
        account_id: AccountId,
        mut block_refs: BTreeSet<BlockNumber>,
    ) -> Result<(PartialAccount, BlockHeader, PartialBlockchain), DataStoreError> {
        let ref_block = block_refs
            .pop_last()
            .ok_or(DataStoreError::other("block set is empty"))?;

        let partial_account_record = self
            .store
            .get_minimal_partial_account(account_id)
            .await?
            .ok_or(DataStoreError::AccountNotFound(account_id))?;

        let partial_account: PartialAccount = if partial_account_record.nonce() == ZERO {
            let full_record = self
                .store
                .get_account(account_id)
                .await?
                .ok_or(DataStoreError::AccountNotFound(account_id))?;
            let account: Account = full_record
                .try_into()
                .map_err(|_| DataStoreError::AccountNotFound(account_id))?;
            PartialAccount::from(&account)
        } else {
            partial_account_record
                .try_into()
                .map_err(|_| DataStoreError::AccountNotFound(account_id))?
        };

        let (block_header, _) = self
            .store
            .get_block_header_by_num(ref_block)
            .await?
            .ok_or(DataStoreError::BlockNotFound(ref_block))?;

        let block_headers: Vec<BlockHeader> = self
            .store
            .get_block_headers(&block_refs)
            .await?
            .into_iter()
            .map(|(header, _)| header)
            .collect();

        let partial_mmr =
            build_partial_mmr(&self.store, ref_block.as_u32(), &block_headers).await?;

        let partial_blockchain =
            PartialBlockchain::new(partial_mmr, block_headers).map_err(|err| {
                DataStoreError::other_with_source("error creating PartialBlockchain", err)
            })?;

        Ok((partial_account, block_header, partial_blockchain))
    }

    async fn get_foreign_account_inputs(
        &self,
        foreign_account_id: AccountId,
        ref_block: BlockNumber,
    ) -> Result<AccountInputs, DataStoreError> {
        use miden_client::rpc::AccountStateAt;
        use miden_client::transaction::ForeignAccount;

        // Fetch the full account via get_account_details.
        let fetched = self
            .rpc
            .get_account_details(foreign_account_id)
            .await
            .map_err(|e| {
                DataStoreError::other(format!(
                    "RPC error fetching foreign account {foreign_account_id}: {e}"
                ))
            })?;

        let account = match fetched {
            miden_client::rpc::domain::account::FetchedAccount::Public(account, _) => account,
            miden_client::rpc::domain::account::FetchedAccount::Private(_, _) => {
                return Err(DataStoreError::other(format!(
                    "foreign account {foreign_account_id} is private"
                )));
            }
        };

        self.mast_store.load_account_code(account.code());

        // Build AccountInputs from the full account.
        // NOTE: get_account() has the same is_public() bug for Network-mode accounts (won't
        // request details), so we build from the full account instead. Fixed in v0.14.0-beta.
        let partial = PartialAccount::from(account.as_ref());

        // Fetch the proof separately. If it fails (e.g. due to the is_public bug), fall back
        // to an empty witness — the consumption checker doesn't validate proofs.
        // TODO: ForeignAccount::public() rejects Network-mode accounts. Fixed in v0.14.0-beta.
        let witness = {
            let foreign = ForeignAccount::Public(foreign_account_id, Default::default());
            match self
                .rpc
                .get_account(foreign, AccountStateAt::Block(ref_block), None)
                .await
            {
                Ok((_, proof)) => proof.into_parts().0,
                Err(_) => miden_protocol::block::account_tree::AccountWitness::new(
                    foreign_account_id,
                    account.commitment(),
                    Default::default(),
                )
                .map_err(|e| DataStoreError::other(format!("{e}")))?,
            }
        };

        Ok(AccountInputs::new(partial, witness))
    }

    async fn get_vault_asset_witnesses(
        &self,
        account_id: AccountId,
        vault_root: Word,
        vault_keys: BTreeSet<AssetVaultKey>,
    ) -> Result<Vec<AssetWitness>, DataStoreError> {
        let mut witnesses = vec![];
        for key in vault_keys {
            match self.store.get_account_asset(account_id, key).await {
                Ok(Some((_, witness))) => witnesses.push(witness),
                Ok(None) => {
                    let vault = self.store.get_account_vault(account_id).await?;
                    if vault.root() != vault_root {
                        return Err(DataStoreError::other("vault root mismatch"));
                    }
                    let witness = AssetWitness::new(vault.open(key).into()).map_err(|err| {
                        DataStoreError::other_with_source("failed to open vault", err)
                    })?;
                    witnesses.push(witness);
                }
                Err(err) => {
                    return Err(DataStoreError::other_with_source(
                        "failed to get account asset",
                        err,
                    ));
                }
            }
        }
        Ok(witnesses)
    }

    async fn get_storage_map_witness(
        &self,
        account_id: AccountId,
        map_root: Word,
        _map_key: Word,
    ) -> Result<StorageMapWitness, DataStoreError> {
        let storage = self
            .store
            .get_account_storage(account_id, AccountStorageFilter::Root(map_root))
            .await?;

        match storage.slots().first().map(StorageSlot::content) {
            Some(StorageSlotContent::Map(map)) => Ok(map.open(&_map_key)),
            _ => Err(DataStoreError::other(format!(
                "storage map with root {map_root} not found for {account_id}"
            ))),
        }
    }

    fn get_note_script(
        &self,
        script_root: Word,
    ) -> impl FutureMaybeSend<Result<Option<NoteScript>, DataStoreError>> {
        let store = self.store.clone();
        async move {
            match store.get_note_script(script_root).await {
                Ok(script) => Ok(Some(script)),
                Err(_) => Err(DataStoreError::other(format!(
                    "note script with root {script_root} not found"
                ))),
            }
        }
    }
}

// MMR HELPERS
// ================================================================================================

async fn build_partial_mmr(
    store: &Arc<dyn Store>,
    forest: u32,
    authenticated_blocks: &[BlockHeader],
) -> Result<PartialMmr, DataStoreError> {
    let peaks = store
        .get_partial_blockchain_peaks_by_block_num(BlockNumber::from(forest))
        .await?;
    let mut partial_mmr = PartialMmr::from_peaks(peaks);

    let block_nums: Vec<BlockNumber> = authenticated_blocks
        .iter()
        .map(BlockHeader::block_num)
        .collect();
    let num_leaves = partial_mmr.forest().num_leaves();
    let paths = get_mmr_paths(store, &block_nums, num_leaves).await?;

    for (header, path) in authenticated_blocks.iter().zip(paths.iter()) {
        partial_mmr
            .track(header.block_num().as_usize(), header.commitment(), path)
            .map_err(|err| DataStoreError::other(format!("MMR error: {err}")))?;
    }

    Ok(partial_mmr)
}

async fn get_mmr_paths(
    store: &Arc<dyn Store>,
    block_nums: &[BlockNumber],
    forest: usize,
) -> Result<Vec<MerklePath>, miden_client::store::StoreError> {
    let mut node_indices = BTreeSet::new();
    for block_num in block_nums {
        let before: usize = forest & block_num.as_usize();
        let after = forest ^ before;
        let path_depth = after.ilog2() as usize;
        let mut idx = InOrderIndex::from_leaf_pos(block_num.as_usize());
        for _ in 0..path_depth {
            node_indices.insert(idx.sibling());
            idx = idx.parent();
        }
    }

    let node_indices: Vec<InOrderIndex> = node_indices.into_iter().collect();
    let mmr_nodes = store
        .get_partial_blockchain_nodes(PartialBlockchainFilter::List(node_indices))
        .await?;

    let mut paths = vec![];
    for block_num in block_nums {
        let mut nodes = vec![];
        let mut idx = InOrderIndex::from_leaf_pos(block_num.as_usize());
        while let Some(node) = mmr_nodes.get(&idx.sibling()) {
            nodes.push(*node);
            idx = idx.parent();
        }
        paths.push(MerklePath::new(nodes));
    }

    Ok(paths)
}
