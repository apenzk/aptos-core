// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

// This example script connects to a locally running Aptos full node to fetch and print blocks starting from the genesis.
// It performs the following steps:
// 1. Initializes a REST client to interact with the Aptos full node using the URL specified in the `NODE_URL` variable.
// 2. Starts fetching blocks from the genesis block (height 0) and prints their details, including block height, block ID, timestamp, and number of transactions.
// 3. Continuously polls the blockchain for new blocks. If a block is not found (indicating no new blocks), it waits a short time before retrying.
// 4. The loop runs indefinitely, fetching and printing blocks as they become available.
//
// This script is designed to work with a local Aptos full node running on `http://0.0.0.0:8080/v1`.
// In our tests the localnet was started with the following in the root of the repository.
//      CARGO_NET_GIT_FETCH_WITH_CLI=true cargo run -p aptos-node -- --test


use aptos_sdk::rest_client::Client;
// use aptos_consensus_types::block::Block;
use aptos_api_types::Block;
use once_cell::sync::Lazy;
use std::{str::FromStr, time::Duration};
use tokio::time::sleep;
use url::Url;

// Use the local full node URL
static NODE_URL: Lazy<Url> = Lazy::new(|| {
    Url::from_str("http://0.0.0.0:8080/v1")
        .expect("Failed to parse node URL")
});

#[tokio::main]
async fn main() {
    let client = Client::new(NODE_URL.clone());

    // Start from the genesis block height
    let mut current_block_height = 0;

    loop {
        match client.get_block_by_height(current_block_height, true).await {
            Ok(response) => {
            // Print block details
            let block: Block = response.into_inner();
            println!("Block Height: {}", block.block_height);
            println!("Block ID: {}", block.block_hash);
            println!("Block Timestamp: {}", block.block_timestamp);
            println!("First Version: {}", block.first_version);
            println!("Last Version: {}", block.last_version);
            println!("Number of Transactions: {}", block.transactions.as_ref().map_or(0, |txs| txs.len()));
            println!("--------------------------");

            // Move to the next block
            current_block_height += 1;
            }
            Err(e) => {
                // Handle the case where there are no new blocks
                if e.to_string().contains("BlockNotFound") {
                    // Wait for 250ms before retrying for a new block
                    sleep(Duration::from_millis(250)).await;
                } else {
                    eprintln!("Error fetching block: {:?}", e);
                    break;
                }
            }
        }
    }
}
