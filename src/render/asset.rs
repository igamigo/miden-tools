use miden_client::asset::Asset;

pub(crate) fn format_asset(asset: &Asset) -> String {
    match asset {
        Asset::Fungible(f) => format!("fungible amount={} faucet={}", f.amount(), f.faucet_id()),
        Asset::NonFungible(nf) => {
            format!(
                "non-fungible faucet-prefix={} value={:?}",
                nf.faucet_id_prefix(),
                nf
            )
        }
    }
}
