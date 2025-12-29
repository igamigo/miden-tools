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

    Err(anyhow!("invalid account id, use 0x-hex or bech32 address"))
}

/// Parse a CLI word input: either a single 0x-hex word or four felts (decimal or 0x-hex).
pub(crate) fn word(values: &[String]) -> Result<Word> {
    match values.len() {
        1 => word_from_hex(values[0].as_str()),
        4 => word_from_felts(values),
        _ => Err(anyhow!("expected either 1 hex word or 4 felts")),
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
    NoteId::try_from_hex(raw).map_err(|err| anyhow!("invalid note id: {err}"))
}

/// Parse a hex-encoded transaction id.
pub(crate) fn transaction_id(raw: &str) -> Result<TransactionId> {
    let word = Word::try_from(raw).map_err(|err| anyhow!("invalid transaction id: {err}"))?;
    Ok(TransactionId::from(word))
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
