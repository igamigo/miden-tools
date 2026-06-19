use miden_client::account::{Account, AccountHeader};
use miden_protocol::account::StorageSlotContent;
use miden_standards::account::faucets::FungibleFaucet;

use super::asset::format_asset;

pub(crate) fn render_account(account: &Account, include_vault: bool) {
    let header = AccountHeader::from(account);
    println!("- account type: {:?}", account.id().account_type());
    println!("- nonce: {}", header.nonce());
    println!("- vault commitment: {}", header.vault_root());
    println!("- storage commitment: {}", header.storage_commitment());
    println!("- code commitment: {}", header.code_commitment());
    println!("- header commitment: {}", header.to_commitment());

    if include_vault {
        render_faucet_metadata(account);
        render_storage(account);
        render_vault(account.vault());
    }
}

fn render_faucet_metadata(account: &Account) {
    // 0.15 collapsed `AccountType` to public/private, so faucets are no longer flagged on the id.
    // Attempt to decode the account as a fungible faucet; non-faucets simply won't decode.
    if let Ok(faucet) = FungibleFaucet::try_from(account) {
        println!("- faucet metadata:");
        println!("    symbol: {}", faucet.symbol());
        println!("    decimals: {}", faucet.decimals());
        println!("    issuance (token supply): {}", faucet.token_supply());
        println!("    max supply: {}", faucet.max_supply());
    }
}

fn render_storage(account: &Account) {
    let slots = account.storage().slots();
    println!("- storage slots: {}", slots.len());
    if slots.is_empty() {
        return;
    }

    println!("- storage slot details:");
    for (idx, slot) in slots.iter().enumerate() {
        match slot.content() {
            StorageSlotContent::Value(word) => {
                println!("  [{idx}] {} (value): {word}", slot.name());
            }
            StorageSlotContent::Map(map) => {
                println!(
                    "  [{idx}] {} (map, root={}, entries={}):",
                    slot.name(),
                    map.root(),
                    map.num_entries()
                );
                for (key, value) in map.entries() {
                    println!("    {key} -> {value}");
                }
            }
        }
    }
}

fn render_vault(vault: &miden_client::asset::AssetVault) {
    if vault.is_empty() {
        println!("- assets: 0");
        return;
    }

    println!("- assets: {}", vault.num_assets());
    println!("- asset details:");
    for (idx, asset) in vault.assets().enumerate() {
        println!("  [{idx}] {}", format_asset(&asset));
    }
}
