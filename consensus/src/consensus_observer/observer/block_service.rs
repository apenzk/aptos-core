use crate::consensus_observer::{
    network::observer_message::OrderedBlock,
    observer::ordered_blocks::OrderedBlockStore,
};
use serde::{Deserialize, Serialize};
use warp::Filter; // Importing Warp to create an HTTP server and define API routes

// Client to interact with the ordered block store
#[derive(Clone)]
pub struct OrderedBlockClient {
    ordered_block_store: OrderedBlockStore, // The internal ordered block store instance
}

impl OrderedBlockClient {
    // Constructor to create a new instance of OrderedBlockClient
    pub fn new(ordered_block_store: OrderedBlockStore) -> Self {
        Self { ordered_block_store }
    }

    // Inserts dummy block into the ordered block store for testing
    pub async fn insert_dummy_block(&self, epoch: u64, round: u64) {
        let dummy_block = OrderedBlock::new_dummy(epoch, round);
        self.ordered_block_store.insert_ordered_block(dummy_block);
    }

    // Method to retrieve a block by its epoch and round
    pub fn get_block_by_round(&self, epoch: u64, round: u64) -> Option<serde_json::Value> {
        // Fetch the ordered block from the store and serialize it into JSON format
        self.ordered_block_store.get_ordered_block(epoch, round).map(|block| {
            serde_json::json!({
                "epoch": epoch,
                "round": round,
                "block_info": block.last_block().block_info(),
            })
        })
    }
}

// Handler for the GET request to fetch a block by epoch and round
pub async fn get_block_handler(
    epoch: u64, 
    round: u64, 
    client: OrderedBlockClient
) -> Result<impl warp::Reply, warp::Rejection> {
    // Try to get the block using the OrderedBlockClient
    if let Some(block) = client.get_block_by_round(epoch, round) {
        // If block is found, return it as a JSON response
        Ok(warp::reply::json(&block))
    } else {
        // If block is not found, return a 404 status with a JSON error message
        Ok(warp::reply::json(&serde_json::json!({
            "error": "Block not found",
            "status": warp::http::StatusCode::NOT_FOUND.as_u16(),
        })))
    }
}



// Main function to start the block service server
#[tokio::main]
pub async fn start_block_service(ordered_block_store: OrderedBlockStore) {
    // Create the OrderedBlockClient instance
    let ordered_block_client = OrderedBlockClient::new(ordered_block_store);

    // Clone the client to use it within Warp filters (needed for request handlers)
    let client_filter = warp::any().map(move || ordered_block_client.clone());

    // Define the API route for fetching a block by its epoch and round
    let get_block_by_round = warp::path!("blocks" / u64 / u64) // Matches the path "/blocks/{epoch}/{round}"
        .and(warp::get()) // Matches only GET requests
        .and(client_filter.clone()) // Pass the cloned OrderedBlockClient to the handler
        .and_then(get_block_handler); // Calls the handler function when the route is matched

    // Combine all routes (if there were more, they would be combined here)
    let routes = get_block_by_round;

    // Start the HTTP server on localhost:3030 with the defined routes
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}
