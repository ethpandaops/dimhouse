//! Wrapper to maintain backwards compatibility with Lighthouse integration

use crate::{ObserverResult, Xatu};
use libp2p::PeerId;
use lighthouse_network::MessageId;
use std::sync::Arc;
use types::EthSpec;

/// A wrapper that looks like a chain but just holds a single exporter
/// This is kept for backwards compatibility with the Lighthouse integration
pub struct XatuChain<E: EthSpec> {
    exporter: Option<Arc<dyn Xatu<E>>>,
}

impl<E: EthSpec> XatuChain<E> {
    /// Create a new empty chain
    pub fn new() -> Self {
        Self { exporter: None }
    }

    /// Create a chain with an exporter
    pub fn with_exporter(exporter: Arc<dyn Xatu<E>>) -> Self {
        Self {
            exporter: Some(exporter),
        }
    }

    /// Check if the chain has an exporter
    pub fn is_enabled(&self) -> bool {
        self.exporter.is_some()
    }

    /// Process a gossip block
    pub fn on_gossip_block(
        &self,
        message_id: MessageId,
        peer_id: PeerId,
        client: Option<String>,
        block: Arc<types::SignedBeaconBlock<E>>,
        timestamp: std::time::Duration,
        topic: String,
        message_size: usize,
    ) -> ObserverResult {
        if let Some(exporter) = &self.exporter {
            exporter.on_gossip_block(
                message_id,
                peer_id,
                client,
                block,
                timestamp.as_millis() as u64,
                topic,
                message_size,
            );
        }
        ObserverResult::Ok
    }

    /// Process a gossip attestation
    pub fn process_gossip_attestation(
        &self,
        message_id: MessageId,
        peer_id: PeerId,
        attestation: Arc<types::SingleAttestation>,
        subnet_id: types::SubnetId,
        should_process: bool,
        timestamp: std::time::Duration,
        topic: String,
        message_size: usize,
    ) -> ObserverResult {
        if let Some(exporter) = &self.exporter {
            exporter.on_gossip_attestation(
                message_id,
                peer_id,
                attestation,
                subnet_id,
                should_process,
                timestamp.as_millis() as u64,
                topic,
                message_size,
            );
        }
        ObserverResult::Ok
    }

    /// Process a gossip aggregate and proof
    pub fn process_gossip_aggregate_and_proof(
        &self,
        message_id: MessageId,
        peer_id: PeerId,
        aggregate: Arc<types::SignedAggregateAndProof<E>>,
        timestamp: std::time::Duration,
        topic: String,
        message_size: usize,
    ) -> ObserverResult {
        if let Some(exporter) = &self.exporter {
            exporter.on_gossip_aggregate_and_proof(
                message_id,
                peer_id,
                aggregate,
                timestamp.as_millis() as u64,
                topic,
                message_size,
            );
        }
        ObserverResult::Ok
    }

    /// Process a gossip blob sidecar
    pub fn process_gossip_blob_sidecar(
        &self,
        message_id: MessageId,
        peer_id: PeerId,
        client: Option<String>,
        blob_index: u64,
        blob_sidecar: Arc<types::BlobSidecar<E>>,
        timestamp: std::time::Duration,
        topic: String,
        message_size: usize,
    ) -> ObserverResult {
        if let Some(exporter) = &self.exporter {
            exporter.on_gossip_blob_sidecar(
                message_id,
                peer_id,
                client,
                blob_index,
                blob_sidecar,
                timestamp.as_millis() as u64,
                topic,
                message_size,
            );
        }
        ObserverResult::Ok
    }

    /// Process a gossip data column sidecar
    pub fn process_gossip_data_column_sidecar(
        &self,
        message_id: MessageId,
        peer_id: PeerId,
        client: Option<String>,
        subnet_id: types::DataColumnSubnetId,
        column_sidecar: Arc<types::DataColumnSidecar<E>>,
        timestamp: std::time::Duration,
        topic: String,
        message_size: usize,
    ) -> ObserverResult {
        if let Some(exporter) = &self.exporter {
            exporter.on_gossip_data_column_sidecar(
                message_id,
                peer_id,
                client,
                subnet_id,
                column_sidecar,
                timestamp.as_millis() as u64,
                topic,
                message_size,
            );
        }
        ObserverResult::Ok
    }
}
