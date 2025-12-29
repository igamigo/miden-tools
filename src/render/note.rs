use miden_client::{
    BlockNumber, Felt, Word,
    account::AccountId,
    asset::Asset,
    note::{NoteAssets, NoteExecutionHint, NoteTag, NoteType, WellKnownNote},
};

use super::asset::format_asset;

pub(crate) fn well_known_label_from_root(script_root: &Word) -> Option<&'static str> {
    match *script_root {
        root if root == WellKnownNote::P2ID.script_root() => Some("P2ID"),
        root if root == WellKnownNote::P2IDE.script_root() => Some("P2IDE"),
        root if root == WellKnownNote::SWAP.script_root() => Some("SWAP"),
        root if root == WellKnownNote::MINT.script_root() => Some("MINT"),
        root if root == WellKnownNote::BURN.script_root() => Some("BURN"),
        _ => None,
    }
}

pub(crate) fn format_note_tag(tag: NoteTag) -> String {
    let raw: u32 = tag.into();
    let execution = tag.execution_mode();
    let target = if tag.is_single_target() {
        "single-target"
    } else {
        "use-case"
    };
    let note_types = if tag.validate(NoteType::Private).is_ok() {
        "any note type"
    } else {
        "public only"
    };

    format!("0x{raw:08x} (mode: {execution:?}, target: {target}, {note_types})")
}

pub(crate) fn render_assets(assets: &NoteAssets) {
    if assets.is_empty() {
        println!("- assets: 0");
        return;
    }

    println!("- assets: {}", assets.num_assets());
    println!("- asset details:");
    for (idx, asset) in assets.iter().enumerate() {
        println!("  [{idx}] {}", format_asset(asset));
    }
}

pub(crate) fn render_well_known_inputs(
    script_root: &Word,
    inputs: &[Felt],
    header_prefix: &str,
    line_prefix: &str,
) -> bool {
    let Some((label, lines)) = well_known_inputs(script_root, inputs) else {
        return false;
    };
    println!("{header_prefix}inputs ({label}):");
    if lines.is_empty() {
        println!("{line_prefix}(none)");
    } else {
        for line in lines {
            println!("{line_prefix}{line}");
        }
    }
    true
}

fn well_known_inputs(script_root: &Word, inputs: &[Felt]) -> Option<(&'static str, Vec<String>)> {
    let label = well_known_label_from_root(script_root)?;
    let lines = match label {
        "P2ID" => decode_p2id(inputs),
        "P2IDE" => decode_p2ide(inputs),
        "SWAP" => decode_swap(inputs),
        "MINT" => decode_mint(inputs),
        "BURN" => decode_burn(inputs),
        _ => Vec::new(),
    };
    Some((label, lines))
}

fn decode_p2id(inputs: &[Felt]) -> Vec<String> {
    if inputs.len() < 2 {
        return vec![format!("raw inputs: {} (expected 2)", inputs.len())];
    }
    let account = account_id_from_inputs(inputs[1], inputs[0]);
    vec![format!("target account: {account}")]
}

fn decode_p2ide(inputs: &[Felt]) -> Vec<String> {
    if inputs.len() < 4 {
        return vec![format!("raw inputs: {} (expected 4)", inputs.len())];
    }
    let account = account_id_from_inputs(inputs[1], inputs[0]);
    let reclaim = format_optional_block_height(inputs[2]);
    let timelock = format_optional_block_height(inputs[3]);
    vec![
        format!("target account: {account}"),
        format!("reclaim after block: {reclaim}"),
        format!("timelock until block: {timelock}"),
    ]
}

fn decode_swap(inputs: &[Felt]) -> Vec<String> {
    if inputs.len() < 12 {
        return vec![format!("raw inputs: {} (expected 12)", inputs.len())];
    }
    let requested_asset = format_asset_from_word(word_from_slice(inputs, 0).unwrap());
    let payback_recipient = word_from_slice(inputs, 4).unwrap();
    let execution_hint = format_execution_hint(inputs[8]);
    let note_type = format_note_type(inputs[9]);
    let note_aux = format_felt(inputs[10]);
    let note_tag = format_note_tag(NoteTag::from(inputs[11].as_int() as u32));

    vec![
        format!("requested asset: {requested_asset}"),
        format!("payback recipient: {}", payback_recipient.to_hex()),
        format!("payback execution hint: {execution_hint}"),
        format!("payback note type: {note_type}"),
        format!("payback note aux: {note_aux}"),
        format!("payback note tag: {note_tag}"),
    ]
}

fn decode_mint(inputs: &[Felt]) -> Vec<String> {
    if inputs.len() < 9 {
        return vec![format!("raw inputs: {} (expected 9)", inputs.len())];
    }
    let target_recipient = word_from_slice(inputs, 0).unwrap();
    let execution_hint = format_execution_hint(inputs[4]);
    let note_type = format_note_type(inputs[5]);
    let note_aux = format_felt(inputs[6]);
    let note_tag = format_note_tag(NoteTag::from(inputs[7].as_int() as u32));
    let amount = format_felt(inputs[8]);

    vec![
        format!("target recipient: {}", target_recipient.to_hex()),
        format!("output note execution hint: {execution_hint}"),
        format!("output note type: {note_type}"),
        format!("output note aux: {note_aux}"),
        format!("output note tag: {note_tag}"),
        format!("amount: {amount}"),
    ]
}

fn decode_burn(inputs: &[Felt]) -> Vec<String> {
    if !inputs.is_empty() {
        return vec![format!("raw inputs: {} (expected 0)", inputs.len())];
    }
    Vec::new()
}

fn account_id_from_inputs(prefix: Felt, suffix: Felt) -> String {
    let account_inputs = [prefix, suffix];
    AccountId::try_from(account_inputs)
        .map(|account| account.to_string())
        .unwrap_or_else(|_| "invalid".to_string())
}

fn word_from_slice(inputs: &[Felt], start: usize) -> Option<Word> {
    let chunk = inputs.get(start..start + 4)?;
    Some([chunk[0], chunk[1], chunk[2], chunk[3]].into())
}

fn format_optional_block_height(value: Felt) -> String {
    let raw = value.as_int();
    if raw == 0 {
        "none".to_string()
    } else if raw <= u32::MAX as u64 {
        BlockNumber::from(raw as u32).to_string()
    } else {
        format!("invalid ({raw})")
    }
}

fn format_execution_hint(value: Felt) -> String {
    match NoteExecutionHint::try_from(value.as_int()) {
        Ok(hint) => format!("{hint:?}"),
        Err(_) => format!("unknown ({})", value.as_int()),
    }
}

fn format_note_type(value: Felt) -> String {
    match NoteType::try_from(value) {
        Ok(note_type) => format!("{note_type:?}"),
        Err(_) => format!("unknown ({})", value.as_int()),
    }
}

fn format_felt(value: Felt) -> String {
    format!("{} (0x{:x})", value.as_int(), value.as_int())
}

fn format_asset_from_word(word: Word) -> String {
    match Asset::try_from(word) {
        Ok(asset) => match asset {
            Asset::Fungible(f) => format!("fungible amount={} faucet={}", f.amount(), f.faucet_id()),
            Asset::NonFungible(nf) => format!(
                "non-fungible faucet-prefix={} value={:?}",
                nf.faucet_id_prefix(),
                nf
            ),
        },
        Err(_) => format!("unknown asset ({})", word.to_hex()),
    }
}
