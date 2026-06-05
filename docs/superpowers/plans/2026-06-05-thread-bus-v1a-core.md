# Thread Bus v1a (core) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Run a single local HTTP MCP "thread bus" that agents in the same thread register (ephemerally, additively) and use to message each other and share thread state.

**Architecture:** One axum HTTP server in the Tauri app exposes `POST /bus/:thread/:dir/mcp` (stateless MCP-over-HTTP, SSE single-event responses). Identity (`me`) comes from the URL path, never from agent input. A `BusRegistry` (Arc<Mutex<HashMap<thread, ThreadBus>>>) holds per-thread inboxes + a shared state blob + the timeline. At spawn, each tool gets an ephemeral, additive injection pointing at its `/bus/<thread>/<direction>/mcp` URL (claude `--mcp-config`, codex `-c`, opencode merged worktree `opencode.json`).

**Tech Stack:** Rust, Tauri v2, axum 0.7 + tokio, serde_json, reqwest (tests). The spike (`/tmp/weft-bus-spike`) proved the HTTP-MCP chain works across all three CLIs.

---

## Reference: spec
`docs/superpowers/specs/2026-06-05-thread-bus-coordination-design.md`. Read it. The spike verified: Claude `--mcp-config` (additive over repo `.mcp.json`), Codex `-c mcp_servers.weft_bus.url=`, OpenCode project `opencode.json` remote MCP (coexists with global servers).

## Scope
**In (v1a):** bus registry + state, MCP-over-HTTP handler, server startup + base URL, per-tool ephemeral additive injection, pty.rs wiring, live e2e.
**Out (v1b follow-on):** UI coordination panel, coordinator auto-wake, passive `.thread/` + PLAN.md layer, `ask` request/response.

## File structure
```
src-tauri/
  Cargo.toml                 # MODIFY: add axum, tower, reqwest (dev)
  src/lib.rs                 # MODIFY: mod bus; start server; manage BusRegistry
  src/bus/mod.rs             # CREATE: re-exports
  src/bus/state.rs           # CREATE: Msg, ThreadBus, BusRegistry (+ unit tests)
  src/bus/server.rs          # CREATE: axum router + MCP JSON-RPC/SSE handler
  src/bus/inject.rs          # CREATE: per-tool ephemeral additive injection (+ tests)
  src/pty.rs                 # MODIFY: apply injection args at spawn
  tests/bus_http.rs          # CREATE: integration test hitting the MCP handler
```

## Shared types (consistent across tasks)
- Identity key `dir` = the direction id as a string (`direction_id.to_string()`).
- `Msg { from: String, to: String, text: String, ts: u64, kind: String }` (`kind` ∈ `"message" | "interface"`).
- Bus base URL: `http://127.0.0.1:<port>`; per-session MCP URL: `<base>/bus/<thread_id>/<dir>/mcp`.
- MCP server name `weft_bus`; tool names: `bus_post`, `bus_broadcast`, `bus_inbox`, `thread_state_get`, `thread_state_set`, `announce_interface_change`.

---

## Task 1: Bus registry + state (pure, unit-tested)

**Files:** Create `src-tauri/src/bus/state.rs`, `src-tauri/src/bus/mod.rs`; modify `src-tauri/src/lib.rs` (add `mod bus;`).

- [ ] **Step 1: Add the dependencies**

In `src-tauri/Cargo.toml` under `[dependencies]` append:
```toml
axum = "0.7"
tower = "0.5"
```
And under a `[dev-dependencies]` section (create it if absent) append:
```toml
reqwest = { version = "0.12", features = ["json"] }
```

- [ ] **Step 2: Write `src-tauri/src/bus/state.rs`**

```rust
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
```

- [ ] **Step 3: Write `src-tauri/src/bus/mod.rs`**
```rust
pub mod inject;
pub mod server;
pub mod state;

pub use state::{BusRegistry, Msg};
```
(Empty `inject.rs`/`server.rs` are created in later tasks; create them now as placeholders so the module compiles.)

Create `src-tauri/src/bus/server.rs`:
```rust
//! Filled in Task 2.
```
Create `src-tauri/src/bus/inject.rs`:
```rust
//! Filled in Task 4.
```

Add `mod bus;` to `src-tauri/src/lib.rs` (alongside the other `mod` lines; leave `run()` and the mcp_bridge block untouched).

- [ ] **Step 4: Run the unit tests**

Run: `cd /Users/solojiang/workspace/weft/src-tauri && cargo test bus::state 2>&1 | tail -12`
Expected: `post_and_inbox_clears`, `broadcast_reaches_others_not_self`, `state_merges`, `threads_isolated` all `ok`.

- [ ] **Step 5: Commit**
```bash
cd /Users/solojiang/workspace/weft
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/bus src-tauri/src/lib.rs
git commit -m "feat(bus): thread-bus registry + state (post/broadcast/inbox/state)"
```
End every commit message in this plan with the trailer:
`Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`

---

## Task 2: MCP-over-HTTP handler

**Files:** Modify `src-tauri/src/bus/server.rs`; create `src-tauri/tests/bus_http.rs`.

The handler implements the minimal MCP server surface the CLIs use: `initialize`, `notifications/initialized`, `tools/list`, `tools/call`. Responses are a single SSE `event: message` (matching the spike). Identity (`me`) and `thread` come from the path `/bus/:thread/:dir/mcp`.

- [ ] **Step 1: Write `src-tauri/src/bus/server.rs`**

```rust
//! MCP-over-HTTP for the thread bus. Stateless: each POST yields one SSE
//! `event: message` carrying the JSON-RPC response. Identity is the URL path,
//! never agent input.

use crate::bus::BusRegistry;
use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};

pub fn router(reg: BusRegistry) -> Router {
    Router::new()
        .route("/bus/:thread/:dir/mcp", post(handle).get(get_not_allowed))
        .route("/health", get(|| async { "ok" }))
        .with_state(reg)
}

async fn get_not_allowed() -> StatusCode {
    StatusCode::METHOD_NOT_ALLOWED
}

/// One SSE event carrying `value`.
fn sse(value: Value) -> Response {
    let body = format!("event: message\ndata: {}\n\n", value);
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/event-stream")],
        body,
    )
        .into_response()
}

async fn handle(
    Path((thread, dir)): Path<(i32, String)>,
    State(reg): State<BusRegistry>,
    Json(req): Json<Value>,
) -> Response {
    // Notifications (no id) get a bare 202.
    let id = match req.get("id") {
        Some(v) => v.clone(),
        None => return StatusCode::ACCEPTED.into_response(),
    };
    let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
    reg.join(thread, &dir);

    let result: Value = match method {
        "initialize" => json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": "weft_bus", "version": "1.0.0" }
        }),
        "tools/list" => json!({ "tools": tool_specs() }),
        "tools/call" => {
            let name = req
                .pointer("/params/name")
                .and_then(|n| n.as_str())
                .unwrap_or("");
            let args = req
                .pointer("/params/arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            call_tool(&reg, thread, &dir, name, &args)
        }
        _ => json!({}),
    };

    sse(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

fn text_result(s: String) -> Value {
    json!({ "content": [{ "type": "text", "text": s }] })
}

fn call_tool(reg: &BusRegistry, thread: i32, me: &str, name: &str, args: &Value) -> Value {
    let s = |k: &str| args.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    match name {
        "bus_post" => {
            reg.post(thread, me, &s("to"), &s("text"), "message");
            text_result(format!("posted to {}", s("to")))
        }
        "bus_broadcast" => {
            reg.broadcast(thread, me, &s("text"), "message");
            text_result("broadcast sent".into())
        }
        "announce_interface_change" => {
            reg.broadcast(thread, me, &s("summary"), "interface");
            text_result("interface change announced".into())
        }
        "bus_inbox" => {
            let msgs = reg.inbox(thread, me);
            text_result(serde_json::to_string(&msgs).unwrap_or_else(|_| "[]".into()))
        }
        "thread_state_get" => text_result(reg.state_get(thread).to_string()),
        "thread_state_set" => {
            let patch = args.get("patch").cloned().unwrap_or_else(|| json!({}));
            reg.state_set(thread, patch);
            text_result("state updated".into())
        }
        _ => text_result(format!("unknown tool: {name}")),
    }
}

fn tool_specs() -> Value {
    let str_prop = || json!({ "type": "string" });
    json!([
        {
            "name": "bus_post",
            "description": "Post a message to another direction's inbox in this thread.",
            "inputSchema": { "type": "object",
                "properties": { "to": str_prop(), "text": str_prop() },
                "required": ["to", "text"] }
        },
        {
            "name": "bus_broadcast",
            "description": "Send a message to every other direction in this thread.",
            "inputSchema": { "type": "object",
                "properties": { "text": str_prop() }, "required": ["text"] }
        },
        {
            "name": "bus_inbox",
            "description": "Read and clear your unread messages from other directions.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "thread_state_get",
            "description": "Read the shared thread state (a JSON object).",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "thread_state_set",
            "description": "Shallow-merge a patch object into the shared thread state.",
            "inputSchema": { "type": "object",
                "properties": { "patch": { "type": "object" } }, "required": ["patch"] }
        },
        {
            "name": "announce_interface_change",
            "description": "Broadcast a contract/interface change to the other directions.",
            "inputSchema": { "type": "object",
                "properties": { "summary": str_prop() }, "required": ["summary"] }
        }
    ])
}

/// Bind an ephemeral port and serve the router; returns the bound base URL.
pub async fn serve(reg: BusRegistry) -> std::io::Result<(String, tokio::task::JoinHandle<()>)> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let base = format!("http://127.0.0.1:{}", addr.port());
    let app = router(reg);
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Ok((base, handle))
}
```

- [ ] **Step 2: Write the integration test `src-tauri/tests/bus_http.rs`**

```rust
//! Drives the bus HTTP MCP handler exactly like a CLI client would: initialize,
//! tools/list, then two directions exchanging a message.
use weft_app_lib::bus::{server, BusRegistry};

async fn rpc(base: &str, thread: i32, dir: &str, body: serde_json::Value) -> String {
    let url = format!("{base}/bus/{thread}/{dir}/mcp");
    let resp = reqwest::Client::new()
        .post(url)
        .header("Accept", "application/json, text/event-stream")
        .json(&body)
        .send()
        .await
        .unwrap();
    resp.text().await.unwrap()
}

#[tokio::test]
async fn two_directions_exchange_a_message() {
    let reg = BusRegistry::new();
    let (base, _h) = server::serve(reg).await.unwrap();

    // both directions initialize (registers membership)
    for dir in ["10", "20"] {
        let out = rpc(
            &base,
            1,
            dir,
            serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        )
        .await;
        assert!(out.contains("weft_bus"), "initialize must return serverInfo");
    }

    // tools/list exposes bus_post
    let tl = rpc(
        &base,
        1,
        "10",
        serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
    )
    .await;
    assert!(tl.contains("bus_post") && tl.contains("bus_inbox"));

    // dir 10 posts to dir 20
    rpc(
        &base,
        1,
        "10",
        serde_json::json!({"jsonrpc":"2.0","id":3,"method":"tools/call",
            "params":{"name":"bus_post","arguments":{"to":"20","text":"hello-20"}}}),
    )
    .await;

    // dir 20 reads its inbox -> sees the message
    let inbox = rpc(
        &base,
        1,
        "20",
        serde_json::json!({"jsonrpc":"2.0","id":4,"method":"tools/call",
            "params":{"name":"bus_inbox","arguments":{}}}),
    )
    .await;
    assert!(inbox.contains("hello-20"), "inbox should contain the posted message: {inbox}");
    assert!(inbox.contains("\\\"from\\\":\\\"10\\\"") || inbox.contains("\"from\":\"10\""));
}
```

Ensure `pub mod bus;` in `lib.rs` (change `mod bus;` from Task 1 to `pub mod bus;`) so the test can reach `weft_app_lib::bus::...`.

- [ ] **Step 3: Run the integration test**

Run: `cd /Users/solojiang/workspace/weft/src-tauri && cargo test --test bus_http 2>&1 | tail -15`
Expected: `two_directions_exchange_a_message ... ok`.

- [ ] **Step 4: Commit**
```bash
cd /Users/solojiang/workspace/weft
git add src-tauri/src/bus/server.rs src-tauri/src/lib.rs src-tauri/tests/bus_http.rs
git commit -m "feat(bus): MCP-over-HTTP handler (initialize/tools/list/tools/call, SSE)"
```

---

## Task 3: Start the bus server in the app

**Files:** Modify `src-tauri/src/lib.rs`.

- [ ] **Step 1: Start the server + manage the registry and base URL**

In `src-tauri/src/lib.rs` `run()`, after the DB is opened and before building the Tauri app, start the bus and capture the base URL. Add this block (the `run()` already uses `tauri::async_runtime::block_on` for the DB — reuse that runtime):

```rust
    // Start the thread-bus HTTP MCP server on an ephemeral port.
    let bus = bus::BusRegistry::new();
    let bus_base: String = {
        let bus = bus.clone();
        tauri::async_runtime::block_on(async move {
            let (base, _handle) = bus::server::serve(bus).await.expect("start bus server");
            // leak the JoinHandle: the server lives for the app's lifetime
            base
        })
    };
    eprintln!("[weft] thread bus on {bus_base}");
```

Then add both to managed state (alongside `.manage(db)` / `.manage(pty::PtyState::default())`):
```rust
        .manage(bus)
        .manage(BusBase(bus_base))
```

And define a tiny newtype near the top of `lib.rs` (after the `mod` lines):
```rust
/// The bus server's base URL, e.g. "http://127.0.0.1:54321".
pub struct BusBase(pub String);
```

- [ ] **Step 2: Verify it compiles and boots**

Run: `cd /Users/solojiang/workspace/weft/src-tauri && cargo build 2>&1 | tail -8`
Expected: `Finished`.

(Behavioral boot check happens in Task 6 via the dev bridge; the server binding is already covered by Task 2's test using the same `serve`.)

- [ ] **Step 3: Commit**
```bash
cd /Users/solojiang/workspace/weft
git add src-tauri/src/lib.rs
git commit -m "feat(bus): start the bus HTTP server at app startup; manage registry + base URL"
```

---

## Task 4: Per-tool ephemeral additive injection

**Files:** Modify `src-tauri/src/bus/inject.rs`.

Builds the spawn-time injection for a given (base, thread, dir, tool, cwd). Verified mechanisms from the spike: claude `--mcp-config <file>` (additive over repo `.mcp.json`), codex `-c mcp_servers.weft_bus.url=<url>`, opencode deep-merge into the worktree `opencode.json` (preserving existing content). All ephemeral / worktree-local — never the canonical repo.

- [ ] **Step 1: Write `src-tauri/src/bus/inject.rs`**

```rust
//! Spawn-time, ADDITIVE injection of the thread bus as an MCP server for each
//! tool. Never overrides a sub-repo's own config: claude/codex use file-less
//! launch flags; opencode deep-merges into the worktree opencode.json (which is
//! a throwaway worktree, not the canonical repo — architecture §2.1).

use std::path::Path;

/// Extra args to PREPEND to the tool's own args (global flags must precede any
/// subcommand, e.g. `codex -c k=v resume <id>`).
pub struct Injection {
    pub args: Vec<String>,
}

fn mcp_url(base: &str, thread: i32, dir: &str) -> String {
    format!("{base}/bus/{thread}/{dir}/mcp")
}

/// Build the injection. `cwd` is the worktree (used for the claude temp config
/// and the opencode merge). `dir` is the direction id as a string.
pub fn inject(base: &str, thread: i32, dir: &str, tool: &str, cwd: &Path) -> Injection {
    let url = mcp_url(base, thread, dir);
    match tool {
        "claude" => {
            // ephemeral --mcp-config file inside the worktree (.weft is gitignored
            // via the worktree's own .git/info/exclude in Task 5 wiring).
            let cfg = cwd.join(".weft-bus.mcp.json");
            let json = serde_json::json!({
                "mcpServers": { "weft_bus": { "type": "http", "url": url } }
            });
            let _ = std::fs::write(&cfg, serde_json::to_vec_pretty(&json).unwrap_or_default());
            Injection {
                args: vec!["--mcp-config".into(), cfg.to_string_lossy().to_string()],
            }
        }
        "codex" => Injection {
            args: vec!["-c".into(), format!("mcp_servers.weft_bus.url={url}")],
        },
        "opencode" => {
            merge_opencode_config(cwd, &url);
            Injection { args: vec![] }
        }
        _ => Injection { args: vec![] },
    }
}

/// Deep-merge `mcp.weft_bus = {type:remote, url, enabled:true}` into the
/// worktree's opencode.json, preserving any existing config the sub-repo shipped.
fn merge_opencode_config(cwd: &Path, url: &str) {
    let path = cwd.join("opencode.json");
    let mut root: serde_json::Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    if !root.is_object() {
        root = serde_json::json!({});
    }
    let obj = root.as_object_mut().unwrap();
    obj.entry("$schema".to_string())
        .or_insert_with(|| serde_json::json!("https://opencode.ai/config.json"));
    let mcp = obj
        .entry("mcp".to_string())
        .or_insert_with(|| serde_json::json!({}));
    if let Some(mcp_obj) = mcp.as_object_mut() {
        mcp_obj.insert(
            "weft_bus".to_string(),
            serde_json::json!({ "type": "remote", "url": url, "enabled": true }),
        );
    }
    let _ = std::fs::write(&path, serde_json::to_vec_pretty(&root).unwrap_or_default());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_writes_mcp_config_and_flags() {
        let dir = std::env::temp_dir().join(format!("weft-inj-claude-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let inj = inject("http://127.0.0.1:9", 1, "10", "claude", &dir);
        assert_eq!(inj.args[0], "--mcp-config");
        let cfg = std::fs::read_to_string(dir.join(".weft-bus.mcp.json")).unwrap();
        assert!(cfg.contains("weft_bus") && cfg.contains("/bus/1/10/mcp"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn codex_uses_config_override() {
        let inj = inject("http://127.0.0.1:9", 2, "30", "codex", Path::new("/tmp"));
        assert_eq!(inj.args, vec!["-c".to_string(),
            "mcp_servers.weft_bus.url=http://127.0.0.1:9/bus/2/30/mcp".to_string()]);
    }

    #[test]
    fn opencode_merges_preserving_existing() {
        let dir = std::env::temp_dir().join(format!("weft-inj-oc-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        // sub-repo already ships an opencode.json with its own mcp server
        std::fs::write(
            dir.join("opencode.json"),
            r#"{"mcp":{"repo_own":{"type":"local","command":["x"]}}}"#,
        )
        .unwrap();
        let inj = inject("http://127.0.0.1:9", 1, "10", "opencode", &dir);
        assert!(inj.args.is_empty());
        let merged: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("opencode.json")).unwrap())
                .unwrap();
        // both the repo's server AND weft_bus must be present
        assert!(merged["mcp"]["repo_own"].is_object(), "repo's own server preserved");
        assert_eq!(merged["mcp"]["weft_bus"]["type"], "remote");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
```

- [ ] **Step 2: Run the injection tests**

Run: `cd /Users/solojiang/workspace/weft/src-tauri && cargo test bus::inject 2>&1 | tail -12`
Expected: `claude_writes_mcp_config_and_flags`, `codex_uses_config_override`, `opencode_merges_preserving_existing` all `ok`.

- [ ] **Step 3: Commit**
```bash
cd /Users/solojiang/workspace/weft
git add src-tauri/src/bus/inject.rs
git commit -m "feat(bus): per-tool ephemeral additive injection (claude/codex/opencode)"
```

---

## Task 5: Wire injection into spawn

**Files:** Modify `src-tauri/src/pty.rs`.

`open_session`/`resume` compute the bus injection for the session's (thread, direction, tool) and prepend the injection args to the tool's own args at spawn.

- [ ] **Step 1: Thread the injection args into `spawn`**

In `src-tauri/src/pty.rs`, change `spawn` to accept extra leading args and prepend them before the driver's args. Find:
```rust
    let (program, args) = driver.command(&spec);
    let mut cmd = CommandBuilder::new(&program);
    for a in &args {
        cmd.arg(a);
    }
```
Replace with (add an `inject_args: &[String]` parameter to `spawn`'s signature, before `session_id`):
```rust
    let (program, dargs) = driver.command(&spec);
    let mut cmd = CommandBuilder::new(&program);
    for a in inject_args.iter().chain(dargs.iter()) {
        cmd.arg(a);
    }
```
Update `spawn`'s signature to:
```rust
fn spawn(
    app: &AppHandle,
    tool: &str,
    inject_args: &[String],
    cwd: &PathBuf,
    resume_id: Option<&str>,
    session_id: i32,
    db: Db,
) -> Result<Active> {
```

- [ ] **Step 2: Compute the injection in `open_session_impl` and pass it**

In `open_session_impl`, after `let cwd = PathBuf::from(&wt.path);` and the `dir` lookup, add (the `dir` model has `thread_id`; `BusBase`/`BusRegistry` are in Tauri state — fetch via `app`):
```rust
    let base = app
        .state::<crate::BusBase>()
        .0
        .clone();
    let inj = crate::bus::inject::inject(&base, dir.thread_id, &direction_id.to_string(), &dir.tool, &cwd);
```
Change the spawn call from:
```rust
    let active =
        spawn(&app, &dir.tool, &cwd, None, sess.id, db.clone()).context("spawn agent")?;
```
to:
```rust
    let active = spawn(&app, &dir.tool, &inj.args, &cwd, None, sess.id, db.clone())
        .context("spawn agent")?;
```
Add `use tauri::Manager;` at the top of pty.rs if not already present (needed for `app.state::<_>()`).

- [ ] **Step 3: Same for `resume_impl`**

In `resume_impl`, after `let cwd = PathBuf::from(&wt.path);` add:
```rust
    let base = app.state::<crate::BusBase>().0.clone();
    let inj = crate::bus::inject::inject(&base, s.direction_id, &s.direction_id.to_string(), &s.tool, &cwd);
```
Wait — injection identity is the DIRECTION id, and the URL needs the THREAD id. `s` (session) has `direction_id` but not `thread_id`. Fetch the direction to get its `thread_id`:
```rust
    let tid = {
        use sea_orm::EntityTrait;
        crate::store::entities::direction::Entity::find_by_id(s.direction_id)
            .one(&db.0)
            .await?
            .map(|d| d.thread_id)
            .unwrap_or(0)
    };
    let base = app.state::<crate::BusBase>().0.clone();
    let inj = crate::bus::inject::inject(&base, tid, &s.direction_id.to_string(), &s.tool, &cwd);
```
And change the resume spawn call from:
```rust
    let active = spawn(&app, &s.tool, &cwd, Some(&native), session_id, db.clone())
        .context("spawn agent --resume")?;
```
to:
```rust
    let active = spawn(&app, &s.tool, &inj.args, &cwd, Some(&native), session_id, db.clone())
        .context("spawn agent --resume")?;
```
(In `open_session_impl`, `dir.thread_id` is already available from the `dir` lookup, so use that directly as shown in Step 2.)

- [ ] **Step 4: Verify it compiles + no test regressions**

Run: `cd /Users/solojiang/workspace/weft/src-tauri && cargo build 2>&1 | tail -8 && cargo test --lib 2>&1 | tail -6`
Expected: `Finished`; existing lib tests (now including bus::state, bus::inject) all pass.

- [ ] **Step 5: Commit**
```bash
cd /Users/solojiang/workspace/weft
git add src-tauri/src/pty.rs
git commit -m "feat(bus): inject the thread bus as an MCP server at session spawn"
```

---

## Task 6: Live end-to-end via the dev bridge

**Files:** none (verification). Confirms two real agents in one thread message each other through the bus.

- [ ] **Step 1: Launch the dev app (isolated home) and connect the bridge**

```bash
R=/private/tmp/weft-bus-e2e; rm -rf "$R"; mkdir -p "$R"; cd "$R"; git init -q
git config user.email u@u.u; git config user.name u; echo x > README.md; git add -A; git commit -qm init
rm -rf /private/tmp/weft-bus-e2e-home
cd /Users/solojiang/workspace/weft
PATH=/Users/solojiang/.nvm/versions/node/v24.15.0/bin:$HOME/.cargo/bin:$PATH WEFT_HOME=/private/tmp/weft-bus-e2e-home TAURI_DEV_HOST= npm run tauri dev
```
Wait for `WebSocket server listening on: 0.0.0.0:9223` and the `[weft] thread bus on http://127.0.0.1:<port>` line. Then `driver_session(start, port 9223)`.

- [ ] **Step 2: Seed a thread with two directions (claude + codex) and open both sessions**

Via `webview_execute_js` + `window.__TAURI_INTERNALS__.invoke` (the async-IIFE-on-window pattern; read results from the DB at `/private/tmp/weft-bus-e2e-home/weft.db`):
create_workspace → add_repo_ref(the e2e repo) → create_thread → create_direction(claude, write) → create_direction(codex, write). Then `open_session` each (drive each tool past its gate via `write_pty` as in M2/C verification).

- [ ] **Step 3: Drive a cross-agent message and assert routing**

Have the claude direction call `bus_post` to the codex direction's id (instruct it in a prompt: "Use the bus_post tool with to=<codex_direction_id>, text=hello-codex"). Then have the codex direction call `bus_inbox` (prompt it: "Call bus_inbox and tell me what you received"). Confirm codex reports `hello-claude`/`hello-codex` from the claude direction.

Cross-check from Bash with `curl` against the bus base URL printed at startup: `POST <base>/bus/<thread>/<codexDir>/mcp` `bus_inbox` should be empty AFTER the agent read it (proving the agent, not curl, consumed it), and the timeline (a later UI concern) holds both.

- [ ] **Step 4: Record the result in the spec and commit**

Add a "✅ v1a 实测结论" line to `docs/superpowers/specs/2026-06-05-thread-bus-coordination-design.md`, then:
```bash
cd /Users/solojiang/workspace/weft
git add docs/superpowers/specs/2026-06-05-thread-bus-coordination-design.md
git commit -m "docs(thread-bus): record v1a live e2e (cross-agent message through the bus)"
```

---

## Self-review checklist (run before handoff)

- **Spec coverage:** single global HTTP MCP server keyed by `/bus/:thread/:dir` (T2,T3) ✓; identity from URL not agent input (T2 `handle`) ✓; tools post/broadcast/inbox/thread_state/announce (T2) ✓; ephemeral additive injection claude/codex/opencode, never overriding sub-repo config (T4, with the preserve-existing test) ✓; wired at spawn for new + resume (T5) ✓; live cross-agent e2e (T6) ✓. Passive `.thread/` layer, coordinator wake, and the UI panel are explicitly **v1b** (out of this plan).
- **Placeholder scan:** none — every step has real code/commands.
- **Type consistency:** `BusRegistry` (join/post/broadcast/inbox/state_get/state_set/log), `Msg{from,to,text,ts,kind}`, `server::serve -> (String, JoinHandle)`, `BusBase(String)`, `inject(base, thread, dir, tool, cwd) -> Injection{args}`, `spawn(app, tool, inject_args, cwd, resume_id, session_id, db)` are used consistently across tasks.

## Notes for the executor
- node v24 for `npm run tauri dev`. The dev MCP bridge is debug-only; the thread-bus HTTP server runs in all builds.
- The bus `serve` binds `127.0.0.1:0` (ephemeral) so multiple app instances don't collide; the chosen URL flows to injection via `BusBase`.
- Driving codex/opencode past their gates in T6: codex shows an "update available" gate (send `2\r`) and writes its rollout only after the first turn; opencode shows an update prompt — same pattern as the C-milestone verification.
- After T6, the next plan (v1b) adds: the UI coordination panel (read `BusRegistry::log` + a `bus_post` command for the human), the basic coordinator wake (inject a turn into an idle target when a message lands), and the passive `.thread/`+PLAN.md layer.
