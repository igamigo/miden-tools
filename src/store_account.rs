use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use miden_client::account::AccountId;
use rusqlite::{Connection, params};

pub(crate) enum StoreAccountQuery {
    AccountId(AccountId),
    Commitment(String),
    Nonce(u64),
}

pub(crate) fn inspect_store_account(store_path: PathBuf, query: StoreAccountQuery) -> Result<()> {
    let conn = Connection::open(&store_path)
        .with_context(|| format!("failed to open store at {}", store_path.display()))?;

    let rows = match query {
        StoreAccountQuery::AccountId(account_id) => {
            let id_value: u128 = account_id.into();
            let id_value: i64 = id_value
                .try_into()
                .map_err(|_| anyhow!("account id out of range for sqlite storage"))?;
            query_accounts(&conn, "SELECT * FROM accounts WHERE id = ?", params![id_value])?
        }
        StoreAccountQuery::Commitment(commitment) => {
            query_accounts(
                &conn,
                "SELECT * FROM accounts WHERE account_commitment = ?",
                params![commitment],
            )?
        }
        StoreAccountQuery::Nonce(nonce) => {
            let nonce: i64 = nonce
                .try_into()
                .map_err(|_| anyhow!("nonce out of range for sqlite storage"))?;
            query_accounts(&conn, "SELECT * FROM accounts WHERE nonce = ?", params![nonce])?
        }
    };

    if rows.is_empty() {
        println!("No account records found");
        return Ok(());
    }

    println!("Account records:");
    for record in rows {
        render_record(&record);
    }

    Ok(())
}

#[derive(Debug)]
struct AccountRecordRow {
    id: i64,
    account_commitment: String,
    code_commitment: String,
    storage_commitment: String,
    vault_root: String,
    nonce: i64,
    locked: bool,
}

fn query_accounts<P: rusqlite::Params>(
    conn: &Connection,
    sql: &str,
    params: P,
) -> Result<Vec<AccountRecordRow>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params, |row| {
        Ok(AccountRecordRow {
            id: row.get(0)?,
            account_commitment: row.get(1)?,
            code_commitment: row.get(2)?,
            storage_commitment: row.get(3)?,
            vault_root: row.get(4)?,
            nonce: row.get(5)?,
            locked: row.get(7)?,
        })
    })?;

    let mut records = Vec::new();
    for row in rows {
        records.push(row?);
    }
    Ok(records)
}

fn render_record(record: &AccountRecordRow) {
    println!("- id: {}", record.id);
    println!("- nonce: {}", record.nonce);
    println!("- locked: {}", if record.locked { "yes" } else { "no" });
    println!("- account commitment: {}", record.account_commitment);
    println!("- code commitment: {}", record.code_commitment);
    println!("- storage commitment: {}", record.storage_commitment);
    println!("- vault root: {}", record.vault_root);
}
