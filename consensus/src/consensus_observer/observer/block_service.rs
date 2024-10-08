use crate::consensus_observer::{
    network::observer_message::OrderedBlock,
    observer::ordered_blocks::OrderedBlockStore,
};
use serde::{Deserialize, Serialize};
use warp::Filter; // Importing Warp to create an HTTP server and define API routes
use async_std::task; // Importing async-std to run async code
use tide::Request; // Importing Tide to create an HTTP server and define API routes

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
    pub async fn insert_dummy_block(&self, epoch: u64, round: u64) -> bool {
        let dummy_block = OrderedBlock::new_dummy(epoch, round);
        let insertion_success = self.ordered_block_store.insert_ordered_block(dummy_block);
        if !insertion_success {
            eprintln!("Failed to insert dummy block for epoch {}, round {}", epoch, round);
            return false;
        }
        true
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

    // Method to get the last ordered block
    pub fn get_last_ordered_block(&self) -> Option<serde_json::Value> {
        self.ordered_block_store.get_last_ordered_block().map(|block_info| {
            serde_json::json!({
                "epoch": block_info.epoch(),
                "round": block_info.round(),
                // Include other fields as necessary
            })
        })
    }

}

// Handler for the GET request to fetch a block by epoch and round
pub async fn get_block_handler_with_warp(
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

// Handler for the GET request to fetch a block by epoch and round
async fn get_block_handler_with_tide(req: tide::Request<OrderedBlockClient>) -> tide::Result {
    // Extract the parameters from the URL
    let epoch: u64 = req.param("epoch")?.parse()?;
    let round: u64 = req.param("round")?.parse()?;

    // Use the OrderedBlockClient to get the block
    let client = req.state();
    if let Some(block) = client.get_block_by_round(epoch, round) {
        let mut response = tide::Response::new(200);
        response.set_body(tide::Body::from_json(&block)?);
        Ok(response)
    } else {
        let mut response = tide::Response::new(404);
        response.set_body(tide::Body::from_json(&serde_json::json!({"error": "Block not found"}))?);
        Ok(response)
    }
}


// pub async fn start_block_service(ordered_block_store: OrderedBlockStore) {
//     println!("\n ................. \n ................ Starting block service...");

//     // Create the OrderedBlockClient instance
//     let ordered_block_client = OrderedBlockClient::new(ordered_block_store);

//     // Clone the client to use it within Warp filters (needed for request handlers)
//     let client_filter = warp::any().map(move || ordered_block_client.clone());

//     // Define the API route for fetching a block by its epoch and round
//     let get_block_by_round = warp::path!("blocks" / u64 / u64)
//         .and(warp::get())
//         .and(client_filter.clone())
//         .and_then(get_block_handler);

//     // Define a health check route
//     let health_route = warp::path("health")
//         .and(warp::get())
//         .map(|| warp::reply::json(&serde_json::json!({ "status": "ok" })));

//     // Combine the health route with other routes
//     let routes = get_block_by_round.or(health_route);

//     // Start the HTTP server on localhost:3030 with the defined routes
//     task::block_on(async {
//         warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
//     });
// }



pub async fn start_block_service_with_tide(ordered_block_store: OrderedBlockStore) {
    println!("\n ................. \n ................ Starting block service...");

    // Create the OrderedBlockClient instance
    let ordered_block_client = OrderedBlockClient::new(ordered_block_store);

    // Initialize a new tide server
    let mut app = tide::with_state(ordered_block_client);

    // Define the API route for fetching a block by its epoch and round
    app.at("/blocks/:epoch/:round").get(get_block_handler_with_tide);

    // Define a health check route
    app.at("/health").get(|_| async {
        let mut response = tide::Response::new(200);
        response.set_body(tide::Body::from_json(&serde_json::json!({"status": "ok"}))?);
        Ok(response)
    });

    // Start the HTTP server on localhost:3030
    app.listen("127.0.0.1:3030").await.unwrap();
}

// Main function to start the block service server
#[tokio::main]
pub async fn start_block_service_with_tokio(ordered_block_store: OrderedBlockStore) {
    println!("\n ................. \n ................ Starting block service...");

    // Create the OrderedBlockClient instance
    let ordered_block_client = OrderedBlockClient::new(ordered_block_store);

    // Clone the client to use it within Warp filters (needed for request handlers)
    let client_filter = warp::any().map(move || ordered_block_client.clone());

    // Define the API route for fetching a block by its epoch and round
    let get_block_by_round = warp::path!("blocks" / u64 / u64) // Matches the path "/blocks/{epoch}/{round}"
        .and(warp::get()) // Matches only GET requests
        .and(client_filter.clone()) // Pass the cloned OrderedBlockClient to the handler
        .and_then(get_block_handler_with_warp); // Calls the handler function when the route is matched

    // Define a health check route
    let health_route = warp::path("health")
        .and(warp::get())
        .map(|| warp::reply::json(&serde_json::json!({ "status": "ok" })));

    // Combine the health route with other routes
    let routes = get_block_by_round.or(health_route);

    // Start the HTTP server on localhost:3030 with the defined routes
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}
