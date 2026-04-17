use std::{collections::HashSet, path::PathBuf};

use anyhow::{Context, Result};
use miden_client::note::{NoteTag, NoteType};
use miden_client::store::{
    InputNoteRecord, InputNoteState, NoteFilter, OutputNoteRecord, OutputNoteState, Store,
};
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

pub(crate) fn render_input_note(note: &miden_client::store::InputNoteRecord) {
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
    render_well_known_inputs(&script_root, details.storage().items(), "- ", "  ");
    if let Some(created_at) = note.created_at() {
        println!("- created at: {created_at}");
    }
    if let Some(proof) = note.inclusion_proof() {
        println!(
            "- inclusion: block {} index {}",
            proof.location().block_num().as_u32(),
            proof.location().block_note_tree_index()
        );
    }
}

pub(crate) fn render_output_note(note: &miden_client::store::OutputNoteRecord) {
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
            proof.location().block_note_tree_index()
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
        render_well_known_inputs(&script_root, recipient.storage().items(), "- ", "  ");
    } else {
        println!("- recipient: n/a");
    }
}

pub(crate) struct NoteListFilters {
    pub(crate) include_input: bool,
    pub(crate) include_output: bool,
    pub(crate) states: Vec<String>,
    pub(crate) tag: Option<NoteTag>,
    pub(crate) note_type: Option<NoteType>,
}

pub(crate) fn list_store_notes(store_path: PathBuf, filters: NoteListFilters) -> Result<()> {
    let rt = Runtime::new()?;
    rt.block_on(async move {
        let store = SqliteStore::new(store_path.clone())
            .await
            .with_context(|| format!("failed to open store at {}", store_path.display()))?;

        let include_input = filters.include_input || !filters.include_output;
        let include_output = filters.include_output || !filters.include_input;
        let state_filters = normalize_state_filters(&filters.states);

        let mut lines = Vec::new();

        if include_input {
            let input_notes = store.get_input_notes(NoteFilter::All).await?;
            for note in input_notes {
                if matches_input_note(&note, &state_filters, filters.tag, filters.note_type) {
                    lines.push(format_input_note_line(&note));
                }
            }
        }

        if include_output {
            let output_notes = store.get_output_notes(NoteFilter::All).await?;
            for note in output_notes {
                if matches_output_note(&note, &state_filters, filters.tag, filters.note_type) {
                    lines.push(format_output_note_line(&note));
                }
            }
        }

        if lines.is_empty() {
            println!("No notes matched filters");
            return Ok(());
        }

        println!("Notes:");
        for line in lines {
            println!("{line}");
        }

        Ok(())
    })
}

fn normalize_state_filters(states: &[String]) -> HashSet<String> {
    let mut filters = HashSet::new();
    for state in states {
        let normalized = state.trim().to_ascii_lowercase();
        if !normalized.is_empty() {
            filters.insert(normalized);
        }
    }
    filters
}

fn matches_input_note(
    note: &InputNoteRecord,
    state_filters: &HashSet<String>,
    tag: Option<NoteTag>,
    note_type: Option<NoteType>,
) -> bool {
    let state_label = input_state_label(note.state());
    if !state_filters.is_empty() && !state_filters.contains(state_label) {
        return false;
    }

    if let Some(tag) = tag
        && note
            .metadata()
            .map(|meta| meta.tag() != tag)
            .unwrap_or(true)
    {
        return false;
    }

    if let Some(note_type) = note_type
        && note
            .metadata()
            .map(|meta| meta.note_type() != note_type)
            .unwrap_or(true)
    {
        return false;
    }

    true
}

fn matches_output_note(
    note: &OutputNoteRecord,
    state_filters: &HashSet<String>,
    tag: Option<NoteTag>,
    note_type: Option<NoteType>,
) -> bool {
    let state_label = output_state_label(note.state());
    if !state_filters.is_empty() && !state_filters.contains(state_label) {
        return false;
    }

    if let Some(tag) = tag
        && note.metadata().tag() != tag
    {
        return false;
    }

    if let Some(note_type) = note_type
        && note.metadata().note_type() != note_type
    {
        return false;
    }

    true
}

fn format_input_note_line(note: &InputNoteRecord) -> String {
    let state_label = input_state_label(note.state());
    let (note_type, tag, sender) = if let Some(metadata) = note.metadata() {
        (
            format!("{:?}", metadata.note_type()),
            format_note_tag(metadata.tag()),
            metadata.sender().to_string(),
        )
    } else {
        ("n/a".to_string(), "n/a".to_string(), "n/a".to_string())
    };

    format!(
        "- input {} state={} type={} tag={} sender={}",
        note.id(),
        state_label,
        note_type,
        tag,
        sender
    )
}

fn format_output_note_line(note: &OutputNoteRecord) -> String {
    let metadata = note.metadata();
    let state_label = output_state_label(note.state());
    let note_type = format!("{:?}", metadata.note_type());
    let tag = format_note_tag(metadata.tag());
    let sender = metadata.sender().to_string();

    format!(
        "- output {} state={} type={} tag={} sender={} expected={}",
        note.id(),
        state_label,
        note_type,
        tag,
        sender,
        note.expected_height().as_u32()
    )
}

fn input_state_label(state: &InputNoteState) -> &'static str {
    match state {
        InputNoteState::Expected(_) => "expected",
        InputNoteState::Unverified(_) => "unverified",
        InputNoteState::Committed(_) => "committed",
        InputNoteState::Invalid(_) => "invalid",
        InputNoteState::ProcessingAuthenticated(_) => "processing-authenticated",
        InputNoteState::ProcessingUnauthenticated(_) => "processing-unauthenticated",
        InputNoteState::ConsumedAuthenticatedLocal(_) => "consumed-authenticated",
        InputNoteState::ConsumedUnauthenticatedLocal(_) => "consumed-unauthenticated",
        InputNoteState::ConsumedExternal(_) => "consumed-external",
    }
}

fn output_state_label(state: &OutputNoteState) -> &'static str {
    match state {
        OutputNoteState::ExpectedPartial => "expected-partial",
        OutputNoteState::ExpectedFull { .. } => "expected-full",
        OutputNoteState::CommittedPartial { .. } => "committed-partial",
        OutputNoteState::CommittedFull { .. } => "committed-full",
        OutputNoteState::Consumed { .. } => "consumed",
    }
}
