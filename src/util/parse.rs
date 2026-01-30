//! Helpers for parsing CLI input into strongly typed values.

use anyhow::{Context, Result, anyhow};
use miden_client::{
    Felt, Word,
    account::AccountId,
    address::{Address, AddressId, NetworkId},
    note::NoteId,
    rpc::Endpoint,
    transaction::TransactionId,
};
use miden_protocol::block::BlockNumber;

/// Parse a user-provided account identifier from hex or bech32.
pub(crate) fn account_id(raw: &str) -> Result<(AccountId, Option<NetworkId>)> {
    if let Ok(id) = AccountId::from_hex(raw) {
        return Ok((id, None));
    }

    if let Ok((network_id, address)) = Address::decode(raw) {
        if let AddressId::AccountId(id) = address.id() {
            return Ok((id, Some(network_id)));
        } else {
            return Err(anyhow!("address does not contain an account id"));
        }
    }

    // Provide helpful suggestions based on input format
    let suggestion = if raw.starts_with("miden") || raw.starts_with("test") {
        "Looks like a bech32 address but failed to decode. Check for typos or invalid characters."
    } else if raw.starts_with("0x") {
        "Hex value provided but failed to parse. Account IDs are 16 hex chars (e.g., 0x1234567890abcdef)."
    } else {
        "Use 0x-prefixed hex (e.g., 0x1234567890abcdef) or a bech32 address (e.g., miden1...)."
    };

    Err(anyhow!("invalid account id: {raw}\n  hint: {suggestion}"))
}

/// Parse a CLI word input: either a single 0x-hex word or four felts (decimal or 0x-hex).
pub(crate) fn word(values: &[String]) -> Result<Word> {
    match values.len() {
        1 => word_from_hex(values[0].as_str()),
        4 => word_from_felts(values),
        n => Err(anyhow!(
            "expected 1 hex word or 4 felts, got {n} values\n  hint: Provide either:\n    - One 0x-prefixed 64-char hex (e.g., 0x0123...)\n    - Four field elements (e.g., 123 456 789 0)"
        )),
    }
}

fn word_from_hex(value: &str) -> Result<Word> {
    Word::try_from(value).map_err(|err| anyhow!("invalid word hex: {err}"))
}

/// Parse four felt values (decimal or 0x-hex) into a Miden `Word`.
fn word_from_felts(values: &[String]) -> Result<Word> {
    if values.len() != 4 {
        return Err(anyhow!("expected exactly 4 felts"));
    }

    let parsed: [u64; 4] = values
        .iter()
        .map(|v| u64(v))
        .collect::<Result<Vec<_>>>()?
        .try_into()
        .expect("length enforced above");

    Ok(parsed.map(Felt::new).into())
}

/// Parse a CLI endpoint string into an RPC `Endpoint`.
pub(crate) fn endpoint_parameter(raw: &str) -> Result<Endpoint> {
    Endpoint::try_from(raw).map_err(anyhow::Error::msg)
}

/// Parse a hex-encoded note id.
pub(crate) fn note_id(raw: &str) -> Result<NoteId> {
    NoteId::try_from_hex(raw).map_err(|err| {
        let hint = if !raw.starts_with("0x") {
            "Note IDs must be 0x-prefixed (e.g., 0x1234...)"
        } else if raw.len() != 66 {
            "Note IDs are 64 hex chars (32 bytes) plus 0x prefix"
        } else {
            "Check for invalid hex characters"
        };
        anyhow!("invalid note id: {err}\n  hint: {hint}")
    })
}

/// Parse a hex-encoded transaction id.
pub(crate) fn transaction_id(raw: &str) -> Result<TransactionId> {
    let word = Word::try_from(raw).map_err(|err| {
        let hint = if !raw.starts_with("0x") {
            "Transaction IDs must be 0x-prefixed"
        } else if raw.len() != 66 {
            "Transaction IDs are 64 hex chars (32 bytes) plus 0x prefix"
        } else {
            "Check for invalid hex characters"
        };
        anyhow!("invalid transaction id: {err}\n  hint: {hint}")
    })?;
    Ok(TransactionId::from_raw(word))
}

/// Parse a block number from decimal or 0x-hex.
pub(crate) fn block_number(raw: &str) -> Result<BlockNumber> {
    let value = u64(raw).with_context(|| {
        "Block numbers can be decimal (e.g., 12345) or hex (e.g., 0x3039)"
    })?;
    let raw: u32 = value.try_into().map_err(|_| {
        anyhow!(
            "block number {} exceeds maximum ({})\n  hint: Block numbers are u32 values",
            value,
            u32::MAX
        )
    })?;
    Ok(BlockNumber::from(raw))
}

/// Parse a u64 from a string
pub(crate) fn u64(input: &str) -> Result<u64> {
    if let Some(stripped) = input.strip_prefix("0x") {
        u64::from_str_radix(stripped, 16).with_context(|| format!("invalid hex: {input}"))
    } else {
        input
            .parse::<u64>()
            .with_context(|| format!("invalid number: {input}"))
    }
}
