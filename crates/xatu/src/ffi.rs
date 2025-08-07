use serde::{Deserialize, Serialize};
use std::ffi::CString;
use std::os::raw::{c_char, c_int};
use std::sync::Mutex;
use tracing::debug;

// Global mutex to ensure thread-safe FFI calls
static FFI_MUTEX: Mutex<()> = Mutex::new(());

#[link(name = "xatu")]
extern "C" {
    fn Init(config_json: *const c_char) -> c_int;
    fn SendEventBatch(events_json: *const c_char) -> c_int;
    fn Shutdown();
}

// Removed thread ID tracking - not needed

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "event_type")]
pub enum EventData {
    #[serde(rename = "BEACON_BLOCK")]
    BeaconBlock {
        peer_id: String,
        message_id: String,
        topic: String,
        message_size: u32,
        timestamp_ms: i64,
        slot: u64,
        epoch: u64,
        block_root: String,
        proposer_index: u64,
    },
    #[serde(rename = "ATTESTATION")]
    Attestation {
        peer_id: String,
        slot: u64,
        epoch: u64,
        attestation_data_root: String,
        subnet_id: u64,
        timestamp: i64,
        message_id: String,
        should_process: bool,
        topic: String,
        message_size: u32,
        // Additional attestation data fields
        source_epoch: u64,
        source_root: String,
        target_epoch: u64,
        target_root: String,
        committee_index: u64,
        // Aggregation and signature fields
        aggregation_bits: String,
        signature: String,
        // Validator specific fields
        attester_index: u64,
    },
    #[serde(rename = "AGGREGATE_AND_PROOF")]
    AggregateAndProof {
        peer_id: String,
        slot: u64,
        epoch: u64,
        attestation_data_root: String,
        aggregator_index: u64,
        timestamp: i64,
        message_id: String,
        topic: String,
        message_size: u32,
        // Additional attestation data fields
        source_epoch: u64,
        source_root: String,
        target_epoch: u64,
        target_root: String,
        committee_index: u64,
        // Aggregation and signature fields
        aggregation_bits: String, // Hex-encoded aggregation bits
        signature: String,        // Hex-encoded signature
    },
    #[serde(rename = "BLOB_SIDECAR")]
    BlobSidecar {
        peer_id: String,
        slot: u64,
        epoch: u64,
        block_root: String,
        parent_root: String,
        state_root: String,
        proposer_index: u64,
        blob_index: u64,
        timestamp_ms: i64,
        message_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        client: Option<String>,
        topic: String,
        message_size: u32,
    },
    #[serde(rename = "DATA_COLUMN_SIDECAR")]
    DataColumnSidecar {
        peer_id: String,
        slot: u64,
        epoch: u64,
        block_root: String,
        parent_root: String,
        state_root: String,
        proposer_index: u64,
        column_index: u64,
        kzg_commitments_count: u32,
        timestamp_ms: i64,
        message_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        client: Option<String>,
        topic: String,
        message_size: u32,
    },
}

pub struct XatuFFI;

impl XatuFFI {
    pub fn init_with_runtime(config: &crate::config::FullConfigWithRuntime) -> Result<(), String> {
        let config_yaml = serde_yaml::to_string(config)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;

        // Lock mutex to ensure thread-safe FFI call
        let _guard = FFI_MUTEX
            .lock()
            .map_err(|e| format!("Failed to lock mutex: {}", e))?;

        let c_config =
            CString::new(config_yaml).map_err(|e| format!("Failed to create CString: {}", e))?;

        unsafe {
            let result = Init(c_config.as_ptr());
            match result {
                0 => Ok(()),
                -1 => Err("Failed to parse configuration".to_string()),
                -2 => Err("Failed to create sink".to_string()),
                -3 => Err("Failed to start sink".to_string()),
                -4 => Err("Network info not provided".to_string()),
                _ => Err(format!("Failed to initialize: error code {}", result)),
            }
        }
    }

    pub fn send_event_batch(events: Vec<EventData>) -> Result<(), String> {
        if events.is_empty() {
            return Ok(());
        }

        // Thread verification removed - not needed for this issue

        let event_count = events.len();
        // Serialize outside of unsafe block
        let json_data = serde_json::to_string(&events)
            .map_err(|e| format!("Failed to serialize events: {}", e))?;

        // Lock mutex to ensure thread-safe FFI call
        let _guard = FFI_MUTEX
            .lock()
            .map_err(|e| format!("Failed to lock mutex: {}", e))?;

        // Create CString and keep it alive for the FFI call
        let c_json =
            CString::new(json_data).map_err(|e| format!("Failed to create CString: {}", e))?;

        unsafe {
            let result = SendEventBatch(c_json.as_ptr());
            match result {
                0 => {
                    debug!("Successfully sent batch of {} events", event_count);
                    Ok(())
                }
                -1 => Err("Forwarder not initialized".to_string()),
                -2 => Err("Failed to parse event data".to_string()),
                -3 => Err("Failed to send event".to_string()),
                -4 => Err("Server returned error".to_string()),
                _ => Err(format!("Unknown error code: {}", result)),
            }
        }
    }

    pub fn close() {
        unsafe {
            Shutdown();
        }
    }
}

impl Drop for XatuFFI {
    fn drop(&mut self) {
        Self::close();
    }
}
