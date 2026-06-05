//! In-memory thread-bus state: per-thread inboxes (keyed by direction), a shared
//! JSON state blob, the message timeline, and the set of known member directions.
//! Identity is always supplied by the caller (the HTTP handler derives it from
//! the URL path), never trusted from agent input.

use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Emitted when a direction should be woken to read its inbox.
#[derive(Clone, Debug)]
pub struct Wake {
    pub thread: i32,
    pub dir: String,
}

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
    wake: Arc<Mutex<Option<Sender<Wake>>>>,
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

    /// Install the channel the coordinator listens on (called once at startup).
    pub fn set_wake_sender(&self, tx: Sender<Wake>) {
        *self.wake.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);
    }

    fn emit_wake(&self, thread: i32, dir: &str) {
        if let Some(tx) = self.wake.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            let _ = tx.send(Wake { thread, dir: dir.to_string() });
        }
    }

    /// Register `dir` as a member of `thread` (idempotent). Called on connect.
    pub fn join(&self, thread: i32, dir: &str) {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
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
        {
            let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let bus = g.entry(thread).or_default();
            bus.log.push(m.clone());
            bus.inboxes.entry(to.to_string()).or_default().push(m);
        }
        self.emit_wake(thread, to);
    }

    /// Broadcast from `from` to every other member of the thread.
    pub fn broadcast(&self, thread: i32, from: &str, text: &str, kind: &str) {
        let m = Msg {
            from: from.to_string(),
            to: "*".to_string(),
            text: text.to_string(),
            ts: now(),
            kind: kind.to_string(),
        };
        let targets: Vec<String> = {
            let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let bus = g.entry(thread).or_default();
            let targets: Vec<String> = bus
                .members
                .iter()
                .filter(|d| d.as_str() != from)
                .cloned()
                .collect();
            bus.log.push(m.clone());
            for d in &targets {
                bus.inboxes.entry(d.clone()).or_default().push(m.clone());
            }
            targets
        };
        for d in targets {
            self.emit_wake(thread, &d);
        }
    }

    /// Read and clear `me`'s unread messages.
    pub fn inbox(&self, thread: i32, me: &str) -> Vec<Msg> {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let bus = g.entry(thread).or_default();
        bus.inboxes.remove(me).unwrap_or_default()
    }

    pub fn state_get(&self, thread: i32) -> serde_json::Value {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let bus = g.entry(thread).or_default();
        if bus.state.is_object() {
            bus.state.clone()
        } else {
            serde_json::json!({})
        }
    }

    /// Shallow-merge `patch` (object) into the shared state.
    pub fn state_set(&self, thread: i32, patch: serde_json::Value) {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
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
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
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
    fn post_emits_wake() {
        let (tx, rx) = std::sync::mpsc::channel();
        let r = BusRegistry::new();
        r.set_wake_sender(tx);
        r.join(1, "10");
        r.post(1, "20", "10", "hi", "message");
        let w = rx.recv_timeout(std::time::Duration::from_secs(1)).unwrap();
        assert_eq!(w.thread, 1);
        assert_eq!(w.dir, "10");
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
