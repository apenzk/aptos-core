// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

/*
This file implements the Quorum Store Coordinator, which manages communication between Quorum Store components like BatchGenerator, ProofCoordinator, and ProofManager.

The coordinator receives commit notifications and shutdown signals, forwarding them to the relevant components. It ensures an orderly shutdown by sending commands to each component in sequence, maintaining proper operation during both regular execution and shutdown.

Function List and Descriptions:

- `new`: Creates a new instance of `QuorumStoreCoordinator`, initializing command channels for batch generation, proof coordination, and message handling.
- `start`: Main event loop that listens for `CoordinatorCommand` messages, handling commit notifications and shutdown signals.
- `CommitNotification`: Handles commit notifications by sending them to BatchGenerator, ProofCoordinator, and ProofManager for further processing.
- `Shutdown`: Coordinates the shutdown process by sending shutdown commands to all components (BatchGenerator, ProofCoordinator, ProofManager, Remote BatchCoordinator, and NetworkListener) and waits for completion.
*/


use crate::{
    monitor,
    quorum_store::{
        batch_coordinator::BatchCoordinatorCommand, batch_generator::BatchGeneratorCommand,
        counters, proof_coordinator::ProofCoordinatorCommand, proof_manager::ProofManagerCommand,
    },
    round_manager::VerifiedEvent,
};
use aptos_channels::aptos_channel;
use aptos_consensus_types::proof_of_store::BatchInfo;
use aptos_logger::prelude::*;
use aptos_types::{account_address::AccountAddress, PeerId};
use futures::StreamExt;
use tokio::sync::{mpsc, oneshot};

pub enum CoordinatorCommand {
    CommitNotification(u64, Vec<BatchInfo>),
    Shutdown(futures_channel::oneshot::Sender<()>),
}

pub struct QuorumStoreCoordinator {
    my_peer_id: PeerId,
    batch_generator_cmd_tx: mpsc::Sender<BatchGeneratorCommand>,
    remote_batch_coordinator_cmd_tx: Vec<mpsc::Sender<BatchCoordinatorCommand>>,
    proof_coordinator_cmd_tx: mpsc::Sender<ProofCoordinatorCommand>,
    proof_manager_cmd_tx: mpsc::Sender<ProofManagerCommand>,
    quorum_store_msg_tx: aptos_channel::Sender<AccountAddress, VerifiedEvent>,
}

impl QuorumStoreCoordinator {
    pub(crate) fn new(
        my_peer_id: PeerId,
        batch_generator_cmd_tx: mpsc::Sender<BatchGeneratorCommand>,
        remote_batch_coordinator_cmd_tx: Vec<mpsc::Sender<BatchCoordinatorCommand>>,
        proof_coordinator_cmd_tx: mpsc::Sender<ProofCoordinatorCommand>,
        proof_manager_cmd_tx: mpsc::Sender<ProofManagerCommand>,
        quorum_store_msg_tx: aptos_channel::Sender<AccountAddress, VerifiedEvent>,
    ) -> Self {
        Self {
            my_peer_id,
            batch_generator_cmd_tx,
            remote_batch_coordinator_cmd_tx,
            proof_coordinator_cmd_tx,
            proof_manager_cmd_tx,
            quorum_store_msg_tx,
        }
    }

    pub async fn start(self, mut rx: futures_channel::mpsc::Receiver<CoordinatorCommand>) {
        while let Some(cmd) = rx.next().await {
            monitor!("quorum_store_coordinator_loop", {
                match cmd {
                    CoordinatorCommand::CommitNotification(block_timestamp, batches) => {
                        counters::QUORUM_STORE_MSG_COUNT
                            .with_label_values(&["QSCoordinator::commit_notification"])
                            .inc();
                        // TODO: need a callback or not?
                        self.proof_coordinator_cmd_tx
                            .send(ProofCoordinatorCommand::CommitNotification(batches.clone()))
                            .await
                            .expect("Failed to send to ProofCoordinator");

                        self.proof_manager_cmd_tx
                            .send(ProofManagerCommand::CommitNotification(
                                block_timestamp,
                                batches.clone(),
                            ))
                            .await
                            .expect("Failed to send to ProofManager");

                        self.batch_generator_cmd_tx
                            .send(BatchGeneratorCommand::CommitNotification(
                                block_timestamp,
                                batches,
                            ))
                            .await
                            .expect("Failed to send to BatchGenerator");
                    },
                    CoordinatorCommand::Shutdown(ack_tx) => {
                        counters::QUORUM_STORE_MSG_COUNT
                            .with_label_values(&["QSCoordinator::shutdown"])
                            .inc();
                        // Note: Shutdown is done from the back of the quorum store pipeline to the
                        // front, so senders are always shutdown before receivers. This avoids sending
                        // messages through closed channels during shutdown.
                        // Oneshots that send data in the reverse order of the pipeline must assume that
                        // the receiver could be unavailable during shutdown, and resolve this without
                        // panicking.

                        let (network_listener_shutdown_tx, network_listener_shutdown_rx) =
                            oneshot::channel();
                        match self.quorum_store_msg_tx.push(
                            self.my_peer_id,
                            VerifiedEvent::Shutdown(network_listener_shutdown_tx),
                        ) {
                            Ok(()) => info!("QS: shutdown network listener sent"),
                            Err(err) => panic!("Failed to send to NetworkListener, Err {:?}", err),
                        };
                        network_listener_shutdown_rx
                            .await
                            .expect("Failed to stop NetworkListener");

                        let (batch_generator_shutdown_tx, batch_generator_shutdown_rx) =
                            oneshot::channel();
                        self.batch_generator_cmd_tx
                            .send(BatchGeneratorCommand::Shutdown(batch_generator_shutdown_tx))
                            .await
                            .expect("Failed to send to BatchGenerator");
                        batch_generator_shutdown_rx
                            .await
                            .expect("Failed to stop BatchGenerator");

                        for remote_batch_coordinator_cmd_tx in self.remote_batch_coordinator_cmd_tx
                        {
                            let (
                                remote_batch_coordinator_shutdown_tx,
                                remote_batch_coordinator_shutdown_rx,
                            ) = oneshot::channel();
                            remote_batch_coordinator_cmd_tx
                                .send(BatchCoordinatorCommand::Shutdown(
                                    remote_batch_coordinator_shutdown_tx,
                                ))
                                .await
                                .expect("Failed to send to Remote BatchCoordinator");
                            remote_batch_coordinator_shutdown_rx
                                .await
                                .expect("Failed to stop Remote BatchCoordinator");
                        }

                        let (proof_coordinator_shutdown_tx, proof_coordinator_shutdown_rx) =
                            oneshot::channel();
                        self.proof_coordinator_cmd_tx
                            .send(ProofCoordinatorCommand::Shutdown(
                                proof_coordinator_shutdown_tx,
                            ))
                            .await
                            .expect("Failed to send to ProofCoordinator");
                        proof_coordinator_shutdown_rx
                            .await
                            .expect("Failed to stop ProofCoordinator");

                        let (proof_manager_shutdown_tx, proof_manager_shutdown_rx) =
                            oneshot::channel();
                        self.proof_manager_cmd_tx
                            .send(ProofManagerCommand::Shutdown(proof_manager_shutdown_tx))
                            .await
                            .expect("Failed to send to ProofManager");
                        proof_manager_shutdown_rx
                            .await
                            .expect("Failed to stop ProofManager");

                        ack_tx
                            .send(())
                            .expect("Failed to send shutdown ack from QuorumStore");
                        break;
                    },
                }
            })
        }
    }
}
