// Internal trait that observers implement
pub use crate::ObserverResult;
use lighthouse_network::MessageId;

pub(crate) trait XatuObserverTrait: Send + Sync {
    fn on_gossip_block<E: types::EthSpec>(
        &self,
        _message_id: MessageId,
        _peer_id: libp2p::PeerId,
        _client: Option<String>,
        _block: std::sync::Arc<types::SignedBeaconBlock<E>>,
        _timestamp_millis: u64,
        _topic: String,
        _message_size: usize,
    ) -> ObserverResult {
        ObserverResult::Ok
    }

    fn on_gossip_attestation<E: types::EthSpec>(
        &self,
        _message_id: MessageId,
        _peer_id: libp2p::PeerId,
        _attestation: std::sync::Arc<types::SingleAttestation>,
        _subnet_id: types::SubnetId,
        _should_process: bool,
        _timestamp_millis: u64,
        _topic: String,
        _message_size: usize,
    ) -> ObserverResult {
        ObserverResult::Ok
    }

    fn on_gossip_aggregate_and_proof<E: types::EthSpec>(
        &self,
        _message_id: MessageId,
        _peer_id: libp2p::PeerId,
        _aggregate: std::sync::Arc<types::SignedAggregateAndProof<E>>,
        _timestamp_millis: u64,
        _topic: String,
        _message_size: usize,
    ) -> ObserverResult {
        ObserverResult::Ok
    }

    fn on_gossip_blob_sidecar<E: types::EthSpec>(
        &self,
        _message_id: MessageId,
        _peer_id: libp2p::PeerId,
        _client: Option<String>,
        _blob_index: u64,
        _blob_sidecar: std::sync::Arc<types::BlobSidecar<E>>,
        _timestamp_millis: u64,
        _topic: String,
        _message_size: usize,
    ) -> ObserverResult {
        ObserverResult::Ok
    }

    fn on_gossip_data_column_sidecar<E: types::EthSpec>(
        &self,
        _message_id: MessageId,
        _peer_id: libp2p::PeerId,
        _client: Option<String>,
        _subnet_id: types::DataColumnSubnetId,
        _column_sidecar: std::sync::Arc<types::DataColumnSidecar<E>>,
        _timestamp_millis: u64,
        _topic: String,
        _message_size: usize,
    ) -> ObserverResult {
        ObserverResult::Ok
    }

    fn on_gossip_execution_payload_envelope<E: types::EthSpec>(
        &self,
        _message_id: MessageId,
        _peer_id: libp2p::PeerId,
        _client: Option<String>,
        _envelope: std::sync::Arc<types::SignedExecutionPayloadEnvelope<E>>,
        _timestamp_millis: u64,
        _topic: String,
        _message_size: usize,
    ) -> ObserverResult {
        ObserverResult::Ok
    }

    fn on_gossip_execution_payload_bid<E: types::EthSpec>(
        &self,
        _message_id: MessageId,
        _peer_id: libp2p::PeerId,
        _client: Option<String>,
        _bid: std::sync::Arc<types::SignedExecutionPayloadBid<E>>,
        _timestamp_millis: u64,
        _topic: String,
        _message_size: usize,
    ) -> ObserverResult {
        ObserverResult::Ok
    }

    fn on_gossip_payload_attestation_message<E: types::EthSpec>(
        &self,
        _message_id: MessageId,
        _peer_id: libp2p::PeerId,
        _client: Option<String>,
        _message: std::sync::Arc<types::PayloadAttestationMessage>,
        _timestamp_millis: u64,
        _topic: String,
        _message_size: usize,
    ) -> ObserverResult {
        ObserverResult::Ok
    }

    fn on_gossip_proposer_preferences<E: types::EthSpec>(
        &self,
        _message_id: MessageId,
        _peer_id: libp2p::PeerId,
        _client: Option<String>,
        _preferences: std::sync::Arc<types::SignedProposerPreferences>,
        _timestamp_millis: u64,
        _topic: String,
        _message_size: usize,
    ) -> ObserverResult {
        ObserverResult::Ok
    }
}
