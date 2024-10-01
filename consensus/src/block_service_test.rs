use crate::consensus_observer::observer::block_service::{start_block_service_with_tokio, start_block_service_with_tide, OrderedBlockClient};
use crate::consensus_observer::observer::ordered_blocks::OrderedBlockStore;
use reqwest;
use tokio::runtime::Builder;
use async_std::task;
use surf;


/// This test function verifies the functionality of the ordered-block service client, 
/// by checking if it can fetch the latest block from the block store and the REST API endpoint.

#[test]
fn test_block_service_client_with_tide() {
    // Run the async test using async-std
    task::block_on(async {
        // Step 1: Initialize the OrderedBlockStore
        println!("\n - - - - - - - - - \n - - - - - - - - - Step 1: Initializing the OrderedBlockStore");
        let ordered_block_store = OrderedBlockStore::new(Default::default());

        // Step 2: Start the block service in a separate async task
        println!("\n - - - - - - - - - \n - - - - - - - - - Step 2: Starting the block service in a separate task");
        let ordered_block_store_clone = ordered_block_store.clone();
        task::spawn(async move {
            start_block_service_with_tide(ordered_block_store_clone).await;
        });

        // Step 3: Delay to ensure the block service has time to start
        println!("\n - - - - - - - - - \n - - - - - - - - - Step 3: Delaying to ensure the block service has time to start");
        task::sleep(std::time::Duration::from_secs(10)).await;

        // Step 4.2: Check if the block service is running by sending a request
        println!("\n - - - - - - - - - \n - - - - - - - - - Step 4: Checking that the block service is running via API");
        let health_check_url = "http://127.0.0.1:3030/health";
        match surf::get(health_check_url).await {
            Ok(mut response) => {
                assert!(response.status() == 200, "Block service health check failed");
            }
            Err(err) => {
                panic!("\nFailed to connect to block service: {}", err);
            }
        }

        // Step 5: Create the OrderedBlockClient
        println!("\n - - - - - - - - - \n - - - - - - - - - Step 5: Creating the OrderedBlockClient");
        let client = OrderedBlockClient::new(ordered_block_store);

          // Step 5.1: Check if the block service is running by accessing the block store directly via the client
        println!("\n - - - - - - - - - \n - - - - - - - - - Step 5.1: Checking that the block store is accessible via the client");
        let last_block = client.get_last_ordered_block();
        if let Some(block_info) = last_block {
            println!("\n - - - - - - - - - \n - - - - - - - - - Last ordered block: {:?}", block_info);
        } else {
            println!("\n - - - - - - - - - \n - - - - - - - - - No blocks found in the store.");
        }

        // Step 6: Insert some blocks into the OrderedBlockStore for testing
        println!("\n - - - - - - - - - \n - - - - - - - - - Step 6: Inserting blocks into the OrderedBlockStore and check its accessible via the client.");
        let epoch = 0;
        for round in 1..=3 {
            println!("Inserting block for epoch {}, round {}", epoch, round);
            let insert_success = client.insert_dummy_block(epoch, round).await;
            if !insert_success {
                panic!("\n - - - - - - - - - \n - - - - - - - - - Failed to insert dummy block for epoch {}, round {}", epoch, round);
            }
            let last_block = client.get_last_ordered_block();
            if let Some(block_info) = last_block {
                println!("\n - - - - - - - - - \n - - - - - - - - - Last ordered block: {:?}", block_info);
            } else {
                println!("\n - - - - - - - - - \n - - - - - - - - - No blocks found in the store.");
            }    
        }


        // Step 7: Fetch the latest block using the client and assert the block was retrieved successfully
        println!("\n - - - - - - - - - \n - - - - - - - - - Step 7: Fetching the latest block using the client directly");
        let round = 2;
        let latest_block = client.get_block_by_round(epoch, round);
        if let Some(block) = &latest_block {
            println!("Block: {:?}", block);
            assert_eq!(block["epoch"], epoch);
            assert_eq!(block["round"], round);
        } else {
            panic!("\n - - - - - - - - - \n - - - - - - - - - Block not found");
        }

        // Step 8: Send a request to the REST API endpoint to verify the inserted block
        println!("\n - - - - - - - - - \n - - - - - - - - - Step 8: Sending request to the REST API endpoint for epoch {}, round {}", epoch, round);
        let api_url = format!("http://127.0.0.1:3030/blocks/{}/{}", epoch, round);
        let mut response = surf::get(&api_url).await.unwrap();
        assert!(response.status() == 200);
        println!("Response status: {}", response.status());

        let response_body: serde_json::Value = response.body_json().await.unwrap();
        assert_eq!(response_body["epoch"], epoch);
        assert_eq!(response_body["round"], round);
        println!("Response body: {:?}", response_body);
    });
}




// This test does not work. The error is 
// "Cannot start a runtime from within a runtime. This happens because a function (like `block_on`) attempted 
// to block the current thread while the thread is being used to drive asynchronous tasks."
#[tokio::test]
async fn test_block_service_client_tokio() {
    // Step 1: Initialize the runtime and OrderedBlockStore
    println!("\n - - - - - - - - - \n - - - - - - - - - Step 1: Initializing the runtime and OrderedBlockStore");
    let ordered_block_store = OrderedBlockStore::new(Default::default());

    // Step 2: Start the block service in a separate task
    // We cannot start a runtime from within a runtime. Hence we cannot call start_block_service with await. 
    // Therefore we add a delay to ensure the block service has time to start before making requests to it.
    println!("\n - - - - - - - - - \n - - - - - - - - - Step 2: Starting the block service in a separate task");
    let ordered_block_store_clone = ordered_block_store.clone();
    tokio::spawn(async move {
        start_block_service_with_tokio(ordered_block_store_clone);
    });
    

    std::thread::sleep(std::time::Duration::from_secs(2));

    // Step 2.1: Check that the block service is running by making a request to the health check endpoint
    // We cannot start a runtime from within a runtime. Hence we cannot call client.get with await. 
    // Therefore we add a delay to ensure the block service has time to start before making requests to it.
    println!("\n - - - - - - - - - \n - - - - - - - - - Step 2.1: Checking that the block service is running");
    let health_check_url = "http://127.0.0.1:3030/health";
    let client = reqwest::Client::new(); // Create a new reqwest client
    println!("\n - - - - - - - - - \n - - - - - - - - - Sending health check request to block service");

    // Send the HTTP request asynchronously
    let health_check_url = health_check_url.to_string(); // Clone the URL to move it into the blocking task
    let response = tokio::task::spawn_blocking(move || {
        // Create a new runtime within this blocking task
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        // Run the async request within this newly created runtime
        rt.block_on(async {
            client.get(&health_check_url).send().await
        })
    }).await.unwrap();
    
    match response {
        Ok(response) => {
            assert!(response.status().is_success(), "Block service health check failed");
        }
        Err(err) => {
            panic!("\n - - - - - - - - - \n - - - - - - - - - Failed to connect to block service: {}", err);
        }
    }
    std::thread::sleep(std::time::Duration::from_secs(2));


    // Step 3: Create the OrderedBlockClient
    println!("\n - - - - - - - - - \n - - - - - - - - - Step 3: Creating the OrderedBlockClient");
    let client = OrderedBlockClient::new(ordered_block_store);

    // Step 4: Insert some blocks into the OrderedBlockStore for testing
    println!("\n - - - - - - - - - \n - - - - - - - - - Step 4: Inserting blocks into the OrderedBlockStore");
    let epoch = 0;
    let round = 1;
    // This is a placeholder. You need to add code to insert ordered blocks as per your system's requirements.
    let insert_success = client.insert_dummy_block(epoch, round).await;
    if !insert_success {
        panic!("\n - - - - - - - - - \n - - - - - - - - - Failed to insert dummy block for epoch {}, round {}", epoch, round);
    }

    // Step 5: Make a request to fetch the latest block using the client and assert the block was retrieved successfully
    println!("\n - - - - - - - - - \n - - - - - - - - - Step 5: Fetching the latest block using the client");
    let latest_block = client.get_block_by_round(epoch, round);
    if let Some(block) = &latest_block {
        assert_eq!(block["epoch"], epoch);
        assert_eq!(block["round"], round);
    } else {
        panic!("\n - - - - - - - - - \n - - - - - - - - - Block not found");
    }

    // Step 6: Send a request to the REST API endpoint
    let api_url = format!("http://127.0.0.1:3030/blocks/{}/{}", epoch, round);
    let response = reqwest::get(&api_url).await.unwrap();
    assert!(response.status().is_success());
    
    let response_body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(response_body["epoch"], epoch);
    assert_eq!(response_body["round"], round);
}
