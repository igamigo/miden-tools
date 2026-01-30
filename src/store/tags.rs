use std::path::PathBuf;

use anyhow::{Context, Result};
use miden_client::store::Store;
use miden_client_sqlite_store::SqliteStore;
use tokio::runtime::Runtime;

use crate::render::note::format_note_tag;

pub(crate) fn list_store_tags(store_path: PathBuf) -> Result<()> {
    let rt = Runtime::new()?;
    rt.block_on(async move {
        let store = SqliteStore::new(store_path.clone())
            .await
            .with_context(|| format!("failed to open store at {}", store_path.display()))?;

        let tags = store.get_note_tags().await?;
        if tags.is_empty() {
            println!("No tracked note tags");
            return Ok(());
        }

        println!("Tracked note tags:");
        for record in tags {
            let source = match record.source {
                miden_client::sync::NoteTagSource::Account(account_id) => {
                    format!("account {}", account_id)
                }
                miden_client::sync::NoteTagSource::Note(note_id) => format!("note {}", note_id),
                miden_client::sync::NoteTagSource::User => "user".to_string(),
            };
            println!("- {} source={}", format_note_tag(record.tag), source);
        }

        Ok(())
    })
}
