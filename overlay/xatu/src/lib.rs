//! Xatu - Ethereum beacon chain event exporter
//!
//! This crate provides FFI-based event export functionality for Lighthouse.

// Public modules
pub mod config;
pub mod shim;

// Internal modules
mod chain;
mod ffi;
mod init;
mod metrics;
mod observer_ffi;
mod observer_trait;

use libp2p::PeerId;
use lighthouse_network::MessageId;
use std::sync::Arc;
use types::{EthSpec, SignedBeaconBlock};

pub use config::{NetworkInfo, XatuConfig};
pub use init::{init, init_with_chain_spec, init_with_chain_spec_and_genesis};

// Keep these for backwards compatibility with Lighthouse integration
pub use chain::XatuChain;
pub use shim::{create_exporter, create_exporter_from_config};

/// The main Xatu trait
pub trait Xatu<E: EthSpec>: Send + Sync {
    /// Called when a beacon block is received via gossip
    fn on_gossip_block(
        &self,
        message_id: MessageId,
        peer_id: PeerId,
        client: Option<String>,
        block: Arc<SignedBeaconBlock<E>>,
        timestamp_millis: u64,
        topic: String,
        message_size: usize,
    );

    /// Called when an attestation is received via gossip
    fn on_gossip_attestation(
        &self,
        message_id: MessageId,
        peer_id: PeerId,
        attestation: Arc<types::SingleAttestation>,
        subnet_id: types::SubnetId,
        should_process: bool,
        timestamp_millis: u64,
        topic: String,
        message_size: usize,
    );

    /// Called when an aggregate and proof is received via gossip
    fn on_gossip_aggregate_and_proof(
        &self,
        message_id: MessageId,
        peer_id: PeerId,
        aggregate: Arc<types::SignedAggregateAndProof<E>>,
        timestamp_millis: u64,
        topic: String,
        message_size: usize,
    );

    /// Called when a blob sidecar is received via gossip
    fn on_gossip_blob_sidecar(
        &self,
        message_id: MessageId,
        peer_id: PeerId,
        client: Option<String>,
        blob_index: u64,
        blob_sidecar: Arc<types::BlobSidecar<E>>,
        timestamp_millis: u64,
        topic: String,
        message_size: usize,
    );

    /// Called when a data column sidecar is received via gossip
    fn on_gossip_data_column_sidecar(
        &self,
        message_id: MessageId,
        peer_id: PeerId,
        client: Option<String>,
        subnet_id: types::DataColumnSubnetId,
        column_sidecar: Arc<types::DataColumnSidecar<E>>,
        timestamp_millis: u64,
        topic: String,
        message_size: usize,
    );

    /// Called when an ePBS execution payload envelope is received via gossip
    fn on_gossip_execution_payload_envelope(
        &self,
        message_id: MessageId,
        peer_id: PeerId,
        client: Option<String>,
        envelope: Arc<types::SignedExecutionPayloadEnvelope<E>>,
        timestamp_millis: u64,
        topic: String,
        message_size: usize,
    );

    /// Called when an ePBS execution payload bid is received via gossip
    fn on_gossip_execution_payload_bid(
        &self,
        message_id: MessageId,
        peer_id: PeerId,
        client: Option<String>,
        bid: Arc<types::SignedExecutionPayloadBid<E>>,
        timestamp_millis: u64,
        topic: String,
        message_size: usize,
    );

    /// Called when an ePBS payload attestation message (PTC vote) is received via gossip
    fn on_gossip_payload_attestation_message(
        &self,
        message_id: MessageId,
        peer_id: PeerId,
        client: Option<String>,
        message: Arc<types::PayloadAttestationMessage>,
        timestamp_millis: u64,
        topic: String,
        message_size: usize,
    );

    /// Called when an ePBS proposer preferences message is received via gossip
    fn on_gossip_proposer_preferences(
        &self,
        message_id: MessageId,
        peer_id: PeerId,
        client: Option<String>,
        preferences: Arc<types::SignedProposerPreferences>,
        timestamp_millis: u64,
        topic: String,
        message_size: usize,
    );
}

/// Result type for observer processing
#[derive(Debug, Clone, PartialEq)]
pub enum ObserverResult {
    Ok,
    Error(String),
}

/// Re-export the concrete implementation
pub use observer_ffi::XatuObserver;
