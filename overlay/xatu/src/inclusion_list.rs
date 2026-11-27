//! EIP-7805 (FOCIL) inclusion list gossip capture.

use crate::emitter;
use crate::ffi::EventData;
use lighthouse_network::{MessageId, PeerId};
use std::time::Duration;
use types::SignedInclusionList;

/// Records a signed inclusion list received via gossip.
pub fn on_gossip_inclusion_list(
    message_id: &MessageId,
    peer_id: &PeerId,
    inclusion_list: &SignedInclusionList,
    timestamp: Duration,
    topic: String,
    message_size: usize,
) {
    let Some(observer) = emitter::active_observer() else {
        return;
    };

    let message = &inclusion_list.message;

    observer.send(EventData::InclusionList {
        peer_id: peer_id.to_string(),
        slot: message.slot.as_u64(),
        validator_index: message.validator_index,
        dependent_root: format!("0x{}", hex::encode(message.dependent_root.0)),
        transactions: message
            .transactions
            .iter()
            .map(|transaction| format!("0x{}", hex::encode(&transaction[..])))
            .collect(),
        timestamp_ms: i64::try_from(timestamp.as_millis()).unwrap_or(i64::MAX),
        message_id: hex::encode(&message_id.0),
        topic,
        message_size: u32::try_from(message_size).unwrap_or(u32::MAX),
    });
}
