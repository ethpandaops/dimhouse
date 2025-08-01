pub use metrics::*;
use std::sync::LazyLock;

// Xatu event counter
pub static XATU_EVENTS_SENT: LazyLock<Result<IntCounterVec>> = LazyLock::new(|| {
    try_create_int_counter_vec(
        "xatu_events_sent_total",
        "Total number of events sent to Xatu sink",
        &["event_type"],
    )
});

// Helper function to increment counter for batch
pub fn inc_events_sent_batch(count: usize) {
    if let Some(counter) = XATU_EVENTS_SENT.as_ref().ok() {
        counter.with_label_values(&["batch"]).inc_by(count as u64);
    }
}
