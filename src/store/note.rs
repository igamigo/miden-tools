use std::path::PathBuf;

use anyhow::{Context, Result};
use miden_client::store::{NoteFilter, Store};
use miden_client_sqlite_store::SqliteStore;
use tokio::runtime::Runtime;

use crate::render::note::{
    format_note_tag, render_assets, render_well_known_inputs, well_known_label_from_root,
};

pub(crate) fn inspect_store_note(
    store_path: PathBuf,
    note_id: miden_client::note::NoteId,
) -> Result<()> {
    let rt = Runtime::new()?;
    rt.block_on(async move {
        let store = SqliteStore::new(store_path.clone())
            .await
            .with_context(|| format!("failed to open store at {}", store_path.display()))?;

        let input_notes = store
            .get_input_notes(NoteFilter::List(vec![note_id]))
            .await?;
        let output_notes = store
            .get_output_notes(NoteFilter::List(vec![note_id]))
            .await?;

        if input_notes.is_empty() && output_notes.is_empty() {
            println!("Note {note_id} not found in store");
            return Ok(());
        }

        let mut first = true;
        for note in input_notes {
            if !first {
                println!();
            }
            first = false;
            render_input_note(&note);
        }

        for note in output_notes {
            if !first {
                println!();
            }
            first = false;
            render_output_note(&note);
        }

        Ok(())
    })
}

fn render_input_note(note: &miden_client::store::InputNoteRecord) {
    let details = note.details();
    let script_root = details.script().root();
    let script_label = match well_known_label_from_root(&script_root) {
        Some(label) => format!("{script_root} ({label})"),
        None => script_root.to_string(),
    };

    println!("Input note {}:", note.id());
    println!("- state: {:?}", note.state());
    if let Some(metadata) = note.metadata() {
        println!("- sender: {}", metadata.sender());
        println!("- type: {:?}", metadata.note_type());
        println!("- tag: {}", format_note_tag(metadata.tag()));
    } else {
        println!("- metadata: n/a");
    }
    if let Some(commitment) = note.commitment() {
        println!("- commitment: {commitment}");
    }
    render_assets(details.assets());
    println!("- script root: {script_label}");
    render_well_known_inputs(&script_root, details.inputs().values(), "- ", "  ");
    if let Some(created_at) = note.created_at() {
        println!("- created at: {created_at}");
    }
    if let Some(proof) = note.inclusion_proof() {
        println!(
            "- inclusion: block {} index {}",
            proof.location().block_num().as_u32(),
            proof.location().node_index_in_block()
        );
    }
}

fn render_output_note(note: &miden_client::store::OutputNoteRecord) {
    let metadata = note.metadata();
    println!("Output note {}:", note.id());
    println!("- state: {:?}", note.state());
    println!("- expected height: {}", note.expected_height().as_u32());
    println!("- sender: {}", metadata.sender());
    println!("- type: {:?}", metadata.note_type());
    println!("- tag: {}", format_note_tag(metadata.tag()));
    if let Some(nullifier) = note.nullifier() {
        println!("- nullifier: {nullifier}");
    }
    if let Some(proof) = note.inclusion_proof() {
        println!(
            "- inclusion: block {} index {}",
            proof.location().block_num().as_u32(),
            proof.location().node_index_in_block()
        );
    }
    render_assets(note.assets());

    if let Some(recipient) = note.recipient() {
        let script_root = recipient.script().root();
        let script_label = match well_known_label_from_root(&script_root) {
            Some(label) => format!("{script_root} ({label})"),
            None => script_root.to_string(),
        };
        println!("- script root: {script_label}");
        render_well_known_inputs(&script_root, recipient.inputs().values(), "- ", "  ");
    } else {
        println!("- recipient: n/a");
    }
}
