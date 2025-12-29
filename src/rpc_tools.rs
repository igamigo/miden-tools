use std::time::Instant;

use anyhow::Result;
use miden_client::rpc::{Endpoint, GrpcClient, NodeRpcClient};
use tokio::runtime::Runtime;

use crate::net::DEFAULT_TIMEOUT_MS;

pub(crate) fn rpc_status(endpoint: Endpoint) -> Result<()> {
    let rt = Runtime::new()?;
    rt.block_on(async move {
        let rpc = GrpcClient::new(&endpoint, DEFAULT_TIMEOUT_MS);
        let connect_start = Instant::now();
        // Warm-up request to estimate connection setup cost.
        let _ = rpc.get_block_header_by_number(None, false).await;
        let connect_latency = connect_start.elapsed();
        let start = Instant::now();
        match rpc.get_block_header_by_number(None, false).await {
            Ok((header, _)) => {
                let latency = start.elapsed();
                println!("RPC status ({endpoint}):");
                println!("- latest block: {}", header.block_num().as_u32());
                println!("- block commitment: {}", header.commitment());
                println!("- chain commitment: {}", header.chain_commitment());
                println!("- timestamp: {}", header.timestamp());
                println!("- connection latency: {}ms", connect_latency.as_millis());
                println!("- request latency: {}ms", latency.as_millis());
            }
            Err(err) => {
                println!("RPC status ({endpoint}): error: {err}");
            }
        }
        Ok(())
    })
}
