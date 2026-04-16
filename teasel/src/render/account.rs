use miden_client::account::{Account, AccountHeader};

use super::asset::format_asset;

pub(crate) fn render_account(account: &Account, include_vault: bool) {
    let header = AccountHeader::from(account);
    println!("- account type: {:?}", account.account_type());
    println!("- nonce: {}", header.nonce());
    println!("- vault commitment: {}", header.vault_root());
    println!("- storage commitment: {}", header.storage_commitment());
    println!("- code commitment: {}", header.code_commitment());
    println!("- header commitment: {}", header.to_commitment());

    if include_vault {
        println!("- storage slots: {}", account.storage().slots().len());
        render_vault(account.vault());
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
