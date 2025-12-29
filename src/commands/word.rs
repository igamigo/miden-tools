use anyhow::Result;
use miden_client::Word;

/// Build and print a Word from user-supplied input.
pub(crate) fn build_word(word: Word) -> Result<()> {
    println!("Word (as hex): {}", word.to_hex());
    println!("Word (decimal felts): {:?}", word.map(|f| f.as_int()));

    let hex_parts: Vec<String> = word.map(|f| format!("0x{:016x}", f.as_int())).to_vec();
    println!("Word (hex felts): [{}]", hex_parts.join(", "));
    Ok(())
}
