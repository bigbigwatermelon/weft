//! In-memory thread-bus state: per-thread inboxes (keyed by direction), a shared
//! JSON state blob, the message timeline, and the set of known member directions.
//! Identity is always supplied by the caller (the HTTP handler derives it from
//! the URL path), never trusted from agent input.

use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Msg {
    pub from: String,
    pub to: String, // "*" for broadcast
    pub text: String,
    pub ts: u64,
    pub kind: String, // "message" | "interface"
}

#[derive(Default)]
struct ThreadBus {
    inboxes: HashMap<String, Vec<Msg>>, // dir -> unread
    log: Vec<Msg>,                      // full timeline (for the UI later)
    state: serde_json::Value,           // shared thread_state blob (object)
    members: HashSet<String>,           // dirs that have connected
}

/// Cloneable handle to all threads' buses.
#[derive(Default, Clone)]
pub struct BusRegistry {
    inner: Arc<Mutex<HashMap<i32, ThreadBus>>>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl BusRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `dir` as a member of `thread` (idempotent). Called on connect.
    pub fn join(&self, thread: i32, dir: &str) {
        let mut g = self.inner.lock().unwrap();
        let bus = g.entry(thread).or_default();
        bus.members.insert(dir.to_string());
        if !bus.state.is_object() {
            bus.state = serde_json::json!({});
        }
    }

    /// Post a message from `from` to a specific `to` direction.
    pub fn post(&self, thread: i32, from: &str, to: &str, text: &str, kind: &str) {
        let m = Msg {
            from: from.to_string(),
            to: to.to_string(),
            text: text.to_string(),
            ts: now(),
            kind: kind.to_string(),
        };
        let mut g = self.inner.lock().unwrap();
        let bus = g.entry(thread).or_default();
        bus.log.push(m.clone());
        bus.inboxes.entry(to.to_string()).or_default().push(m);
    }

    /// Broadcast from `from` to every other member of the thread.
    pub fn broadcast(&self, thread: i32, from: &str, text: &str, kind: &str) {
        let mut g = self.inner.lock().unwrap();
        let bus = g.entry(thread).or_default();
        let targets: Vec<String> = bus
            .members
            .iter()
            .filter(|d| d.as_str() != from)
            .cloned()
            .collect();
        let m = Msg {
            from: from.to_string(),
            to: "*".to_string(),
            text: text.to_string(),
            ts: now(),
            kind: kind.to_string(),
        };
        bus.log.push(m.clone());
        for d in targets {
            bus.inboxes.entry(d).or_default().push(m.clone());
        }
    }

    /// Read and clear `me`'s unread messages.
    pub fn inbox(&self, thread: i32, me: &str) -> Vec<Msg> {
        let mut g = self.inner.lock().unwrap();
        let bus = g.entry(thread).or_default();
        bus.inboxes.remove(me).unwrap_or_default()
    }

    pub fn state_get(&self, thread: i32) -> serde_json::Value {
        let mut g = self.inner.lock().unwrap();
        let bus = g.entry(thread).or_default();
        if bus.state.is_object() {
            bus.state.clone()
        } else {
            serde_json::json!({})
        }
    }

    /// Shallow-merge `patch` (object) into the shared state.
    pub fn state_set(&self, thread: i32, patch: serde_json::Value) {
        let mut g = self.inner.lock().unwrap();
        let bus = g.entry(thread).or_default();
        if !bus.state.is_object() {
            bus.state = serde_json::json!({});
        }
        if let (Some(dst), Some(src)) = (bus.state.as_object_mut(), patch.as_object()) {
            for (k, v) in src {
                dst.insert(k.clone(), v.clone());
            }
        }
    }

    /// The full timeline for a thread (for the UI in v1b).
    pub fn log(&self, thread: i32) -> Vec<Msg> {
        let mut g = self.inner.lock().unwrap();
        g.entry(thread).or_default().log.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn post_and_inbox_clears() {
        let r = BusRegistry::new();
        r.join(1, "10");
        r.join(1, "20");
        r.post(1, "10", "20", "hi", "message");
        let got = r.inbox(1, "20");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].from, "10");
        assert_eq!(got[0].text, "hi");
        // cleared after read
        assert_eq!(r.inbox(1, "20").len(), 0);
        // other dir unaffected
        assert_eq!(r.inbox(1, "10").len(), 0);
    }

    #[test]
    fn broadcast_reaches_others_not_self() {
        let r = BusRegistry::new();
        for d in ["10", "20", "30"] {
            r.join(1, d);
        }
        r.broadcast(1, "10", "all hands", "message");
        assert_eq!(r.inbox(1, "10").len(), 0);
        assert_eq!(r.inbox(1, "20").len(), 1);
        assert_eq!(r.inbox(1, "30").len(), 1);
    }

    #[test]
    fn state_merges() {
        let r = BusRegistry::new();
        r.state_set(1, serde_json::json!({"a": 1}));
        r.state_set(1, serde_json::json!({"b": 2}));
        assert_eq!(r.state_get(1), serde_json::json!({"a": 1, "b": 2}));
    }

    #[test]
    fn threads_isolated() {
        let r = BusRegistry::new();
        r.join(1, "10");
        r.join(2, "10");
        r.post(1, "x", "10", "t1", "message");
        assert_eq!(r.inbox(2, "10").len(), 0);
        assert_eq!(r.inbox(1, "10").len(), 1);
    }
}
