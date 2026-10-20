//! Process-wide access to the xatu observer, for hooks that have no `XatuChain` handle.

use crate::observer_ffi::XatuObserver;
use lighthouse_network::types::SyncState;
use std::sync::{Arc, OnceLock};
use tracing::warn;

type EventGate = Box<dyn Fn() -> bool + Send + Sync>;

static OBSERVER: OnceLock<Arc<XatuObserver>> = OnceLock::new();
static EVENT_GATE: OnceLock<EventGate> = OnceLock::new();

pub(crate) fn register_observer(observer: Arc<XatuObserver>) {
    if OBSERVER.set(observer).is_err() {
        warn!("Xatu observer already registered, keeping the first one");
    }
}

/// Returns the registered observer while the event gate is open.
#[cfg_attr(not(xatu_partial_columns), allow(dead_code))]
pub(crate) fn active_observer() -> Option<&'static XatuObserver> {
    let observer = OBSERVER.get()?;
    events_enabled().then_some(observer.as_ref())
}

/// Registers the predicate that decides whether events are exported. Only the first gate is kept.
pub fn set_event_gate<F>(gate: F)
where
    F: Fn() -> bool + Send + Sync + 'static,
{
    if EVENT_GATE.set(Box::new(gate)).is_err() {
        warn!("Xatu event gate already registered, keeping the first one");
    }
}

/// Whether events may be exported right now. Open until a gate is registered.
pub fn events_enabled() -> bool {
    EVENT_GATE.get().is_none_or(|gate| gate())
}

/// Whether a node in `sync_state` exports gossip events: always once the head is synced, and
/// during finalized sync only when optimistic finalized sync is enabled.
pub fn sync_state_allows_events(sync_state: &SyncState, optimistic_finalized_sync: bool) -> bool {
    match sync_state {
        SyncState::Synced
        | SyncState::BackFillSyncing { .. }
        | SyncState::CustodyBackFillSyncing { .. }
        | SyncState::SyncingHead { .. } => true,
        SyncState::SyncingFinalized { .. } | SyncState::SyncTransition => optimistic_finalized_sync,
        SyncState::Stalled => false,
    }
}
