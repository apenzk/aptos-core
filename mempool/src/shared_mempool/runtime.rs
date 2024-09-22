// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

/*
This file sets up the shared mempool and its runtime environment by initializing the necessary components and spawning asynchronous tasks.
It is responsible for setting up the core asynchronous processes required for shared mempool operations in a distributed environment.

Function List and Descriptions:

- `start_shared_mempool`:
  - Initializes and starts the shared mempool by spawning tasks for transaction synchronization (`coordinator`), garbage collection (`gc_coordinator`), and optionally snapshot logging (`snapshot_job`).
  - Parameters include the Tokio runtime handle, node configuration, mempool state, network client, event receivers, and subscribers.
  - This function is critical for ensuring that shared mempool routines run concurrently.

- `bootstrap`:
  - Bootstraps the shared mempool by creating a dedicated runtime and initializing the core mempool and VM validator.
  - Spawns the necessary tasks using `start_shared_mempool`.
  - Returns the created runtime, allowing the shared mempool to operate in its own isolated environment.

Core Tasks and Responsibilities:
- `coordinator`: Handles network events and transaction broadcasts, coordinating between nodes.
- `gc_coordinator`: Performs garbage collection on expired transactions based on system TTL.
- `snapshot_job`: Periodically logs the internal state of the mempool for debugging and monitoring purposes.

*/


use crate::{
    core_mempool::CoreMempool,
    network::MempoolSyncMsg,
    shared_mempool::{
        coordinator::{coordinator, gc_coordinator, snapshot_job},
        types::{MempoolEventsReceiver, SharedMempool, SharedMempoolNotification},
    },
    QuorumStoreRequest,
};
use aptos_config::config::{NodeConfig, NodeType};
use aptos_event_notifications::{DbBackedOnChainConfig, ReconfigNotificationListener};
use aptos_infallible::{Mutex, RwLock};
use aptos_logger::Level;
use aptos_mempool_notifications::MempoolNotificationListener;
use aptos_network::application::{
    interface::{NetworkClient, NetworkServiceEvents},
    storage::PeersAndMetadata,
};
use aptos_storage_interface::DbReader;
use aptos_types::on_chain_config::OnChainConfigProvider;
use aptos_vm_validator::vm_validator::{PooledVMValidator, TransactionValidation};
use futures::channel::mpsc::{Receiver, UnboundedSender};
use std::sync::Arc;
use tokio::runtime::{Handle, Runtime};

/// Bootstrap of SharedMempool.
/// Creates a separate Tokio Runtime that runs the following routines:
///   - outbound_sync_task (task that periodically broadcasts transactions to peers).
///   - inbound_network_task (task that handles inbound mempool messages and network events).
///   - gc_task (task that performs GC of all expired transactions by SystemTTL).
pub(crate) fn start_shared_mempool<TransactionValidator, ConfigProvider>(
    executor: &Handle,
    config: &NodeConfig,
    mempool: Arc<Mutex<CoreMempool>>,
    network_client: NetworkClient<MempoolSyncMsg>,
    network_service_events: NetworkServiceEvents<MempoolSyncMsg>,
    client_events: MempoolEventsReceiver,
    quorum_store_requests: Receiver<QuorumStoreRequest>,
    mempool_listener: MempoolNotificationListener,
    mempool_reconfig_events: ReconfigNotificationListener<ConfigProvider>,
    db: Arc<dyn DbReader>,
    validator: Arc<RwLock<TransactionValidator>>,
    subscribers: Vec<UnboundedSender<SharedMempoolNotification>>,
    peers_and_metadata: Arc<PeersAndMetadata>,
) where
    TransactionValidator: TransactionValidation + 'static,
    ConfigProvider: OnChainConfigProvider,
{
    let node_type = NodeType::extract_from_config(config);
    let smp: SharedMempool<NetworkClient<MempoolSyncMsg>, TransactionValidator> =
        SharedMempool::new(
            mempool.clone(),
            config.mempool.clone(),
            network_client,
            db,
            validator,
            subscribers,
            node_type,
        );

    executor.spawn(coordinator(
        smp,
        executor.clone(),
        network_service_events,
        client_events,
        quorum_store_requests,
        mempool_listener,
        mempool_reconfig_events,
        config.mempool.shared_mempool_peer_update_interval_ms,
        peers_and_metadata,
    ));

    executor.spawn(gc_coordinator(
        mempool.clone(),
        config.mempool.system_transaction_gc_interval_ms,
    ));

    if aptos_logger::enabled!(Level::Trace) {
        executor.spawn(snapshot_job(
            mempool,
            config.mempool.mempool_snapshot_interval_secs,
        ));
    }
}

pub fn bootstrap(
    config: &NodeConfig,
    db: Arc<dyn DbReader>,
    network_client: NetworkClient<MempoolSyncMsg>,
    network_service_events: NetworkServiceEvents<MempoolSyncMsg>,
    client_events: MempoolEventsReceiver,
    quorum_store_requests: Receiver<QuorumStoreRequest>,
    mempool_listener: MempoolNotificationListener,
    mempool_reconfig_events: ReconfigNotificationListener<DbBackedOnChainConfig>,
    peers_and_metadata: Arc<PeersAndMetadata>,
) -> Runtime {
    let runtime = aptos_runtimes::spawn_named_runtime("shared-mem".into(), None);
    let mempool = Arc::new(Mutex::new(CoreMempool::new(config)));
    let vm_validator = Arc::new(RwLock::new(PooledVMValidator::new(
        Arc::clone(&db),
        num_cpus::get(),
    )));
    start_shared_mempool(
        runtime.handle(),
        config,
        mempool,
        network_client,
        network_service_events,
        client_events,
        quorum_store_requests,
        mempool_listener,
        mempool_reconfig_events,
        db,
        vm_validator,
        vec![],
        peers_and_metadata,
    );
    runtime
}
