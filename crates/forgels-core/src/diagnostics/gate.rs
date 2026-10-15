use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Trailing debounce for diagnostic publishes.
///
/// Each change refreshes the stamp, so fast typing yields one publish with
/// the latest version after the quiet period. Time is injected, so tests
/// stay deterministic without sleeps. Flushing to real
/// `textDocument/publishDiagnostics` connects when analysis lands.
#[derive(Debug)]
pub struct PublishGate {
    delay: Duration,
    pending: HashMap<String, (u64, Instant)>,
}

impl PublishGate {
    pub fn new(delay: Duration) -> Self {
        Self {
            delay,
            pending: HashMap::new(),
        }
    }

    /// Record a change. Latest version wins, stamp refreshes.
    pub fn mark_dirty(&mut self, uri: &str, version: u64, now: Instant) {
        self.pending.insert(uri.to_string(), (version, now));
    }

    /// Drain URIs quiet for at least the delay, sorted for determinism.
    pub fn take_due(&mut self, now: Instant) -> Vec<(String, u64)> {
        let mut due: Vec<(String, u64)> = Vec::new();
        self.pending.retain(|uri, (version, stamped)| {
            if now.duration_since(*stamped) >= self.delay {
                due.push((uri.clone(), *version));
                false
            } else {
                true
            }
        });
        due.sort();
        due
    }

    /// Drain every pending URI, due or not, sorted for determinism.
    pub fn take_all(&mut self) -> Vec<(String, u64)> {
        let mut all: Vec<(String, u64)> = self
            .pending
            .iter()
            .map(|(uri, (version, _))| (uri.clone(), *version))
            .collect();
        self.pending.clear();
        all.sort();
        all
    }
}
