use crate::consensus_observer::observer::block_service::{start_block_service, OrderedBlockClient};
use crate::consensus_observer::observer::ordered_blocks::OrderedBlockStore;
use reqwest;

/// This test function verifies the functionality of the ordered-block service client, 
/// by checking if it can fetch the latest block from the block store and the REST API endpoint.

#[tokio::test]
async fn test_block_service_client() {
    // Step 1: Initialize the runtime and OrderedBlockStore
    let ordered_block_store = OrderedBlockStore::new(Default::default());

    // Step 2: Start the block service in a separate task
    let ordered_block_store_clone = ordered_block_store.clone();
    tokio::spawn(async move {
        start_block_service(ordered_block_store_clone);
    });

    // Step 3: Create the OrderedBlockClient
    let client = OrderedBlockClient::new(ordered_block_store);

    // Step 4: Insert some blocks into the OrderedBlockStore for testing
    let epoch = 0;
    let round = 1;
    // This is a placeholder. You need to add code to insert ordered blocks as per your system's requirements.
    client.insert_dummy_block(epoch, round).await;

    // Step 5: Make a request to fetch the latest block using the client
    let latest_block = client.get_block_by_round(epoch, round);

  // Assert the block was retrieved successfully
  if let Some(block) = &latest_block {
    assert_eq!(block["epoch"], epoch);
    assert_eq!(block["round"], round);
} else {
    panic!("Block not found");
}

    // Step 6: Send a request to the REST API endpoint
    let api_url = format!("http://127.0.0.1:3030/blocks/{}/{}", epoch, round);
    let response = reqwest::get(&api_url).await.unwrap();
    assert!(response.status().is_success());
    
    let response_body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(response_body["epoch"], epoch);
    assert_eq!(response_body["round"], round);
}
