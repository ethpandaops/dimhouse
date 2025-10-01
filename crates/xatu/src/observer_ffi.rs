use crate::ffi::*;
use crate::observer_trait::ObserverResult;
use crossbeam_channel::{bounded, Sender};
use libp2p::PeerId;
use lighthouse_network::MessageId;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;
use tracing::{debug, error, info, warn};
use types::{
    BlobSidecar, DataColumnSidecar, DataColumnSubnetId, EthSpec, SignedAggregateAndProof,
    SignedBeaconBlock, SingleAttestation, SubnetId,
};

pub struct XatuObserver {
    initialized: Arc<AtomicBool>,
    network_info: Option<crate::config::NetworkInfo>,
    event_sender: Option<Sender<EventData>>,
}

impl XatuObserver {
    pub fn new_with_full_config(
        full_config: &crate::config::FullConfig,
        network_info: Option<crate::config::NetworkInfo>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let initialized = Arc::new(AtomicBool::new(false));

        // Clone for the spawned task
        let network_info_clone = network_info.clone();

        // Try to get log level from RUST_LOG env var or default to info
        let log_level = std::env::var("RUST_LOG")
            .ok()
            .and_then(|rust_log| {
                // Parse common RUST_LOG patterns
                if rust_log.contains("trace") {
                    Some("trace".to_string())
                } else if rust_log.contains("debug") {
                    Some("debug".to_string())
                } else if rust_log.contains("info") {
                    Some("info".to_string())
                } else if rust_log.contains("warn") {
                    Some("warn".to_string())
                } else if rust_log.contains("error") {
                    Some("error".to_string())
                } else {
                    Some("info".to_string())
                }
            })
            .or_else(|| Some("info".to_string()));

        // Get implementation details
        let client_name = "lighthouse";
        let client_version = env!("CARGO_PKG_VERSION");

        // Build Xatu processor config
        let xatu_config = crate::config::XatuProcessorConfig {
            name: full_config
                .node
                .as_ref()
                .map(|n| n.name.clone())
                .unwrap_or_else(|| "lighthouse".to_string()),
            outputs: full_config.outputs.clone(),
            ethereum: crate::config::XatuEthereum {
                implementation: "lighthouse".to_string(),
                genesis_time: network_info_clone
                    .as_ref()
                    .map(|n| n.genesis_time)
                    .unwrap_or(0),
                seconds_per_slot: network_info_clone
                    .as_ref()
                    .map(|n| n.seconds_per_slot)
                    .unwrap_or(12),
                slots_per_epoch: network_info_clone
                    .as_ref()
                    .map(|n| n.slots_per_epoch)
                    .unwrap_or(32),
                network: crate::config::Network {
                    name: network_info_clone
                        .as_ref()
                        .map(|n| n.network_name.clone())
                        .unwrap_or_else(|| "unknown".to_string()),
                    id: network_info_clone
                        .as_ref()
                        .map(|n| n.network_id)
                        .unwrap_or(0),
                },
            },
            client: crate::config::ClientInfo {
                name: client_name.to_string(),
                version: client_version.to_string(),
            },
            ntp_server: full_config.ntp_server.clone(),
        };

        // Create combined config with runtime info
        let config_with_runtime = crate::config::FullConfigWithRuntime {
            log_level,
            processor: xatu_config,
        };

        // If network info is missing, fail immediately
        if network_info.is_none() {
            return Err("Network info is required for Xatu initialization".into());
        }

        // Create a channel to get initialization result from dedicated thread
        let (init_sender, init_receiver) = std::sync::mpsc::channel();

        // Create event channel for batching - use crossbeam for thread safety
        let (event_sender, event_receiver) = bounded::<EventData>(10000);

        // Start dedicated FFI thread
        let initialized_for_thread = initialized.clone();
        thread::spawn(move || {
            debug!("Starting dedicated FFI thread");

            // Initialize FFI on this thread
            debug!("Initializing Xatu FFI on dedicated thread...");
            match XatuFFI::init_with_runtime(&config_with_runtime) {
                Ok(()) => {
                    initialized_for_thread.store(true, Ordering::Relaxed);
                    let _ = init_sender.send(Ok(()));
                }
                Err(e) => {
                    error!("FATAL: Failed to initialize Xatu FFI: {}", e);
                    let _ = init_sender.send(Err(e));
                    return;
                }
            }

            // Continue with batch processing on same thread
            debug!("Starting Xatu event batch processor on same thread with 1 second interval and max batch size of 10000");
            let mut event_batch = Vec::new();
            let mut total_events_processed = 0u64;
            let mut total_batches_sent = 0u64;
            let mut last_batch_time = std::time::Instant::now();

            loop {
                // Check if it's time to send a batch (1 second interval)
                let now = std::time::Instant::now();
                let time_since_last_batch = now.duration_since(last_batch_time);

                // Try to receive events with a timeout
                let timeout = if event_batch.is_empty() {
                    Duration::from_secs(1)
                } else {
                    // If we have events, check more frequently
                    Duration::from_millis(100)
                };

                match event_receiver.recv_timeout(timeout) {
                    Ok(event) => {
                        event_batch.push(event);
                        let current_batch_size = event_batch.len();

                        if current_batch_size % 1000 == 0 && current_batch_size > 0 {
                            debug!(
                                "Batch size reached {}, will send at 10000 or next timer tick",
                                current_batch_size
                            );
                        }

                        // If batch gets too large, send immediately
                        if current_batch_size >= 10000 {
                            debug!("Batch size limit reached (10000 events), sending immediately");
                            let batch = std::mem::take(&mut event_batch);
                            let count = batch.len();
                            match XatuFFI::send_event_batch(batch) {
                                Ok(()) => {
                                    total_events_processed += count as u64;
                                    total_batches_sent += 1;
                                    debug!(
                                        "Successfully sent batch #{} with {} events (size limit). Total events: {}", 
                                        total_batches_sent, count, total_events_processed
                                    );
                                    crate::metrics::inc_events_sent_batch(count);
                                }
                                Err(e) => {
                                    error!("Failed to send event batch (size limit): {}", e);
                                }
                            }
                            last_batch_time = now;
                        }
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                        // Check if it's time to send what we have
                        if time_since_last_batch >= Duration::from_secs(1)
                            && !event_batch.is_empty()
                            && initialized_for_thread.load(Ordering::Relaxed)
                        {
                            let batch = std::mem::take(&mut event_batch);
                            let count = batch.len();
                            match XatuFFI::send_event_batch(batch) {
                                Ok(()) => {
                                    total_events_processed += count as u64;
                                    total_batches_sent += 1;
                                    debug!(
                                        "Successfully sent batch #{} with {} events (timer). Total events: {}", 
                                        total_batches_sent, count, total_events_processed
                                    );
                                    crate::metrics::inc_events_sent_batch(count);
                                }
                                Err(e) => {
                                    error!("Failed to send event batch (timer): {}", e);
                                }
                            }
                            last_batch_time = now;
                        }
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                        warn!("Event channel disconnected, stopping batch processor");
                        break;
                    }
                }
            }
        });

        // Wait for initialization result
        match init_receiver.recv() {
            Ok(Ok(())) => {
                info!("Xatu FFI initialization completed successfully");
            }
            Ok(Err(e)) => {
                return Err(format!("Failed to initialize Xatu FFI: {}", e).into());
            }
            Err(_) => {
                return Err("FFI thread failed to send initialization result".into());
            }
        }

        // event_sender was already created above, no need to create it again

        Ok(Self {
            initialized,
            network_info,
            event_sender: Some(event_sender),
        })
    }

    pub fn with_network_info(mut self, network_info: crate::config::NetworkInfo) -> Self {
        self.network_info = Some(network_info);
        self
    }
}

impl crate::observer_trait::XatuObserverTrait for XatuObserver {
    fn on_gossip_block<E: EthSpec>(
        &self,
        message_id: MessageId,
        peer_id: PeerId,
        _client: Option<String>,
        block: Arc<SignedBeaconBlock<E>>,
        timestamp_millis: u64,
        topic: String,
        message_size: usize,
    ) -> ObserverResult {
        let slot = block.slot();
        let signed_block_header = block.signed_block_header();
        let block_root = signed_block_header.message.canonical_root();
        debug!(
            "Xatu FFI: Received gossip block - slot: {}, root: 0x{}, message_id: {:?}",
            slot,
            hex::encode(&block_root.0[..8]),
            message_id
        );

        if !self.initialized.load(Ordering::Relaxed) {
            warn!(
                "Xatu FFI: Not initialized yet, skipping block at slot {}",
                slot
            );
            return ObserverResult::Ok;
        }

        let proposer_index = block.message().proposer_index();
        let slot_u64 = slot.as_u64();

        // Get network info for calculations
        let network_info = match self.network_info.as_ref() {
            Some(info) => info,
            None => {
                error!("Xatu FFI: Network info not available, cannot calculate timestamps");
                return ObserverResult::Error("Network info not available".to_string());
            }
        };

        // Calculate epoch using network-specific slots per epoch
        let epoch = slot_u64 / network_info.slots_per_epoch;

        let event = EventData::BeaconBlock {
            peer_id: peer_id.to_string(),
            message_id: hex::encode(&message_id.0),
            topic,
            message_size: message_size as u32,
            timestamp_ms: timestamp_millis as i64,
            slot: slot_u64,
            epoch,
            block_root: format!("0x{}", hex::encode(block_root.0)),
            proposer_index,
        };

        debug!(
            "Xatu FFI: Processing block event - slot: {}, peer: {}",
            slot, peer_id
        );

        if let Some(sender) = &self.event_sender {
            match sender.send(event) {
                Ok(()) => {
                    debug!(
                        "Queued beacon block event for slot {} from peer {}",
                        slot, peer_id
                    );
                }
                Err(e) => {
                    error!(
                        "Failed to queue beacon block event for slot {}: {:?}",
                        slot, e
                    );
                }
            }
        }

        ObserverResult::Ok
    }

    fn on_gossip_attestation<E: EthSpec>(
        &self,
        message_id: MessageId,
        peer_id: PeerId,
        attestation: Arc<SingleAttestation>,
        subnet_id: SubnetId,
        should_process: bool,
        timestamp_millis: u64,
        topic: String,
        message_size: usize,
    ) -> ObserverResult {
        let beacon_block_root = attestation.data.beacon_block_root;
        debug!(
            "Xatu FFI: Received gossip attestation - subnet: {}, beacon_block_root: 0x{}, message_id: {:?}",
            *subnet_id,
            hex::encode(&beacon_block_root.0[..8]),
            message_id
        );

        if !self.initialized.load(Ordering::Relaxed) {
            warn!("Xatu FFI: Not initialized yet, skipping attestation");
            return ObserverResult::Ok;
        }

        let slot = attestation.data.slot;
        let slot_u64 = slot.as_u64();

        // Get network info for epoch calculation
        let network_info = match self.network_info.as_ref() {
            Some(info) => info,
            None => {
                error!("Xatu FFI: Network info not available");
                return ObserverResult::Error("Network info not available".to_string());
            }
        };

        let epoch = slot_u64 / network_info.slots_per_epoch;

        let event = EventData::Attestation {
            peer_id: peer_id.to_string(),
            slot: slot_u64,
            epoch,
            attestation_data_root: format!("0x{}", hex::encode(beacon_block_root.0)),
            subnet_id: u64::from(subnet_id),
            timestamp_ms: timestamp_millis as i64,
            message_id: hex::encode(&message_id.0),
            should_process,
            topic,
            message_size: message_size as u32,
            // Additional attestation data fields
            source_epoch: attestation.data.source.epoch.as_u64(),
            source_root: format!("0x{}", hex::encode(attestation.data.source.root.0)),
            target_epoch: attestation.data.target.epoch.as_u64(),
            target_root: format!("0x{}", hex::encode(attestation.data.target.root.0)),
            committee_index: attestation.committee_index,
            // Aggregation and signature fields
            // For single attestations, we don't have aggregation bits, so we'll use an empty string
            aggregation_bits: String::from("0x"),
            signature: format!("0x{}", hex::encode(attestation.signature.serialize())),
            // Validator specific fields
            attester_index: attestation.attester_index,
        };

        debug!(
            "Xatu FFI: Processing attestation event - slot: {}, subnet: {}, peer: {}",
            slot, *subnet_id, peer_id
        );

        if let Some(sender) = &self.event_sender {
            if let Err(e) = sender.send(event) {
                error!("Failed to queue attestation event: {:?}", e);
            } else {
                debug!(
                    "Queued attestation event for slot {} subnet {}",
                    slot, *subnet_id
                );
            }
        }

        ObserverResult::Ok
    }

    fn on_gossip_aggregate_and_proof<E: EthSpec>(
        &self,
        message_id: MessageId,
        peer_id: PeerId,
        aggregate: Arc<SignedAggregateAndProof<E>>,
        timestamp_millis: u64,
        topic: String,
        message_size: usize,
    ) -> ObserverResult {
        let attestation_data = aggregate.message().aggregate().data();
        let beacon_block_root = attestation_data.beacon_block_root;
        let aggregator_index = aggregate.message().aggregator_index();

        debug!(
            "Xatu FFI: Received gossip aggregate and proof - beacon_block_root: 0x{}, aggregator: {}, message_id: {:?}",
            hex::encode(&beacon_block_root.0[..8]),
            aggregator_index,
            message_id
        );

        if !self.initialized.load(Ordering::Relaxed) {
            warn!("Xatu FFI: Not initialized yet, skipping aggregate and proof");
            return ObserverResult::Ok;
        }

        let slot = attestation_data.slot;
        let slot_u64 = slot.as_u64();

        // Get network info for epoch calculation
        let network_info = match self.network_info.as_ref() {
            Some(info) => info,
            None => {
                error!("Xatu FFI: Network info not available");
                return ObserverResult::Error("Network info not available".to_string());
            }
        };

        let epoch = slot_u64 / network_info.slots_per_epoch;

        let event = EventData::AggregateAndProof {
            peer_id: peer_id.to_string(),
            slot: slot_u64,
            epoch,
            attestation_data_root: format!("0x{}", hex::encode(beacon_block_root.0)),
            aggregator_index,
            timestamp_ms: timestamp_millis as i64,
            message_id: hex::encode(&message_id.0),
            topic,
            message_size: message_size as u32,
            // Additional attestation data fields
            source_epoch: attestation_data.source.epoch.as_u64(),
            source_root: format!("0x{}", hex::encode(attestation_data.source.root.0)),
            target_epoch: attestation_data.target.epoch.as_u64(),
            target_root: format!("0x{}", hex::encode(attestation_data.target.root.0)),
            // For Electra, get committee index from committee_bits; for pre-Electra use data.index
            committee_index: aggregate
                .message()
                .aggregate()
                .committee_index()
                .unwrap_or(attestation_data.index),
            // Aggregation and signature fields
            aggregation_bits: match aggregate.message().aggregate() {
                types::AttestationRef::Base(att) => {
                    format!("0x{}", hex::encode(att.aggregation_bits.as_slice()))
                }
                types::AttestationRef::Electra(att) => {
                    format!("0x{}", hex::encode(att.aggregation_bits.as_slice()))
                }
            },
            signature: format!("0x{}", hex::encode(aggregate.signature().serialize())),
        };

        debug!(
            "Xatu FFI: Processing aggregate and proof event - slot: {}, aggregator: {}, peer: {}",
            slot, aggregator_index, peer_id
        );

        if let Some(sender) = &self.event_sender {
            if let Err(e) = sender.send(event) {
                error!("Failed to queue aggregate and proof event: {:?}", e);
            } else {
                debug!("Queued aggregate and proof event for slot {}", slot);
            }
        }

        ObserverResult::Ok
    }

    fn on_gossip_blob_sidecar<E: EthSpec>(
        &self,
        message_id: MessageId,
        peer_id: PeerId,
        client: Option<String>,
        blob_index: u64,
        blob_sidecar: Arc<BlobSidecar<E>>,
        timestamp_millis: u64,
        topic: String,
        message_size: usize,
    ) -> ObserverResult {
        let block_root = blob_sidecar.block_root();
        let slot = blob_sidecar.slot();

        debug!(
            "Xatu FFI: Received gossip blob sidecar - slot: {}, index: {}, root: 0x{}, message_id: {:?}",
            slot,
            blob_index,
            hex::encode(&block_root.0[..8]),
            message_id
        );

        if !self.initialized.load(Ordering::Relaxed) {
            warn!("Xatu FFI: Not initialized yet, skipping blob sidecar");
            return ObserverResult::Ok;
        }

        let slot_u64 = slot.as_u64();

        // Get network info for epoch calculation
        let network_info = match self.network_info.as_ref() {
            Some(info) => info,
            None => {
                error!("Xatu FFI: Network info not available");
                return ObserverResult::Error("Network info not available".to_string());
            }
        };

        let epoch = slot_u64 / network_info.slots_per_epoch;

        let event = EventData::BlobSidecar {
            peer_id: peer_id.to_string(),
            slot: slot_u64,
            epoch,
            block_root: format!("0x{}", hex::encode(block_root.0)),
            parent_root: format!(
                "0x{}",
                hex::encode(blob_sidecar.signed_block_header.message.parent_root.0)
            ),
            state_root: format!(
                "0x{}",
                hex::encode(blob_sidecar.signed_block_header.message.state_root.0)
            ),
            proposer_index: blob_sidecar.block_proposer_index(),
            blob_index,
            timestamp_ms: timestamp_millis as i64,
            message_id: hex::encode(&message_id.0),
            client,
            topic,
            message_size: message_size as u32,
        };

        debug!(
            "Xatu FFI: Processing blob sidecar event - slot: {}, index: {}, peer: {}",
            slot, blob_index, peer_id
        );

        if let Some(sender) = &self.event_sender {
            if let Err(e) = sender.send(event) {
                error!("Failed to queue blob sidecar event: {:?}", e);
            } else {
                debug!(
                    "Queued blob sidecar event for slot {} index {}",
                    slot, blob_index
                );
            }
        }

        ObserverResult::Ok
    }

    fn on_gossip_data_column_sidecar<E: EthSpec>(
        &self,
        message_id: MessageId,
        peer_id: PeerId,
        client: Option<String>,
        _subnet_id: DataColumnSubnetId,
        column_sidecar: Arc<DataColumnSidecar<E>>,
        timestamp_millis: u64,
        topic: String,
        message_size: usize,
    ) -> ObserverResult {
        let block_root = column_sidecar.block_root();
        let slot = column_sidecar.slot();
        let column_index = column_sidecar.index;
        let kzg_commitments_count = column_sidecar.kzg_commitments.len() as u32;

        debug!(
            "Xatu FFI: Received gossip data column sidecar - slot: {}, column_index: {}, root: 0x{}, message_id: {:?}",
            slot,
            column_index,
            hex::encode(&block_root.0[..8]),
            message_id
        );

        if !self.initialized.load(Ordering::Relaxed) {
            warn!("Xatu FFI: Not initialized yet, skipping data column sidecar");
            return ObserverResult::Ok;
        }

        let slot_u64 = slot.as_u64();

        // Get network info for epoch calculation
        let network_info = match self.network_info.as_ref() {
            Some(info) => info,
            None => {
                error!("Xatu FFI: Network info not available");
                return ObserverResult::Error("Network info not available".to_string());
            }
        };

        let epoch = slot_u64 / network_info.slots_per_epoch;

        let event = EventData::DataColumnSidecar {
            peer_id: peer_id.to_string(),
            slot: slot_u64,
            epoch,
            block_root: format!("0x{}", hex::encode(block_root.0)),
            parent_root: format!(
                "0x{}",
                hex::encode(column_sidecar.signed_block_header.message.parent_root.0)
            ),
            state_root: format!(
                "0x{}",
                hex::encode(column_sidecar.signed_block_header.message.state_root.0)
            ),
            proposer_index: column_sidecar.block_proposer_index(),
            column_index,
            kzg_commitments_count,
            timestamp_ms: timestamp_millis as i64,
            message_id: hex::encode(&message_id.0),
            client,
            topic,
            message_size: message_size as u32,
        };

        debug!(
            "Xatu FFI: Processing data column sidecar event - slot: {}, column_index: {}, peer: {}",
            slot, column_index, peer_id
        );

        if let Some(sender) = &self.event_sender {
            if let Err(e) = sender.send(event) {
                error!("Failed to queue data column sidecar event: {:?}", e);
            } else {
                debug!(
                    "Queued data column sidecar event for slot {} column_index {}",
                    slot, column_index
                );
            }
        }

        ObserverResult::Ok
    }
}

impl<E: EthSpec> crate::Xatu<E> for XatuObserver {
    fn on_gossip_block(
        &self,
        message_id: MessageId,
        peer_id: PeerId,
        client: Option<String>,
        block: Arc<SignedBeaconBlock<E>>,
        timestamp_millis: u64,
        topic: String,
        message_size: usize,
    ) {
        let _ = <Self as crate::observer_trait::XatuObserverTrait>::on_gossip_block::<E>(
            self,
            message_id,
            peer_id,
            client,
            block,
            timestamp_millis,
            topic,
            message_size,
        );
    }

    fn on_gossip_attestation(
        &self,
        message_id: MessageId,
        peer_id: PeerId,
        attestation: Arc<SingleAttestation>,
        subnet_id: SubnetId,
        should_process: bool,
        timestamp_millis: u64,
        topic: String,
        message_size: usize,
    ) {
        let _ = <Self as crate::observer_trait::XatuObserverTrait>::on_gossip_attestation::<E>(
            self,
            message_id,
            peer_id,
            attestation,
            subnet_id,
            should_process,
            timestamp_millis,
            topic,
            message_size,
        );
    }

    fn on_gossip_aggregate_and_proof(
        &self,
        message_id: MessageId,
        peer_id: PeerId,
        aggregate: Arc<SignedAggregateAndProof<E>>,
        timestamp_millis: u64,
        topic: String,
        message_size: usize,
    ) {
        let _ =
            <Self as crate::observer_trait::XatuObserverTrait>::on_gossip_aggregate_and_proof::<E>(
                self,
                message_id,
                peer_id,
                aggregate,
                timestamp_millis,
                topic,
                message_size,
            );
    }

    fn on_gossip_blob_sidecar(
        &self,
        message_id: MessageId,
        peer_id: PeerId,
        client: Option<String>,
        blob_index: u64,
        blob_sidecar: Arc<BlobSidecar<E>>,
        timestamp_millis: u64,
        topic: String,
        message_size: usize,
    ) {
        let _ = <Self as crate::observer_trait::XatuObserverTrait>::on_gossip_blob_sidecar::<E>(
            self,
            message_id,
            peer_id,
            client,
            blob_index,
            blob_sidecar,
            timestamp_millis,
            topic,
            message_size,
        );
    }

    fn on_gossip_data_column_sidecar(
        &self,
        message_id: MessageId,
        peer_id: PeerId,
        client: Option<String>,
        subnet_id: DataColumnSubnetId,
        column_sidecar: Arc<DataColumnSidecar<E>>,
        timestamp_millis: u64,
        topic: String,
        message_size: usize,
    ) {
        let _ =
            <Self as crate::observer_trait::XatuObserverTrait>::on_gossip_data_column_sidecar::<E>(
                self,
                message_id,
                peer_id,
                client,
                subnet_id,
                column_sidecar,
                timestamp_millis,
                topic,
                message_size,
            );
    }
}

impl Drop for XatuObserver {
    fn drop(&mut self) {
        if self.initialized.load(Ordering::Relaxed) {
            info!("Xatu FFI: Closing forwarder");
            XatuFFI::close();
        }
    }
}
