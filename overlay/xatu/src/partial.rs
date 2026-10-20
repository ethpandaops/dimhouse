//! Gossipsub partial data column capture: received and published parts, validation verdicts for
//! received parts, and custody column completions by whatever path.

use crate::emitter;
use crate::ffi::{
    DataColumnCompletion, EventData, PartialDataColumnPart, PartialDataColumnVerdict,
};
use lighthouse_network::types::{GossipEncoding, GossipKind};
use lighthouse_network::{GossipTopic, PeerId, PubsubPartialMessage};
use ssz::{Decode, Encode};
use std::collections::{HashMap, VecDeque};
use std::fmt::Debug;
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};
use types::{
    CellBitmap, ColumnIndex, DataColumnSidecar, DataColumnSubnetId, EthSpec, Hash256,
    PartialDataColumn, PartialDataColumnGroupId, PartialDataColumnPartsMetadata,
    PartialDataColumnRef, Slot,
};

const DIRECTION_RECV: &str = "RECV";
const DIRECTION_SEND: &str = "SEND";
const CAPTURE_POINT_APP: &str = "APP";

const STAGE_DECODE: &str = "DECODE";
const STAGE_HEADER: &str = "HEADER";
const STAGE_CELLS: &str = "CELLS";

const OUTCOME_ACCEPT: &str = "ACCEPT";
const OUTCOME_IGNORE: &str = "IGNORE";
const OUTCOME_REJECT: &str = "REJECT";

const GROUP_VERSION_FULU: u8 = 0;
const GROUP_VERSION_GLOAS: u8 = 1;

/// Verification failures Lighthouse penalises as invalid data (low tolerance).
const REJECT_REASONS: &[&str] = &[
    "InvalidVariant",
    "ProposalSignatureInvalid",
    "UnknownValidator",
    "ProposerIndexMismatch",
    "IsNotLaterThanParent",
    "BlockSlotMismatch",
    "InvalidSubnetId",
    "InvalidInclusionProof",
    "InvalidKzgProof",
    "MismatchesCachedColumn",
    "UnexpectedDataColumn",
    "InvalidColumnIndex",
    "MaxBlobsPerBlockExceeded",
    "InconsistentCommitmentsLength",
    "InconsistentProofsLength",
    "NotFinalizedDescendant",
    "HeaderMismatches",
    "HeaderIncorrectRoot",
    "IncorrectSlot",
    "EmptyMessage",
    "InconsistentPresentCount",
    "PartialColumnsDisabled",
];

/// Verification failures raised while checking the block header or bid rather than the cells.
const HEADER_REASONS: &[&str] = &[
    "ParentUnknown",
    "BlockRootUnknown",
    "PubkeyCacheTimeout",
    "BeaconChainError",
    "InvalidVariant",
    "ProposalSignatureInvalid",
    "UnknownValidator",
    "ProposerIndexMismatch",
    "IsNotLaterThanParent",
    "BlockSlotMismatch",
    "InvalidInclusionProof",
    "NotFinalizedDescendant",
    "FutureSlot",
    "PastFinalizedSlot",
    "MaxBlobsPerBlockExceeded",
    "InconsistentCommitmentsLength",
    "MissingHeader",
    "HeaderMismatches",
    "HeaderIncorrectRoot",
    "IncorrectSlot",
];

/// Columns tracked for completion bookkeeping, about 64 blocks of a supernode's custody.
const TRACKED_COLUMNS: usize = 8192;
const MAX_TRACKED_PEERS: usize = 64;
const MAX_REASON_LEN: usize = 128;

/// How a custody data column became complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletedVia {
    FullGossip,
    PartialGossip,
    El,
    Reconstruction,
}

impl CompletedVia {
    fn as_str(self) -> &'static str {
        match self {
            Self::FullGossip => "FULL_GOSSIP",
            Self::PartialGossip => "PARTIAL_GOSSIP",
            Self::El => "EL",
            Self::Reconstruction => "RECONSTRUCTION",
        }
    }
}

/// Records a partial data column part received from `peer_id`. `column` is the decoded message;
/// it is `None` for metadata-only parts and for messages that failed to decode (`decode_error`).
pub fn on_partial_data_column_received<E: EthSpec>(
    peer_id: &PeerId,
    topic: &GossipTopic,
    group_id: &[u8],
    column: Option<&PartialDataColumn<E>>,
    message_size: usize,
    metadata: Option<&[u8]>,
    decode_error: Option<&str>,
) {
    let Some(observer) = emitter::active_observer() else {
        return;
    };

    let mut part = new_part(DIRECTION_RECV, peer_id.to_string(), topic, group_id);
    part.has_message = column.is_some() || decode_error.is_some();
    part.message_size = saturating_u32(message_size);
    if let Some(column) = column {
        fill_column(&mut part, column.to_ref());
    }
    if let Some(bytes) = metadata {
        part.has_metadata = true;
        part.metadata_size = saturating_u32(bytes.len());
        if let Ok(decoded) = PartialDataColumnPartsMetadata::<E>::from_ssz_bytes(bytes) {
            fill_metadata::<E>(&mut part, &decoded.available, &decoded.requests);
        }
    }

    let verdict = decode_error.map(|error| PartialDataColumnVerdict {
        timestamp_ms: part.timestamp_ms,
        peer_id: part.peer_id.clone(),
        topic: part.topic.clone(),
        group_id: part.group_id.clone(),
        block_root: part.block_root.clone(),
        slot: part.slot,
        column_index: part.column_index,
        stage: STAGE_DECODE.to_string(),
        outcome: OUTCOME_REJECT.to_string(),
        reason: decode_reason(error),
        ..Default::default()
    });

    observer.send(EventData::PartialDataColumn(part));
    if let Some(verdict) = verdict {
        observer.send(EventData::PartialDataColumnOutcome(verdict));
    }
}

/// Records the partial data columns the node hands to gossipsub for publishing. Gossipsub decides
/// per peer what to send, so these parts carry no peer and no message size.
pub fn on_partial_data_columns_published<E: EthSpec>(messages: &[PubsubPartialMessage<E>]) {
    let Some(observer) = emitter::active_observer() else {
        return;
    };

    for message in messages {
        let (column, requests, header) = match message {
            PubsubPartialMessage::DataColumnFulu {
                column,
                request_cells,
                header,
            } => (
                PartialDataColumnRef::Fulu(column.as_ref()),
                request_cells,
                Some(header.as_ref()),
            ),
            PubsubPartialMessage::DataColumnGloas {
                column,
                request_cells,
            } => (
                PartialDataColumnRef::Gloas(column.as_ref()),
                request_cells,
                None,
            ),
        };

        let Some(topic) = data_column_topic(*column.index()) else {
            continue;
        };
        let group_id = encode_group_id(column);

        let mut part = new_part(DIRECTION_SEND, String::new(), &topic, &group_id);
        fill_column(&mut part, column);
        part.has_message = part.cells_count > 0;
        if let Some(header) = header {
            part.slot = header.slot().as_u64();
            part.kzg_commitments_count = saturating_u32(header.kzg_commitments.len());
            part.proposer_index = header.signed_block_header.message.proposer_index;
        }

        let available = column.sidecar().cells_present_bitmap();
        let requests = requests.union(available);
        part.has_metadata = true;
        part.metadata_size = saturating_u32(
            PartialDataColumnPartsMetadata::<E> {
                available: available.clone(),
                requests: requests.clone(),
            }
            .ssz_bytes_len(),
        );
        fill_metadata::<E>(&mut part, available, &requests);

        observer.send(EventData::PartialDataColumn(part));
    }
}

/// Records a custody data column becoming complete. Only the first completion of each block root
/// and column index is reported.
pub fn on_data_column_completed<E: EthSpec>(
    column: &DataColumnSidecar<E>,
    via: CompletedVia,
    peer_id: Option<&PeerId>,
) {
    let Some(observer) = emitter::active_observer() else {
        return;
    };

    let block_root = column.block_root();
    let column_index = *column.index();
    let Some(progress) = tracker().complete((block_root, column_index)) else {
        return;
    };

    let cells_total = column.column().len();
    let kzg_commitments_count = match column {
        DataColumnSidecar::Fulu(fulu) => fulu.kzg_commitments.len(),
        DataColumnSidecar::Gloas(_) => cells_total,
    };
    let gossip_cells = progress.gossip_cells.min(cells_total);
    let (cells_from_gossip, cells_from_el, contributing_peers) = match via {
        CompletedVia::FullGossip => (cells_total, 0, 1),
        CompletedVia::PartialGossip | CompletedVia::El => {
            (gossip_cells, cells_total - gossip_cells, progress.peers)
        }
        CompletedVia::Reconstruction => (gossip_cells, 0, progress.peers),
    };

    observer.send(EventData::DataColumnCompleted(DataColumnCompletion {
        timestamp_ms: now_ms(),
        peer_id: peer_id.map(ToString::to_string).unwrap_or_default(),
        topic: data_column_topic(column_index)
            .map(|topic| topic.to_string())
            .unwrap_or_default(),
        block_root: hex_root(block_root),
        slot: column.slot().as_u64(),
        column_index,
        completed_via: via.as_str().to_string(),
        cells_total: saturating_u32(cells_total),
        cells_from_gossip: saturating_u32(cells_from_gossip),
        cells_from_el: saturating_u32(cells_from_el),
        contributing_peers: saturating_u32(contributing_peers),
        kzg_commitments_count: saturating_u32(kzg_commitments_count),
    }));
}

/// A received partial data column awaiting its validation verdict.
#[derive(Debug, Clone)]
pub struct ReceivedPartial {
    peer_id: PeerId,
    topic: GossipTopic,
    group_version: u8,
    block_root: Hash256,
    slot: Option<Slot>,
    column_index: ColumnIndex,
    cells_count: usize,
}

impl ReceivedPartial {
    pub fn new<E: EthSpec>(
        peer_id: PeerId,
        topic: &GossipTopic,
        column: &PartialDataColumn<E>,
    ) -> Self {
        let column = column.to_ref();
        let (group_version, slot) = match column {
            PartialDataColumnRef::Fulu(fulu) => (
                GROUP_VERSION_FULU,
                fulu.sidecar.header.as_ref().map(|header| header.slot()),
            ),
            PartialDataColumnRef::Gloas(gloas) => (GROUP_VERSION_GLOAS, Some(gloas.slot)),
        };

        Self {
            peer_id,
            topic: topic.clone(),
            group_version,
            block_root: *column.block_root(),
            slot,
            column_index: *column.index(),
            cells_count: column.sidecar().cells_present_bitmap().num_set_bits(),
        }
    }

    /// Records a failed verification, classifying `error` by its variant name.
    pub fn record_failure(&self, error: &impl Debug) {
        let Some(observer) = emitter::active_observer() else {
            return;
        };

        let debug = format!("{error:?}");
        let reason = reason_name(&debug);
        let (stage, outcome) = classify_failure(reason);
        observer.send(EventData::PartialDataColumnOutcome(
            self.verdict(self.slot, stage, outcome, reason, 0),
        ));
    }

    /// Records a verified partial that added `new_cells` cells to the node's copy of the column.
    pub fn record_accept(&self, slot: Slot, new_cells: usize) {
        let Some(observer) = emitter::active_observer() else {
            return;
        };

        if new_cells > 0 {
            tracker().add_gossip_cells(
                (self.block_root, self.column_index),
                self.peer_id,
                new_cells,
            );
        }
        observer.send(EventData::PartialDataColumnOutcome(self.verdict(
            Some(slot),
            STAGE_CELLS,
            OUTCOME_ACCEPT,
            "",
            new_cells,
        )));
    }

    fn verdict(
        &self,
        slot: Option<Slot>,
        stage: &str,
        outcome: &str,
        reason: &str,
        new_cells: usize,
    ) -> PartialDataColumnVerdict {
        let mut group_id = vec![self.group_version];
        match (self.group_version, slot) {
            (GROUP_VERSION_GLOAS, Some(slot)) => PartialDataColumnGroupId {
                beacon_block_root: self.block_root,
                slot,
            }
            .ssz_append(&mut group_id),
            _ => group_id.extend_from_slice(self.block_root.as_slice()),
        }

        PartialDataColumnVerdict {
            timestamp_ms: now_ms(),
            peer_id: self.peer_id.to_string(),
            topic: self.topic.to_string(),
            group_id: hex_bytes(&group_id),
            block_root: hex_root(self.block_root),
            slot: slot.map_or(0, |slot| slot.as_u64()),
            column_index: self.column_index,
            stage: stage.to_string(),
            outcome: outcome.to_string(),
            reason: reason.to_string(),
            cells_count: saturating_u32(self.cells_count),
            new_cells_count: saturating_u32(new_cells),
        }
    }
}

/// The gossip topic currently carrying `column_index`, once the network service has registered
/// its fork context.
fn data_column_topic(column_index: ColumnIndex) -> Option<GossipTopic> {
    let fork_context = emitter::fork_context()?;
    let subnet = DataColumnSubnetId::from_column_index(column_index, &fork_context.spec);
    Some(GossipTopic::new(
        GossipKind::DataColumnSidecar(subnet),
        GossipEncoding::default(),
        fork_context.current_fork_digest(),
    ))
}

fn new_part(
    direction: &str,
    peer_id: String,
    topic: &GossipTopic,
    group_id: &[u8],
) -> PartialDataColumnPart {
    let (group_version, block_root, slot) = parse_group_id(group_id);
    PartialDataColumnPart {
        timestamp_ms: now_ms(),
        direction: direction.to_string(),
        capture_point: CAPTURE_POINT_APP.to_string(),
        peer_id,
        topic: topic.to_string(),
        group_id: hex_bytes(group_id),
        group_version: u32::from(group_version),
        block_root: block_root.map(hex_root).unwrap_or_default(),
        slot: slot.map_or(0, |slot| slot.as_u64()),
        column_index: match topic.kind() {
            GossipKind::DataColumnSidecar(subnet) => **subnet,
            _ => 0,
        },
        ..Default::default()
    }
}

fn fill_column<E: EthSpec>(part: &mut PartialDataColumnPart, column: PartialDataColumnRef<'_, E>) {
    part.block_root = hex_root(*column.block_root());
    part.column_index = *column.index();

    let sidecar = column.sidecar();
    let bitmap = sidecar.cells_present_bitmap();
    part.cells_count = saturating_u32(bitmap.num_set_bits());
    part.bitmap_length = saturating_u32(bitmap.len());
    part.cells_bitmap = hex_bytes(&bitmap.as_ssz_bytes());

    match column {
        PartialDataColumnRef::Fulu(fulu) => {
            if let Some(header) = fulu.sidecar.header.as_ref() {
                part.has_header = true;
                part.slot = header.slot().as_u64();
                part.kzg_commitments_count = saturating_u32(header.kzg_commitments.len());
                part.proposer_index = header.signed_block_header.message.proposer_index;
            }
        }
        PartialDataColumnRef::Gloas(gloas) => part.slot = gloas.slot.as_u64(),
    }
}

fn fill_metadata<E: EthSpec>(
    part: &mut PartialDataColumnPart,
    available: &CellBitmap<E>,
    requests: &CellBitmap<E>,
) {
    part.metadata_available_count = saturating_u32(available.num_set_bits());
    part.metadata_requests_count = saturating_u32(requests.num_set_bits());
    part.metadata_available = hex_bytes(&available.as_ssz_bytes());
    part.metadata_requests = hex_bytes(&requests.as_ssz_bytes());
}

/// Splits a partial group ID into its version byte, block root and (gloas only) slot.
fn parse_group_id(group_id: &[u8]) -> (u8, Option<Hash256>, Option<Slot>) {
    let Some((&version, rest)) = group_id.split_first() else {
        return (0, None, None);
    };

    match version {
        GROUP_VERSION_FULU => (version, Hash256::from_ssz_bytes(rest).ok(), None),
        GROUP_VERSION_GLOAS => match PartialDataColumnGroupId::from_ssz_bytes(rest) {
            Ok(group) => (version, Some(group.beacon_block_root), Some(group.slot)),
            Err(_) => (version, None, None),
        },
        _ => (version, None, None),
    }
}

fn encode_group_id<E: EthSpec>(column: PartialDataColumnRef<'_, E>) -> Vec<u8> {
    match column {
        PartialDataColumnRef::Fulu(fulu) => {
            let mut group_id = vec![GROUP_VERSION_FULU];
            group_id.extend_from_slice(fulu.block_root.as_slice());
            group_id
        }
        PartialDataColumnRef::Gloas(gloas) => {
            let mut group_id = vec![GROUP_VERSION_GLOAS];
            PartialDataColumnGroupId {
                beacon_block_root: gloas.block_root,
                slot: gloas.slot,
            }
            .ssz_append(&mut group_id);
            group_id
        }
    }
}

/// Returns the variant name of a verification error's `Debug` output, unwrapping the
/// `GossipDataColumnError` wrapper used by partial verification errors.
fn reason_name(debug: &str) -> &str {
    let inner = debug
        .strip_prefix("GossipDataColumnError(")
        .unwrap_or(debug);
    inner
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .next()
        .unwrap_or(inner)
}

fn classify_failure(reason: &str) -> (&'static str, &'static str) {
    let stage = if HEADER_REASONS.contains(&reason) {
        STAGE_HEADER
    } else {
        STAGE_CELLS
    };
    let outcome = if REJECT_REASONS.contains(&reason) {
        OUTCOME_REJECT
    } else {
        OUTCOME_IGNORE
    };
    (stage, outcome)
}

/// Keeps the error class of a decode failure and drops the decoder's detail payload.
fn decode_reason(error: &str) -> String {
    let reason = error.split(':').next().unwrap_or(error).trim();
    reason.chars().take(MAX_REASON_LEN).collect()
}

#[derive(Debug, Default, Clone)]
struct ColumnProgress {
    gossip_cells: usize,
    peers: Vec<PeerId>,
    completed: bool,
}

/// Per block root and column bookkeeping for completion events, bounded to `TRACKED_COLUMNS`
/// with oldest-first eviction.
#[derive(Debug, Default)]
struct ColumnTracker {
    columns: HashMap<(Hash256, ColumnIndex), ColumnProgress>,
    order: VecDeque<(Hash256, ColumnIndex)>,
}

/// Progress of a column at the moment it completed.
struct CompletedProgress {
    gossip_cells: usize,
    peers: usize,
}

impl ColumnTracker {
    fn entry(&mut self, key: (Hash256, ColumnIndex)) -> &mut ColumnProgress {
        if !self.columns.contains_key(&key) {
            if self.order.len() >= TRACKED_COLUMNS {
                if let Some(oldest) = self.order.pop_front() {
                    self.columns.remove(&oldest);
                }
            }
            self.order.push_back(key);
        }
        self.columns.entry(key).or_default()
    }

    fn add_gossip_cells(&mut self, key: (Hash256, ColumnIndex), peer_id: PeerId, cells: usize) {
        let progress = self.entry(key);
        progress.gossip_cells = progress.gossip_cells.saturating_add(cells);
        if progress.peers.len() < MAX_TRACKED_PEERS && !progress.peers.contains(&peer_id) {
            progress.peers.push(peer_id);
        }
    }

    /// Marks the column complete, returning its progress unless it had already completed.
    fn complete(&mut self, key: (Hash256, ColumnIndex)) -> Option<CompletedProgress> {
        let progress = self.entry(key);
        if progress.completed {
            return None;
        }
        progress.completed = true;
        Some(CompletedProgress {
            gossip_cells: progress.gossip_cells,
            peers: progress.peers.len(),
        })
    }
}

static TRACKER: LazyLock<Mutex<ColumnTracker>> = LazyLock::new(Mutex::default);

fn tracker() -> MutexGuard<'static, ColumnTracker> {
    TRACKER.lock().unwrap_or_else(PoisonError::into_inner)
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

fn saturating_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn hex_bytes(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}

fn hex_root(root: Hash256) -> String {
    hex_bytes(root.as_slice())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reason_name_unwraps_gossip_data_column_errors() {
        assert_eq!(
            reason_name("GossipDataColumnError(InvalidKzgProof { index: 3 })"),
            "InvalidKzgProof"
        );
        assert_eq!(
            reason_name(
                "GossipDataColumnError(ParentUnknown { parent_root: 0x00, slot: Slot(1) })"
            ),
            "ParentUnknown"
        );
        assert_eq!(reason_name("MissingHeader"), "MissingHeader");
        assert_eq!(
            reason_name("HeaderIncorrectRoot { group_id: 0x01, header_hash: 0x02 }"),
            "HeaderIncorrectRoot"
        );
        assert_eq!(
            reason_name("InternalError(UnexpectedBounds)"),
            "InternalError"
        );
    }

    #[test]
    fn classify_failure_follows_lighthouse_penalties() {
        assert_eq!(
            classify_failure("InvalidKzgProof"),
            (STAGE_CELLS, OUTCOME_REJECT)
        );
        assert_eq!(
            classify_failure("ProposalSignatureInvalid"),
            (STAGE_HEADER, OUTCOME_REJECT)
        );
        assert_eq!(
            classify_failure("MissingHeader"),
            (STAGE_HEADER, OUTCOME_IGNORE)
        );
        assert_eq!(
            classify_failure("PriorKnown"),
            (STAGE_CELLS, OUTCOME_IGNORE)
        );
        assert_eq!(
            classify_failure("SomethingNew"),
            (STAGE_CELLS, OUTCOME_IGNORE)
        );
    }

    #[test]
    fn decode_reason_drops_detail() {
        assert_eq!(
            decode_reason("Error decoding sidecar: OffsetOutOfBounds(12)"),
            "Error decoding sidecar"
        );
        assert_eq!(
            decode_reason("Empty partial group id"),
            "Empty partial group id"
        );
    }

    #[test]
    fn parse_group_id_handles_both_versions() {
        let root = Hash256::repeat_byte(7);

        let mut fulu = vec![GROUP_VERSION_FULU];
        fulu.extend_from_slice(root.as_slice());
        assert_eq!(parse_group_id(&fulu), (0, Some(root), None));

        let mut gloas = vec![GROUP_VERSION_GLOAS];
        PartialDataColumnGroupId {
            beacon_block_root: root,
            slot: Slot::new(42),
        }
        .ssz_append(&mut gloas);
        assert_eq!(gloas.len(), 41);
        assert_eq!(parse_group_id(&gloas), (1, Some(root), Some(Slot::new(42))));

        assert_eq!(parse_group_id(&[]), (0, None, None));
        assert_eq!(parse_group_id(&[GROUP_VERSION_FULU, 1, 2]), (0, None, None));
    }

    #[test]
    fn tracker_completes_once_and_counts_gossip_cells() {
        let mut tracker = ColumnTracker::default();
        let key = (Hash256::repeat_byte(1), 5);
        let peer = PeerId::random();

        tracker.add_gossip_cells(key, peer, 2);
        tracker.add_gossip_cells(key, peer, 1);
        tracker.add_gossip_cells(key, PeerId::random(), 1);

        let progress = tracker.complete(key).expect("first completion");
        assert_eq!(progress.gossip_cells, 4);
        assert_eq!(progress.peers, 2);
        assert!(tracker.complete(key).is_none());
    }

    #[test]
    fn tracker_evicts_oldest_columns() {
        let mut tracker = ColumnTracker::default();
        for index in 0..(TRACKED_COLUMNS as u64 + 1) {
            assert!(tracker.complete((Hash256::ZERO, index)).is_some());
        }
        assert_eq!(tracker.columns.len(), TRACKED_COLUMNS);
        assert!(tracker.complete((Hash256::ZERO, 0)).is_some());
    }
}
