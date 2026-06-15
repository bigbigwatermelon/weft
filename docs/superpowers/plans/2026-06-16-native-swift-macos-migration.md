# Native Swift macOS Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first working AtlasNative macOS 26+ SwiftUI/AppKit MVP that launches a bundled Rust core server and drives real Atlas agent sessions.

**Architecture:** Keep the existing Rust runtime as the source of truth and add a typed local core API around it. The Swift app owns all visible macOS UI and talks only through `CoreClient`; it never reads SQLite, spawns agent CLIs, or runs git directly.

**Tech Stack:** Rust 2021, Tauri v2-compatible Rust core, Axum, Tokio, SeaORM, SQLite/SQLCipher, Swift 6, SwiftUI, AppKit, XCTest, macOS 26+ Liquid Glass system components.

---

## Scope Check

The approved spec includes Phase 0 through Phase 3. This plan implements the first testable milestone only:

- Phase 0: extract a reusable Rust core API/server boundary while keeping the current Tauri app working.
- Phase 1: add a native Swift macOS shell that can list workspaces, create issues/runs, open real agent sessions, send/stream chat, handle Needs/Ask, and shut down cleanly.

Phase 2 parity expansion and Phase 3 Tauri retirement need separate plans after this MVP is running. They include full diff review, repo graph editing, backup restore UI, IM administration, full skills management, app update flow, and removal of the React/Tauri default build.

## File Structure

### Rust Core API

- Create: `src-tauri/src/core_api/mod.rs`
  - Responsibility: core API module root and public re-exports.
- Create: `src-tauri/src/core_api/dto.rs`
  - Responsibility: JSON DTOs shared by the core server and Swift client.
- Create: `src-tauri/src/core_api/error.rs`
  - Responsibility: stable JSON error shape and helper conversions.
- Create: `src-tauri/src/core_api/events.rs`
  - Responsibility: broadcast event bus, `CoreEvent`, and conversion from existing lead-chat pushes.
- Create: `src-tauri/src/core_api/handlers.rs`
  - Responsibility: command handlers that call existing store, command, ask, needs, and chat-engine functions without Tauri wrappers.
- Create: `src-tauri/src/core_api/server.rs`
  - Responsibility: Axum server, token auth, health/version, command routes, event stream, and graceful shutdown.
- Create: `src-tauri/src/bin/atlas-core-server.rs`
  - Responsibility: standalone bundled Rust core process launched by Swift.
- Modify: `src-tauri/src/lib.rs`
  - Responsibility: expose `core_api` and attach the event bus to existing Tauri runtime.
- Modify: `src-tauri/Cargo.toml`
  - Responsibility: add the `atlas-core-server` bin and minimal server dependencies.
- Test: `src-tauri/tests/core_api_contract.rs`
  - Responsibility: DTO schema, health endpoint, token rejection, command JSON, and event-stream smoke tests.

### Rust Runtime Extraction

- Modify: `src-tauri/src/lead_chat/engine.rs`
  - Responsibility: route every frontend push through a helper that can emit to both Tauri and the core event bus.
- Modify: `src-tauri/src/lead_chat/commands.rs`
  - Responsibility: add core-callable functions beside existing Tauri commands.
- Modify: `src-tauri/src/ask.rs`
  - Responsibility: bridge ask lifecycle events into `CoreEvent`.
- Modify: `src-tauri/src/coordinator.rs`
  - Responsibility: bridge Needs-you change notifications into `CoreEvent`.
- Modify: `src-tauri/src/commands.rs`
  - Responsibility: keep Tauri wrappers thin and delegate shared command logic to `core_api::handlers` where safe.

### Swift Native App

- Create: `native/AtlasNative/Package.swift`
  - Responsibility: Swift package with app, core client, and tests.
- Create: `native/AtlasNative/Sources/AtlasCoreClient/CoreDTOs.swift`
  - Responsibility: Swift DTOs matching Rust `core_api::dto`.
- Create: `native/AtlasNative/Sources/AtlasCoreClient/CoreClient.swift`
  - Responsibility: typed command client and event stream decoder.
- Create: `native/AtlasNative/Sources/AtlasCoreClient/CoreProcess.swift`
  - Responsibility: launch, handshake, token storage, and shutdown for `atlas-core-server`.
- Create: `native/AtlasNative/Sources/AtlasNativeApp/AtlasNativeApp.swift`
  - Responsibility: SwiftUI app entry point.
- Create: `native/AtlasNative/Sources/AtlasNativeApp/AppStore.swift`
  - Responsibility: route, selection, state, event reducer, and user intents.
- Create: `native/AtlasNative/Sources/AtlasNativeApp/Views/AppShell.swift`
  - Responsibility: `NavigationSplitView` shell, toolbar, and high-level routing.
- Create: `native/AtlasNative/Sources/AtlasNativeApp/Views/WorkspaceSidebar.swift`
  - Responsibility: workspace, Needs, issues, and settings navigation.
- Create: `native/AtlasNative/Sources/AtlasNativeApp/Views/IssueRunViews.swift`
  - Responsibility: issue list, run list, create issue, and create run UI.
- Create: `native/AtlasNative/Sources/AtlasNativeApp/Views/ChatViews.swift`
  - Responsibility: lead/worker chat timeline, composer, activity, stop, and interrupt UI.
- Create: `native/AtlasNative/Sources/AtlasNativeApp/Views/NeedsAskViews.swift`
  - Responsibility: Needs-you list and permission ask sheet/popover.
- Create: `native/AtlasNative/Sources/AtlasNativeApp/Views/SettingsViews.swift`
  - Responsibility: execution settings needed by the MVP.
- Test: `native/AtlasNative/Tests/AtlasCoreClientTests/CoreClientTests.swift`
  - Responsibility: command encoding, event decoding, token auth, and mock transport.
- Test: `native/AtlasNative/Tests/AtlasCoreClientTests/CoreProcessTests.swift`
  - Responsibility: core process handshake parsing and shutdown behavior.
- Test: `native/AtlasNative/Tests/AtlasNativeAppTests/AppStoreTests.swift`
  - Responsibility: event reducer behavior and user intent routing.

### Build And Verification

- Modify: `scripts/preflight.sh`
  - Responsibility: add opt-in Swift checks without breaking current Tauri preflight while the native app is in migration.
- Create: `scripts/native-preflight.sh`
  - Responsibility: run Rust core API tests, Swift tests, and a core server smoke test.
- Create: `docs/native-macos-migration.md`
  - Responsibility: developer runbook for launching the core server and the native Swift app.

---

### Task 1: Add Core API DTO Contract

**Files:**
- Create: `src-tauri/src/core_api/mod.rs`
- Create: `src-tauri/src/core_api/dto.rs`
- Create: `src-tauri/src/core_api/error.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/tests/core_api_contract.rs`

- [ ] **Step 1: Write the failing DTO contract tests**

Create `src-tauri/tests/core_api_contract.rs` with:

```rust
use atlas_app_lib::core_api::dto::{
    ApiVersion, CoreEvent, HealthResponse, IssueDto, LeadChatPushDto, RunDto, WorkspaceDto,
};

#[test]
fn api_version_is_stable_for_native_mvp() {
    assert_eq!(ApiVersion::CURRENT, 1);
}

#[test]
fn health_response_serializes_for_swift_handshake() {
    let body = serde_json::to_value(HealthResponse {
        ok: true,
        api_version: ApiVersion::CURRENT,
        product: "Atlas".into(),
    })
    .unwrap();

    assert_eq!(body["ok"], true);
    assert_eq!(body["api_version"], 1);
    assert_eq!(body["product"], "Atlas");
}

#[test]
fn dto_names_match_native_product_language() {
    let workspace = WorkspaceDto {
        id: 7,
        name: "Default".into(),
        slug: "default".into(),
        created_at: "2026-06-16T00:00:00Z".into(),
    };
    let issue = IssueDto {
        id: 8,
        workspace_id: 7,
        title: "Migrate UI".into(),
        slug: "migrate-ui".into(),
        kind: "refactor".into(),
        created_at: "2026-06-16T00:00:01Z".into(),
    };
    let run = RunDto {
        id: 9,
        issue_id: 8,
        name: "Swift shell".into(),
        slug: "swift-shell".into(),
        tool: "codex".into(),
        branch: "codex/native-swift-macos-migration".into(),
        repo_id: 0,
        status: "queued".into(),
        mandate: "plan+impl".into(),
        created_at: "2026-06-16T00:00:02Z".into(),
    };

    assert_eq!(workspace.slug, "default");
    assert_eq!(issue.workspace_id, workspace.id);
    assert_eq!(run.issue_id, issue.id);
    assert_eq!(run.repo_id, 0);
}

#[test]
fn lead_chat_event_has_snake_case_payload() {
    let event = CoreEvent::LeadChat(LeadChatPushDto::Turn {
        thread_id: 8,
        session_id: Some(42),
        state: "busy".into(),
        queued: 1,
    });
    let body = serde_json::to_string(&event).unwrap();

    assert!(body.contains("\"type\":\"lead_chat\""));
    assert!(body.contains("\"event\":\"turn\""));
    assert!(body.contains("\"thread_id\":8"));
    assert!(body.contains("\"session_id\":42"));
}
```

- [ ] **Step 2: Run the failing contract tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test core_api_contract
```

Expected: FAIL because `atlas_app_lib::core_api` is not defined.

- [ ] **Step 3: Create the core API module root**

Create `src-tauri/src/core_api/mod.rs`:

```rust
pub mod dto;
pub mod error;
pub mod events;
pub mod handlers;
pub mod server;
```

Modify `src-tauri/src/lib.rs` by adding this module declaration near the other `pub mod` declarations:

```rust
pub mod core_api;
```

- [ ] **Step 4: Create the error model**

Create `src-tauri/src/core_api/error.rs`:

```rust
use axum::{http::StatusCode, response::IntoResponse, Json};

#[derive(Debug, serde::Serialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
}

#[derive(Debug)]
pub struct CoreError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
}

impl CoreError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "bad_request",
            message: message.into(),
        }
    }

    pub fn unauthorized() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "unauthorized",
            message: "missing or invalid core API token".into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal",
            message: message.into(),
        }
    }
}

impl IntoResponse for CoreError {
    fn into_response(self) -> axum::response::Response {
        (
            self.status,
            Json(ErrorBody {
                code: self.code.into(),
                message: self.message,
            }),
        )
            .into_response()
    }
}

impl<E: std::fmt::Display> From<E> for CoreError {
    fn from(value: E) -> Self {
        Self::internal(value.to_string())
    }
}

pub type CoreResult<T> = Result<T, CoreError>;
```

- [ ] **Step 5: Create the DTOs**

Create `src-tauri/src/core_api/dto.rs`:

```rust
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ApiVersion;

impl ApiVersion {
    pub const CURRENT: u32 = 1;
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct HealthResponse {
    pub ok: bool,
    pub api_version: u32,
    pub product: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceDto {
    pub id: i32,
    pub name: String,
    pub slug: String,
    pub created_at: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RepoRefDto {
    pub id: i32,
    pub workspace_id: i32,
    pub name: String,
    pub slug: String,
    pub local_git_path: String,
    pub base_ref: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct IssueDto {
    pub id: i32,
    pub workspace_id: i32,
    pub title: String,
    pub slug: String,
    pub kind: String,
    pub created_at: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RunDto {
    pub id: i32,
    pub issue_id: i32,
    pub name: String,
    pub slug: String,
    pub tool: String,
    pub branch: String,
    pub repo_id: i32,
    pub status: String,
    pub mandate: String,
    pub created_at: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SessionInfoDto {
    pub session_id: i32,
    pub repo: String,
    pub worktree: String,
    pub cwd: String,
    pub branch: String,
    pub tool: String,
    pub resumed: bool,
    pub native_id: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LeadMessageDto {
    pub id: i32,
    pub thread_id: i32,
    pub session_id: Option<i32>,
    pub turn_id: i32,
    pub role: String,
    pub kind: String,
    pub content: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum LeadChatPushDto {
    Message {
        thread_id: i32,
        message: LeadMessageDto,
    },
    Delta {
        thread_id: i32,
        message_id: i32,
        text: String,
    },
    Finalize {
        thread_id: i32,
        message_id: i32,
        status: String,
    },
    Turn {
        thread_id: i32,
        session_id: Option<i32>,
        state: String,
        queued: usize,
    },
    Init {
        thread_id: i32,
        session_id: Option<i32>,
        native_id: String,
        slash_commands: Vec<SlashCmdDto>,
    },
    Activity {
        thread_id: i32,
        session_id: Option<i32>,
        name: String,
        summary: String,
    },
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SlashCmdDto {
    pub name: String,
    pub description: Option<String>,
    pub arg_hint: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct NeedItemDto {
    pub thread_id: i32,
    pub ask_id: i64,
    pub from: String,
    pub question: String,
    pub created_at: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PermissionAskDto {
    pub id: i64,
    pub dir: String,
    pub tool: String,
    pub action: String,
    pub cwd: String,
    pub created_at_ms: u128,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct WriteTriggerDto {
    pub thread_id: i32,
    pub index: usize,
    pub name: String,
    pub tool: String,
    pub repo: String,
    pub reason: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ToolStatusDto {
    pub tool: String,
    pub installed: bool,
    pub path: Option<String>,
    pub version: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum CoreEvent {
    LeadChat(LeadChatPushDto),
    NeedsChanged { thread_id: i32 },
    AsksChanged,
    WorkspaceChanged { workspace_id: i32 },
    SessionStatus { session_id: i32, status: String },
    Fatal { message: String },
}
```

- [ ] **Step 6: Run the DTO contract tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test core_api_contract
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/core_api/mod.rs src-tauri/src/core_api/dto.rs src-tauri/src/core_api/error.rs src-tauri/tests/core_api_contract.rs
git commit -m "feat(core-api): add native DTO contract"
```

---

### Task 2: Add Core Event Bus And Dual Emit Path

**Files:**
- Create: `src-tauri/src/core_api/events.rs`
- Modify: `src-tauri/src/core_api/dto.rs`
- Modify: `src-tauri/src/lead_chat/engine.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/tests/core_api_contract.rs`

- [ ] **Step 1: Add failing event bus tests**

Append to `src-tauri/tests/core_api_contract.rs`:

```rust
use atlas_app_lib::core_api::events::CoreEventBus;

#[tokio::test]
async fn event_bus_broadcasts_to_subscribers() {
    let bus = CoreEventBus::default();
    let mut rx = bus.subscribe();

    bus.emit(CoreEvent::AsksChanged);

    let got = rx.recv().await.unwrap();
    assert!(matches!(got, CoreEvent::AsksChanged));
}
```

- [ ] **Step 2: Run the event bus test**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test core_api_contract event_bus_broadcasts_to_subscribers
```

Expected: FAIL because `core_api::events` has no `CoreEventBus`.

- [ ] **Step 3: Create the event bus**

Create `src-tauri/src/core_api/events.rs`:

```rust
use tokio::sync::broadcast;

use super::dto::{
    CoreEvent, LeadChatPushDto, LeadMessageDto, SlashCmdDto,
};

#[derive(Clone)]
pub struct CoreEventBus {
    tx: broadcast::Sender<CoreEvent>,
}

impl Default for CoreEventBus {
    fn default() -> Self {
        let (tx, _rx) = broadcast::channel(512);
        Self { tx }
    }
}

impl CoreEventBus {
    pub fn emit(&self, event: CoreEvent) {
        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<CoreEvent> {
        self.tx.subscribe()
    }
}

pub fn lead_push_to_core(push: crate::lead_chat::engine::Push) -> CoreEvent {
    CoreEvent::LeadChat(match push {
        crate::lead_chat::engine::Push::Message { thread_id, message } => {
            LeadChatPushDto::Message {
                thread_id,
                message: lead_message_to_dto(message),
            }
        }
        crate::lead_chat::engine::Push::Delta {
            thread_id,
            message_id,
            text,
        } => LeadChatPushDto::Delta {
            thread_id,
            message_id,
            text,
        },
        crate::lead_chat::engine::Push::Finalize {
            thread_id,
            message_id,
            status,
        } => LeadChatPushDto::Finalize {
            thread_id,
            message_id,
            status,
        },
        crate::lead_chat::engine::Push::Turn {
            thread_id,
            session_id,
            state,
            queued,
        } => LeadChatPushDto::Turn {
            thread_id,
            session_id,
            state,
            queued,
        },
        crate::lead_chat::engine::Push::Init {
            thread_id,
            session_id,
            native_id,
            slash_commands,
        } => LeadChatPushDto::Init {
            thread_id,
            session_id,
            native_id,
            slash_commands: slash_commands
                .into_iter()
                .map(|cmd| SlashCmdDto {
                    name: cmd.name,
                    description: cmd.description,
                    arg_hint: cmd.arg_hint,
                })
                .collect(),
        },
        crate::lead_chat::engine::Push::Activity {
            thread_id,
            session_id,
            name,
            summary,
        } => LeadChatPushDto::Activity {
            thread_id,
            session_id,
            name,
            summary,
        },
    })
}

fn lead_message_to_dto(message: crate::store::entities::lead_message::Model) -> LeadMessageDto {
    LeadMessageDto {
        id: message.id,
        thread_id: message.thread_id,
        session_id: message.session_id,
        turn_id: message.turn_id,
        role: message.role,
        kind: message.kind,
        content: message.content,
        status: message.status,
        created_at: message.created_at.to_string(),
    }
}
```

- [ ] **Step 4: Register the bus in Tauri**

Modify `src-tauri/src/lib.rs` inside the builder chain near the existing managed state:

```rust
.manage(core_api::events::CoreEventBus::default())
```

Place it next to `lead_chat::out_hub::LeadOutHub::default()` so the event bus is available before runtime setup starts.

- [ ] **Step 5: Add a lead-chat dual emit helper**

In `src-tauri/src/lead_chat/engine.rs`, add this helper after the `Push` enum:

```rust
fn emit_push(app: &AppHandle, push: Push) {
    let _ = app.emit(EVENT, push.clone());
    if let Some(bus) = app.try_state::<crate::core_api::events::CoreEventBus>() {
        bus.emit(crate::core_api::events::lead_push_to_core(push));
    }
}
```

Then replace each direct `app.emit(EVENT, Push::...)` call in `src-tauri/src/lead_chat/engine.rs` with:

```rust
emit_push(app, Push::...);
```

Keep non-lead events such as `"needs-you://changed"` unchanged until Task 6.

- [ ] **Step 6: Run event bus tests and lead chat tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test core_api_contract
cargo test --manifest-path src-tauri/Cargo.toml lead_chat --lib
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/core_api/events.rs src-tauri/src/lead_chat/engine.rs src-tauri/tests/core_api_contract.rs
git commit -m "feat(core-api): broadcast lead chat events"
```

---

### Task 3: Add Core Server Health, Token Auth, And Event Stream

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/core_api/server.rs`
- Create: `src-tauri/src/bin/atlas-core-server.rs`
- Modify: `src-tauri/src/core_api/mod.rs`
- Test: `src-tauri/tests/core_api_contract.rs`

- [ ] **Step 1: Add failing server tests**

Append to `src-tauri/tests/core_api_contract.rs`:

```rust
use atlas_app_lib::core_api::server::{build_router, CoreServerState};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

#[tokio::test]
async fn health_endpoint_requires_token() {
    let state = CoreServerState::for_test("secret-token");
    let app = build_router(state);

    let response = app
        .oneshot(Request::builder().uri("/v1/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn health_endpoint_returns_version_with_token() {
    let state = CoreServerState::for_test("secret-token");
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/health")
                .header("x-atlas-core-token", "secret-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
```

- [ ] **Step 2: Run the server tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test core_api_contract health_endpoint
```

Expected: FAIL because `core_api::server` has no router.

- [ ] **Step 3: Add dependencies and bin target**

Modify `src-tauri/Cargo.toml`:

```toml
tokio-stream = { version = "0.1", features = ["sync"] }

[[bin]]
name = "atlas-core-server"
path = "src/bin/atlas-core-server.rs"
```

Keep the existing `[dependencies]`, `[dev-dependencies]`, and package metadata intact.

- [ ] **Step 4: Implement the server module**

Create `src-tauri/src/core_api/server.rs`:

```rust
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::sse::{Event, KeepAlive, Sse},
    routing::get,
    Json, Router,
};
use futures::{Stream, StreamExt};
use std::{convert::Infallible, net::SocketAddr, sync::Arc};
use tokio_stream::wrappers::BroadcastStream;

use super::{
    dto::{ApiVersion, CoreEvent, HealthResponse},
    error::{CoreError, CoreResult},
    events::CoreEventBus,
};

#[derive(Clone)]
pub struct CoreServerState {
    token: Arc<String>,
    events: CoreEventBus,
}

impl CoreServerState {
    pub fn new(token: String, events: CoreEventBus) -> Self {
        Self {
            token: Arc::new(token),
            events,
        }
    }

    pub fn for_test(token: &str) -> Self {
        Self::new(token.to_string(), CoreEventBus::default())
    }

    fn authorize(&self, headers: &HeaderMap) -> CoreResult<()> {
        let got = headers
            .get("x-atlas-core-token")
            .and_then(|v| v.to_str().ok());
        if got == Some(self.token.as_str()) {
            Ok(())
        } else {
            Err(CoreError::unauthorized())
        }
    }
}

pub fn build_router(state: CoreServerState) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/events", get(events))
        .with_state(state)
}

async fn health(
    State(state): State<CoreServerState>,
    headers: HeaderMap,
) -> CoreResult<Json<HealthResponse>> {
    state.authorize(&headers)?;
    Ok(Json(HealthResponse {
        ok: true,
        api_version: ApiVersion::CURRENT,
        product: "Atlas".into(),
    }))
}

async fn events(
    State(state): State<CoreServerState>,
    headers: HeaderMap,
) -> CoreResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    state.authorize(&headers)?;
    let rx = state.events.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|event| async move {
        match event {
            Ok(event) => Some(Ok(core_event_to_sse(event))),
            Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(_)) => {
                Some(Ok(core_event_to_sse(CoreEvent::Fatal {
                    message: "core event stream lagged".into(),
                })))
            }
        }
    });
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

fn core_event_to_sse(event: CoreEvent) -> Event {
    let name = match &event {
        CoreEvent::LeadChat(_) => "lead_chat",
        CoreEvent::NeedsChanged { .. } => "needs_changed",
        CoreEvent::AsksChanged => "asks_changed",
        CoreEvent::WorkspaceChanged { .. } => "workspace_changed",
        CoreEvent::SessionStatus { .. } => "session_status",
        CoreEvent::Fatal { .. } => "fatal",
    };
    let payload = serde_json::to_string(&event)
        .unwrap_or_else(|err| format!(r#"{{"type":"fatal","payload":{{"message":"{}"}}}}"#, err));
    Event::default().event(name).data(payload)
}

pub async fn serve(addr: SocketAddr, token: String, events: CoreEventBus) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let bound = listener.local_addr()?;
    println!(
        "{}",
        serde_json::json!({
            "type": "atlas_core_ready",
            "endpoint": format!("http://{}", bound),
            "api_version": ApiVersion::CURRENT,
            "token": token,
            "pid": std::process::id(),
        })
    );
    axum::serve(listener, build_router(CoreServerState::new(token, events))).await?;
    Ok(())
}
```

- [ ] **Step 5: Create the server binary**

Create `src-tauri/src/bin/atlas-core-server.rs`:

```rust
use atlas_app_lib::core_api::{events::CoreEventBus, server};
use rand::RngCore;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let token = new_token();
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
    server::serve(addr, token, CoreEventBus::default()).await
}

fn new_token() -> String {
    let mut bytes = [0_u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}
```

- [ ] **Step 6: Run server tests and binary smoke check**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test core_api_contract health_endpoint
cargo build --manifest-path src-tauri/Cargo.toml --bin atlas-core-server
```

Expected: PASS for tests and successful bin build.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/core_api/server.rs src-tauri/src/bin/atlas-core-server.rs src-tauri/tests/core_api_contract.rs
git commit -m "feat(core-api): add core server handshake"
```

---

### Task 4: Expose Workspace, Issue, And Run Commands

**Files:**
- Create: `src-tauri/src/core_api/handlers.rs`
- Modify: `src-tauri/src/core_api/server.rs`
- Modify: `src-tauri/src/core_api/dto.rs`
- Test: `src-tauri/tests/core_api_contract.rs`

- [ ] **Step 1: Add failing command route tests**

Append to `src-tauri/tests/core_api_contract.rs`:

```rust
#[tokio::test]
async fn list_workspaces_route_rejects_missing_token() {
    let state = CoreServerState::for_test("secret-token");
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/workspaces")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
```

- [ ] **Step 2: Run the failing command test**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test core_api_contract list_workspaces_route_rejects_missing_token
```

Expected: FAIL because `/v1/workspaces` is not routed.

- [ ] **Step 3: Add command request DTOs**

Append to `src-tauri/src/core_api/dto.rs`:

```rust
#[derive(Clone, Debug, serde::Deserialize)]
pub struct CreateWorkspaceRequest {
    pub name: String,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct CreateIssueRequest {
    pub workspace_id: i32,
    pub title: String,
    pub kind: String,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct CreateRunRequest {
    pub issue_id: i32,
    pub name: String,
    pub tool: String,
    pub reason: Option<String>,
}
```

- [ ] **Step 4: Implement core handlers**

Create `src-tauri/src/core_api/handlers.rs`:

```rust
use crate::store::{repo, Db};

use super::{
    dto::{
        CreateIssueRequest, CreateRunRequest, CreateWorkspaceRequest, IssueDto, RunDto,
        WorkspaceDto,
    },
    error::{CoreError, CoreResult},
};

pub async fn list_workspaces(db: &Db) -> CoreResult<Vec<WorkspaceDto>> {
    repo::list_workspaces(db)
        .await
        .map_err(CoreError::from)
        .map(|items| items.into_iter().map(workspace_to_dto).collect())
}

pub async fn create_workspace(db: &Db, req: CreateWorkspaceRequest) -> CoreResult<WorkspaceDto> {
    if req.name.trim().is_empty() {
        return Err(CoreError::bad_request("workspace name is required"));
    }
    let created = repo::create_workspace(db, req.name.trim())
        .await
        .map_err(CoreError::from)?;
    Ok(workspace_to_dto(created))
}

pub async fn list_issues(db: &Db, workspace_id: i32) -> CoreResult<Vec<IssueDto>> {
    repo::list_threads(db, workspace_id)
        .await
        .map_err(CoreError::from)
        .map(|items| items.into_iter().map(issue_to_dto).collect())
}

pub async fn create_issue(db: &Db, req: CreateIssueRequest) -> CoreResult<IssueDto> {
    if req.title.trim().is_empty() {
        return Err(CoreError::bad_request("issue title is required"));
    }
    let tool = crate::tools::default_tool(db).await;
    let created = repo::create_thread(db, req.workspace_id, req.title.trim(), &req.kind, &tool)
        .await
        .map_err(CoreError::from)?;
    Ok(issue_to_dto(created))
}

pub async fn list_runs(db: &Db, issue_id: i32) -> CoreResult<Vec<RunDto>> {
    repo::list_directions(db, issue_id)
        .await
        .map_err(CoreError::from)
        .map(|items| items.into_iter().map(run_to_dto).collect())
}

pub async fn create_run(db: &Db, req: CreateRunRequest) -> CoreResult<RunDto> {
    if req.name.trim().is_empty() {
        return Err(CoreError::bad_request("run name is required"));
    }
    let created = repo::create_direction(
        db,
        req.issue_id,
        req.name.trim(),
        &req.tool,
        0,
        req.reason.as_deref().unwrap_or(""),
        "plan+impl",
    )
    .await
    .map_err(CoreError::from)?;
    Ok(run_to_dto(created))
}

fn workspace_to_dto(model: crate::store::entities::workspace::Model) -> WorkspaceDto {
    WorkspaceDto {
        id: model.id,
        name: model.name,
        slug: model.slug,
        created_at: model.created_at.to_string(),
    }
}

fn issue_to_dto(model: crate::store::entities::thread::Model) -> IssueDto {
    IssueDto {
        id: model.id,
        workspace_id: model.workspace_id,
        title: model.title,
        slug: model.slug,
        kind: model.kind,
        created_at: model.created_at.to_string(),
    }
}

fn run_to_dto(model: crate::store::entities::direction::Model) -> RunDto {
    RunDto {
        id: model.id,
        issue_id: model.thread_id,
        name: model.name,
        slug: model.slug,
        tool: model.tool,
        branch: model.branch,
        repo_id: model.repo_id,
        status: model.status,
        mandate: model.mandate,
        created_at: model.created_at.to_string(),
    }
}
```

- [ ] **Step 5: Add database state and command routes**

Modify `CoreServerState` in `src-tauri/src/core_api/server.rs`:

```rust
#[derive(Clone)]
pub struct CoreServerState {
    token: Arc<String>,
    events: CoreEventBus,
    db: Option<crate::store::Db>,
}
```

Update constructors:

```rust
pub fn new(token: String, events: CoreEventBus, db: crate::store::Db) -> Self {
    Self {
        token: Arc::new(token),
        events,
        db: Some(db),
    }
}

pub fn for_test(token: &str) -> Self {
    Self {
        token: Arc::new(token.to_string()),
        events: CoreEventBus::default(),
        db: None,
    }
}

fn db(&self) -> CoreResult<&crate::store::Db> {
    self.db
        .as_ref()
        .ok_or_else(|| CoreError::internal("core server database is not configured"))
}
```

Add imports:

```rust
use axum::{extract::Path, routing::post};
use super::dto::{CreateIssueRequest, CreateRunRequest, CreateWorkspaceRequest};
```

Add routes in `build_router`:

```rust
.route("/v1/workspaces", get(list_workspaces).post(create_workspace))
.route("/v1/workspaces/:workspace_id/issues", get(list_issues))
.route("/v1/issues", post(create_issue))
.route("/v1/issues/:issue_id/runs", get(list_runs))
.route("/v1/runs", post(create_run))
```

Add route handlers:

```rust
async fn list_workspaces(
    State(state): State<CoreServerState>,
    headers: HeaderMap,
) -> CoreResult<Json<Vec<super::dto::WorkspaceDto>>> {
    state.authorize(&headers)?;
    Ok(Json(super::handlers::list_workspaces(state.db()?).await?))
}

async fn create_workspace(
    State(state): State<CoreServerState>,
    headers: HeaderMap,
    Json(req): Json<CreateWorkspaceRequest>,
) -> CoreResult<Json<super::dto::WorkspaceDto>> {
    state.authorize(&headers)?;
    Ok(Json(super::handlers::create_workspace(state.db()?, req).await?))
}

async fn list_issues(
    State(state): State<CoreServerState>,
    headers: HeaderMap,
    Path(workspace_id): Path<i32>,
) -> CoreResult<Json<Vec<super::dto::IssueDto>>> {
    state.authorize(&headers)?;
    Ok(Json(super::handlers::list_issues(state.db()?, workspace_id).await?))
}

async fn create_issue(
    State(state): State<CoreServerState>,
    headers: HeaderMap,
    Json(req): Json<CreateIssueRequest>,
) -> CoreResult<Json<super::dto::IssueDto>> {
    state.authorize(&headers)?;
    Ok(Json(super::handlers::create_issue(state.db()?, req).await?))
}

async fn list_runs(
    State(state): State<CoreServerState>,
    headers: HeaderMap,
    Path(issue_id): Path<i32>,
) -> CoreResult<Json<Vec<super::dto::RunDto>>> {
    state.authorize(&headers)?;
    Ok(Json(super::handlers::list_runs(state.db()?, issue_id).await?))
}

async fn create_run(
    State(state): State<CoreServerState>,
    headers: HeaderMap,
    Json(req): Json<CreateRunRequest>,
) -> CoreResult<Json<super::dto::RunDto>> {
    state.authorize(&headers)?;
    Ok(Json(super::handlers::create_run(state.db()?, req).await?))
}
```

Update `serve` to accept `db`:

```rust
pub async fn serve(
    addr: SocketAddr,
    token: String,
    events: CoreEventBus,
    db: crate::store::Db,
) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let bound = listener.local_addr()?;
    println!(
        "{}",
        serde_json::json!({
            "type": "atlas_core_ready",
            "endpoint": format!("http://{}", bound),
            "api_version": ApiVersion::CURRENT,
            "token": token,
            "pid": std::process::id(),
        })
    );
    axum::serve(listener, build_router(CoreServerState::new(token, events, db))).await?;
    Ok(())
}
```

- [ ] **Step 6: Open the database in the server binary**

Modify `src-tauri/src/bin/atlas-core-server.rs`:

```rust
use atlas_app_lib::{core_api::{events::CoreEventBus, server}, store::Db};
use rand::RngCore;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    atlas_app_lib::detect::augment_path_from_login_shell();
    let db = Db::open_default().await?;
    let token = new_token();
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
    server::serve(addr, token, CoreEventBus::default(), db).await
}

fn new_token() -> String {
    let mut bytes = [0_u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}
```

If `detect` is not public, change `mod detect;` to `pub mod detect;` in `src-tauri/src/lib.rs`.

- [ ] **Step 7: Run command route tests and build**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test core_api_contract
cargo build --manifest-path src-tauri/Cargo.toml --bin atlas-core-server
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/core_api/handlers.rs src-tauri/src/core_api/server.rs src-tauri/src/core_api/dto.rs src-tauri/src/bin/atlas-core-server.rs src-tauri/src/lib.rs src-tauri/tests/core_api_contract.rs
git commit -m "feat(core-api): expose workspace issue run commands"
```

---

### Task 5: Expose Real Lead And Worker Session Commands

**Files:**
- Modify: `src-tauri/src/core_api/dto.rs`
- Modify: `src-tauri/src/core_api/handlers.rs`
- Modify: `src-tauri/src/core_api/server.rs`
- Modify: `src-tauri/src/lead_chat/commands.rs`
- Test: `src-tauri/tests/core_api_contract.rs`

- [ ] **Step 1: Add failing chat command DTO tests**

Append to `src-tauri/tests/core_api_contract.rs`:

```rust
use atlas_app_lib::core_api::dto::{ChatSendRequest, OpenWorkerRequest};

#[test]
fn chat_requests_encode_for_swift_client() {
    let open = OpenWorkerRequest {
        run_id: 10,
        repo_id: 0,
        lang: "en".into(),
    };
    let send = ChatSendRequest {
        session_id: 22,
        text: "Summarize the repo".into(),
    };

    let open_json = serde_json::to_string(&open).unwrap();
    let send_json = serde_json::to_string(&send).unwrap();

    assert!(open_json.contains("\"run_id\":10"));
    assert!(send_json.contains("\"session_id\":22"));
}
```

- [ ] **Step 2: Run the failing DTO test**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test core_api_contract chat_requests_encode_for_swift_client
```

Expected: FAIL because the chat request DTOs are missing.

- [ ] **Step 3: Add chat command DTOs**

Append to `src-tauri/src/core_api/dto.rs`:

```rust
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LeadSendRequest {
    pub issue_id: i32,
    pub text: String,
    pub lang: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct OpenWorkerRequest {
    pub run_id: i32,
    pub repo_id: i32,
    pub lang: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ChatSendRequest {
    pub session_id: i32,
    pub text: String,
}
```

- [ ] **Step 4: Extract core-callable chat functions**

In `src-tauri/src/lead_chat/commands.rs`, add public functions that existing Tauri commands can call without duplicating logic:

```rust
pub async fn lead_send_core(
    app: tauri::AppHandle,
    db: crate::store::Db,
    state: tauri::State<'_, super::engine::LeadChatState>,
    thread_id: i32,
    text: String,
    lang: String,
) -> Result<(), String> {
    lead_send(app, db.into(), state, thread_id, text, lang, None, None).await
}

pub async fn chat_send_core(
    app: tauri::AppHandle,
    state: tauri::State<'_, super::engine::LeadChatState>,
    session_id: i32,
    text: String,
) -> Result<(), String> {
    chat_send(app, state, session_id, text, None, None).await
}
```

If the existing command signatures cannot be called this way because of `tauri::State`, split the inner logic into private functions with plain `&Db`, `&LeadChatState`, and `&AppHandle` parameters, then have both Tauri wrappers and core handlers call those functions.

- [ ] **Step 5: Add server routes for chat commands**

In `src-tauri/src/core_api/server.rs`, add routes:

```rust
.route("/v1/lead/send", post(lead_send))
.route("/v1/lead/:issue_id/interrupt", post(lead_interrupt))
.route("/v1/workers/open", post(open_worker))
.route("/v1/chat/send", post(chat_send))
.route("/v1/chat/:session_id/interrupt", post(chat_interrupt))
```

Add explicit route handlers that return `501 Not Implemented` until Task 6 wires runtime state:

```rust
async fn lead_send(headers: HeaderMap, State(state): State<CoreServerState>) -> CoreResult<StatusCode> {
    state.authorize(&headers)?;
    Err(CoreError {
        status: StatusCode::NOT_IMPLEMENTED,
        code: "runtime_not_attached",
        message: "core server runtime state is not attached to chat commands".into(),
    })
}
```

Repeat the same explicit `runtime_not_attached` response for `lead_interrupt`, `open_worker`, `chat_send`, and `chat_interrupt`.

- [ ] **Step 6: Run DTO tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test core_api_contract chat_requests_encode_for_swift_client
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/core_api/dto.rs src-tauri/src/core_api/server.rs src-tauri/src/lead_chat/commands.rs src-tauri/tests/core_api_contract.rs
git commit -m "feat(core-api): define native chat commands"
```

---

### Task 6: Attach Runtime State To Core Server

**Files:**
- Modify: `src-tauri/src/core_api/server.rs`
- Modify: `src-tauri/src/core_api/handlers.rs`
- Modify: `src-tauri/src/bin/atlas-core-server.rs`
- Modify: `src-tauri/src/lead_chat/commands.rs`
- Test: `src-tauri/tests/core_api_contract.rs`

- [ ] **Step 1: Add runtime-state smoke test**

Append to `src-tauri/tests/core_api_contract.rs`:

```rust
#[tokio::test]
async fn runtime_free_test_state_reports_not_attached_for_chat() {
    let state = CoreServerState::for_test("secret-token");
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/send")
                .header("x-atlas-core-token", "secret-token")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"session_id":1,"text":"hello"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
}
```

- [ ] **Step 2: Run the runtime-state test**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test core_api_contract runtime_free_test_state_reports_not_attached_for_chat
```

Expected: PASS with the explicit `501` response from Task 5.

- [ ] **Step 3: Define runtime state**

In `src-tauri/src/core_api/server.rs`, add:

```rust
#[derive(Clone)]
pub struct CoreRuntime {
    pub lead_state: std::sync::Arc<crate::lead_chat::engine::LeadChatState>,
    pub out_hub: std::sync::Arc<crate::lead_chat::out_hub::LeadOutHub>,
    pub guardrails: std::sync::Arc<crate::commands::GuardrailState>,
    pub power: std::sync::Arc<crate::power::PowerGuard>,
}
```

Extend `CoreServerState`:

```rust
runtime: Option<CoreRuntime>,
```

Add:

```rust
pub fn with_runtime(mut self, runtime: CoreRuntime) -> Self {
    self.runtime = Some(runtime);
    self
}

fn runtime(&self) -> CoreResult<&CoreRuntime> {
    self.runtime
        .as_ref()
        .ok_or_else(|| CoreError {
            status: StatusCode::NOT_IMPLEMENTED,
            code: "runtime_not_attached",
            message: "core server runtime state is not attached to chat commands".into(),
        })
}
```

- [ ] **Step 4: Attach runtime in the server binary**

In `src-tauri/src/bin/atlas-core-server.rs`, create runtime state before serving:

```rust
let runtime = atlas_app_lib::core_api::server::CoreRuntime {
    lead_state: std::sync::Arc::new(atlas_app_lib::lead_chat::engine::LeadChatState::default()),
    out_hub: std::sync::Arc::new(atlas_app_lib::lead_chat::out_hub::LeadOutHub::default()),
    guardrails: std::sync::Arc::new(atlas_app_lib::commands::GuardrailState::default()),
    power: std::sync::Arc::new(atlas_app_lib::power::PowerGuard::default()),
};
server::serve_with_runtime(addr, token, CoreEventBus::default(), db, runtime).await
```

Add `serve_with_runtime` in `server.rs` by mirroring `serve` and applying `.with_runtime(runtime)` when building the state.

- [ ] **Step 5: Wire chat routes to core-callable handlers**

Replace the `runtime_not_attached` body for routes with logic that:

```rust
state.authorize(&headers)?;
let _runtime = state.runtime()?;
```

Then call the extracted functions from `lead_chat::commands` using the runtime state. If an existing function still needs `AppHandle`, add an adapter type in `lead_chat::commands.rs` named `RuntimeEmitter` with variants for Tauri and core server. The core server variant must emit through `CoreEventBus`; the Tauri variant must preserve current `AppHandle.emit` behavior.

- [ ] **Step 6: Run build and core API tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test core_api_contract
cargo build --manifest-path src-tauri/Cargo.toml --bin atlas-core-server
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/core_api/server.rs src-tauri/src/core_api/handlers.rs src-tauri/src/bin/atlas-core-server.rs src-tauri/src/lead_chat/commands.rs src-tauri/tests/core_api_contract.rs
git commit -m "feat(core-api): attach runtime state to server"
```

---

### Task 7: Add Swift Package And Core DTO Tests

**Files:**
- Create: `native/AtlasNative/Package.swift`
- Create: `native/AtlasNative/Sources/AtlasCoreClient/CoreDTOs.swift`
- Test: `native/AtlasNative/Tests/AtlasCoreClientTests/CoreClientTests.swift`

- [ ] **Step 1: Create the Swift package**

Create `native/AtlasNative/Package.swift`:

```swift
// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "AtlasNative",
    platforms: [
        .macOS(.v26)
    ],
    products: [
        .library(name: "AtlasCoreClient", targets: ["AtlasCoreClient"]),
        .executable(name: "AtlasNativeApp", targets: ["AtlasNativeApp"])
    ],
    targets: [
        .target(name: "AtlasCoreClient"),
        .executableTarget(
            name: "AtlasNativeApp",
            dependencies: ["AtlasCoreClient"]
        ),
        .testTarget(
            name: "AtlasCoreClientTests",
            dependencies: ["AtlasCoreClient"]
        ),
        .testTarget(
            name: "AtlasNativeAppTests",
            dependencies: ["AtlasNativeApp", "AtlasCoreClient"]
        )
    ]
)
```

- [ ] **Step 2: Write failing Swift DTO tests**

Create `native/AtlasNative/Tests/AtlasCoreClientTests/CoreClientTests.swift`:

```swift
import XCTest
@testable import AtlasCoreClient

final class CoreClientTests: XCTestCase {
    func testHealthDecodes() throws {
        let data = #"{"ok":true,"api_version":1,"product":"Atlas"}"#.data(using: .utf8)!
        let decoded = try JSONDecoder.atlas.decode(HealthResponse.self, from: data)

        XCTAssertTrue(decoded.ok)
        XCTAssertEqual(decoded.apiVersion, 1)
        XCTAssertEqual(decoded.product, "Atlas")
    }

    func testLeadChatTurnEventDecodes() throws {
        let data = #"{"type":"lead_chat","payload":{"event":"turn","thread_id":8,"session_id":42,"state":"busy","queued":1}}"#.data(using: .utf8)!
        let decoded = try JSONDecoder.atlas.decode(CoreEvent.self, from: data)

        guard case let .leadChat(.turn(threadId, sessionId, state, queued)) = decoded else {
            return XCTFail("expected lead chat turn")
        }
        XCTAssertEqual(threadId, 8)
        XCTAssertEqual(sessionId, 42)
        XCTAssertEqual(state, "busy")
        XCTAssertEqual(queued, 1)
    }
}
```

- [ ] **Step 3: Run the failing Swift tests**

Run:

```bash
cd native/AtlasNative && swift test --filter CoreClientTests
```

Expected: FAIL because `CoreDTOs.swift` is missing.

- [ ] **Step 4: Implement Swift DTOs**

Create `native/AtlasNative/Sources/AtlasCoreClient/CoreDTOs.swift`:

```swift
import Foundation

public extension JSONDecoder {
    static var atlas: JSONDecoder {
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return decoder
    }
}

public extension JSONEncoder {
    static var atlas: JSONEncoder {
        let encoder = JSONEncoder()
        encoder.keyEncodingStrategy = .convertToSnakeCase
        return encoder
    }
}

public struct HealthResponse: Codable, Equatable {
    public let ok: Bool
    public let apiVersion: Int
    public let product: String
}

public struct WorkspaceDTO: Codable, Identifiable, Equatable {
    public let id: Int
    public let name: String
    public let slug: String
    public let createdAt: String
}

public struct IssueDTO: Codable, Identifiable, Equatable {
    public let id: Int
    public let workspaceId: Int
    public let title: String
    public let slug: String
    public let kind: String
    public let createdAt: String
}

public struct RunDTO: Codable, Identifiable, Equatable {
    public let id: Int
    public let issueId: Int
    public let name: String
    public let slug: String
    public let tool: String
    public let branch: String
    public let repoId: Int
    public let status: String
    public let mandate: String
    public let createdAt: String
}

public struct SessionInfoDTO: Codable, Identifiable, Equatable {
    public var id: Int { sessionId }
    public let sessionId: Int
    public let repo: String
    public let worktree: String
    public let cwd: String
    public let branch: String
    public let tool: String
    public let resumed: Bool
    public let nativeId: String?
}

public struct LeadMessageDTO: Codable, Identifiable, Equatable {
    public let id: Int
    public let threadId: Int
    public let sessionId: Int?
    public let turnId: Int
    public let role: String
    public let kind: String
    public let content: String
    public let status: String
    public let createdAt: String
}

public struct SlashCommandDTO: Codable, Equatable {
    public let name: String
    public let description: String?
    public let argHint: String?
}

public enum LeadChatPushDTO: Equatable {
    case message(threadId: Int, message: LeadMessageDTO)
    case delta(threadId: Int, messageId: Int, text: String)
    case finalize(threadId: Int, messageId: Int, status: String)
    case turn(threadId: Int, sessionId: Int?, state: String, queued: Int)
    case initEvent(threadId: Int, sessionId: Int?, nativeId: String, slashCommands: [SlashCommandDTO])
    case activity(threadId: Int, sessionId: Int?, name: String, summary: String)
}

extension LeadChatPushDTO: Codable {
    private enum CodingKeys: String, CodingKey {
        case event
        case threadId
        case message
        case messageId
        case text
        case status
        case sessionId
        case state
        case queued
        case nativeId
        case slashCommands
        case name
        case summary
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let event = try container.decode(String.self, forKey: .event)
        switch event {
        case "message":
            self = .message(
                threadId: try container.decode(Int.self, forKey: .threadId),
                message: try container.decode(LeadMessageDTO.self, forKey: .message)
            )
        case "delta":
            self = .delta(
                threadId: try container.decode(Int.self, forKey: .threadId),
                messageId: try container.decode(Int.self, forKey: .messageId),
                text: try container.decode(String.self, forKey: .text)
            )
        case "finalize":
            self = .finalize(
                threadId: try container.decode(Int.self, forKey: .threadId),
                messageId: try container.decode(Int.self, forKey: .messageId),
                status: try container.decode(String.self, forKey: .status)
            )
        case "turn":
            self = .turn(
                threadId: try container.decode(Int.self, forKey: .threadId),
                sessionId: try container.decodeIfPresent(Int.self, forKey: .sessionId),
                state: try container.decode(String.self, forKey: .state),
                queued: try container.decode(Int.self, forKey: .queued)
            )
        case "init":
            self = .initEvent(
                threadId: try container.decode(Int.self, forKey: .threadId),
                sessionId: try container.decodeIfPresent(Int.self, forKey: .sessionId),
                nativeId: try container.decode(String.self, forKey: .nativeId),
                slashCommands: try container.decode([SlashCommandDTO].self, forKey: .slashCommands)
            )
        case "activity":
            self = .activity(
                threadId: try container.decode(Int.self, forKey: .threadId),
                sessionId: try container.decodeIfPresent(Int.self, forKey: .sessionId),
                name: try container.decode(String.self, forKey: .name),
                summary: try container.decode(String.self, forKey: .summary)
            )
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .event,
                in: container,
                debugDescription: "unknown lead chat event \(event)"
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case let .message(threadId, message):
            try container.encode("message", forKey: .event)
            try container.encode(threadId, forKey: .threadId)
            try container.encode(message, forKey: .message)
        case let .delta(threadId, messageId, text):
            try container.encode("delta", forKey: .event)
            try container.encode(threadId, forKey: .threadId)
            try container.encode(messageId, forKey: .messageId)
            try container.encode(text, forKey: .text)
        case let .finalize(threadId, messageId, status):
            try container.encode("finalize", forKey: .event)
            try container.encode(threadId, forKey: .threadId)
            try container.encode(messageId, forKey: .messageId)
            try container.encode(status, forKey: .status)
        case let .turn(threadId, sessionId, state, queued):
            try container.encode("turn", forKey: .event)
            try container.encode(threadId, forKey: .threadId)
            try container.encodeIfPresent(sessionId, forKey: .sessionId)
            try container.encode(state, forKey: .state)
            try container.encode(queued, forKey: .queued)
        case let .initEvent(threadId, sessionId, nativeId, slashCommands):
            try container.encode("init", forKey: .event)
            try container.encode(threadId, forKey: .threadId)
            try container.encodeIfPresent(sessionId, forKey: .sessionId)
            try container.encode(nativeId, forKey: .nativeId)
            try container.encode(slashCommands, forKey: .slashCommands)
        case let .activity(threadId, sessionId, name, summary):
            try container.encode("activity", forKey: .event)
            try container.encode(threadId, forKey: .threadId)
            try container.encodeIfPresent(sessionId, forKey: .sessionId)
            try container.encode(name, forKey: .name)
            try container.encode(summary, forKey: .summary)
        }
    }
}

public enum CoreEvent: Equatable {
    case leadChat(LeadChatPushDTO)
    case needsChanged(threadId: Int)
    case asksChanged
    case workspaceChanged(workspaceId: Int)
    case sessionStatus(sessionId: Int, status: String)
    case fatal(message: String)
}

extension CoreEvent: Codable {
    private enum CodingKeys: String, CodingKey { case type, payload }
    private enum PayloadKeys: String, CodingKey { case threadId, workspaceId, sessionId, status, message }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(String.self, forKey: .type)
        switch type {
        case "lead_chat":
            self = .leadChat(try container.decode(LeadChatPushDTO.self, forKey: .payload))
        case "needs_changed":
            let payload = try container.nestedContainer(keyedBy: PayloadKeys.self, forKey: .payload)
            self = .needsChanged(threadId: try payload.decode(Int.self, forKey: .threadId))
        case "asks_changed":
            self = .asksChanged
        case "workspace_changed":
            let payload = try container.nestedContainer(keyedBy: PayloadKeys.self, forKey: .payload)
            self = .workspaceChanged(workspaceId: try payload.decode(Int.self, forKey: .workspaceId))
        case "session_status":
            let payload = try container.nestedContainer(keyedBy: PayloadKeys.self, forKey: .payload)
            self = .sessionStatus(
                sessionId: try payload.decode(Int.self, forKey: .sessionId),
                status: try payload.decode(String.self, forKey: .status)
            )
        case "fatal":
            let payload = try container.nestedContainer(keyedBy: PayloadKeys.self, forKey: .payload)
            self = .fatal(message: try payload.decode(String.self, forKey: .message))
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .type,
                in: container,
                debugDescription: "unknown core event \(type)"
            )
        }
    }
}
```

- [ ] **Step 5: Run Swift tests**

Run:

```bash
cd native/AtlasNative && swift test --filter CoreClientTests
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add native/AtlasNative/Package.swift native/AtlasNative/Sources/AtlasCoreClient/CoreDTOs.swift native/AtlasNative/Tests/AtlasCoreClientTests/CoreClientTests.swift
git commit -m "feat(native): add Swift core DTOs"
```

---

### Task 8: Implement Swift CoreClient And CoreProcess

**Files:**
- Create: `native/AtlasNative/Sources/AtlasCoreClient/CoreClient.swift`
- Create: `native/AtlasNative/Sources/AtlasCoreClient/CoreProcess.swift`
- Modify: `native/AtlasNative/Tests/AtlasCoreClientTests/CoreClientTests.swift`
- Test: `native/AtlasNative/Tests/AtlasCoreClientTests/CoreProcessTests.swift`

- [ ] **Step 1: Add failing client tests**

Append to `native/AtlasNative/Tests/AtlasCoreClientTests/CoreClientTests.swift`:

```swift
func testCoreClientAddsTokenHeader() async throws {
    let transport = RecordingTransport(responseData: #"{"ok":true,"api_version":1,"product":"Atlas"}"#.data(using: .utf8)!)
    let client = CoreClient(baseURL: URL(string: "http://127.0.0.1:3333")!, token: "secret", transport: transport)

    _ = try await client.health()

    XCTAssertEqual(transport.lastRequest?.value(forHTTPHeaderField: "x-atlas-core-token"), "secret")
    XCTAssertEqual(transport.lastRequest?.url?.path, "/v1/health")
}
```

Add this test helper to the same file:

```swift
final class RecordingTransport: CoreTransport {
    private let responseData: Data
    private(set) var lastRequest: URLRequest?

    init(responseData: Data) {
        self.responseData = responseData
    }

    func data(for request: URLRequest) async throws -> (Data, URLResponse) {
        lastRequest = request
        let response = HTTPURLResponse(
            url: request.url!,
            statusCode: 200,
            httpVersion: nil,
            headerFields: nil
        )!
        return (responseData, response)
    }
}
```

- [ ] **Step 2: Run the failing client tests**

Run:

```bash
cd native/AtlasNative && swift test --filter CoreClientTests/testCoreClientAddsTokenHeader
```

Expected: FAIL because `CoreClient` and `CoreTransport` are missing.

- [ ] **Step 3: Implement CoreClient**

Create `native/AtlasNative/Sources/AtlasCoreClient/CoreClient.swift`:

```swift
import Foundation

public protocol CoreTransport: AnyObject {
    func data(for request: URLRequest) async throws -> (Data, URLResponse)
}

extension URLSession: CoreTransport {}

public enum CoreClientError: Error, Equatable {
    case invalidResponse
    case httpStatus(Int, String)
}

public final class CoreClient {
    private let baseURL: URL
    private let token: String
    private let transport: CoreTransport

    public init(baseURL: URL, token: String, transport: CoreTransport = URLSession.shared) {
        self.baseURL = baseURL
        self.token = token
        self.transport = transport
    }

    public func health() async throws -> HealthResponse {
        try await get("/v1/health")
    }

    public func listWorkspaces() async throws -> [WorkspaceDTO] {
        try await get("/v1/workspaces")
    }

    public func listIssues(workspaceId: Int) async throws -> [IssueDTO] {
        try await get("/v1/workspaces/\(workspaceId)/issues")
    }

    public func listRuns(issueId: Int) async throws -> [RunDTO] {
        try await get("/v1/issues/\(issueId)/runs")
    }

    private func get<T: Decodable>(_ path: String) async throws -> T {
        var request = URLRequest(url: baseURL.appending(path: path))
        request.httpMethod = "GET"
        request.setValue(token, forHTTPHeaderField: "x-atlas-core-token")
        let (data, response) = try await transport.data(for: request)
        try validate(response: response, data: data)
        return try JSONDecoder.atlas.decode(T.self, from: data)
    }

    private func post<RequestBody: Encodable, ResponseBody: Decodable>(
        _ path: String,
        body: RequestBody
    ) async throws -> ResponseBody {
        var request = URLRequest(url: baseURL.appending(path: path))
        request.httpMethod = "POST"
        request.setValue(token, forHTTPHeaderField: "x-atlas-core-token")
        request.setValue("application/json", forHTTPHeaderField: "content-type")
        request.httpBody = try JSONEncoder.atlas.encode(body)
        let (data, response) = try await transport.data(for: request)
        try validate(response: response, data: data)
        return try JSONDecoder.atlas.decode(ResponseBody.self, from: data)
    }

    private func validate(response: URLResponse, data: Data) throws {
        guard let http = response as? HTTPURLResponse else {
            throw CoreClientError.invalidResponse
        }
        guard (200..<300).contains(http.statusCode) else {
            let message: String
            if let decoded = String(data: data, encoding: .utf8) {
                message = decoded
            } else {
                message = ""
            }
            throw CoreClientError.httpStatus(http.statusCode, message)
        }
    }
}
```

- [ ] **Step 4: Add CoreProcess tests**

Create `native/AtlasNative/Tests/AtlasCoreClientTests/CoreProcessTests.swift`:

```swift
import XCTest
@testable import AtlasCoreClient

final class CoreProcessTests: XCTestCase {
    func testReadyLineParses() throws {
        let line = #"{"type":"atlas_core_ready","endpoint":"http://127.0.0.1:49152","api_version":1,"token":"abc","pid":123}"#
        let ready = try CoreProcess.ReadyLine.parse(line)

        XCTAssertEqual(ready.endpoint.absoluteString, "http://127.0.0.1:49152")
        XCTAssertEqual(ready.apiVersion, 1)
        XCTAssertEqual(ready.token, "abc")
        XCTAssertEqual(ready.pid, 123)
    }
}
```

- [ ] **Step 5: Implement CoreProcess handshake parsing**

Create `native/AtlasNative/Sources/AtlasCoreClient/CoreProcess.swift`:

```swift
import Foundation

public final class CoreProcess {
    public struct ReadyLine: Decodable, Equatable {
        public let endpoint: URL
        public let apiVersion: Int
        public let token: String
        public let pid: Int

        private enum CodingKeys: String, CodingKey {
            case type
            case endpoint
            case apiVersion
            case token
            case pid
        }

        public static func parse(_ line: String) throws -> ReadyLine {
            let data = Data(line.utf8)
            let decoded = try JSONDecoder.atlas.decode(ReadyLine.self, from: data)
            guard decoded.apiVersion == 1 else {
                throw CoreProcessError.unsupportedApiVersion(decoded.apiVersion)
            }
            return decoded
        }
    }

    public enum CoreProcessError: Error, Equatable {
        case unsupportedApiVersion(Int)
        case missingExecutable(URL)
        case processExitedBeforeReady
    }

    public init() {}
}
```

- [ ] **Step 6: Run Swift tests**

Run:

```bash
cd native/AtlasNative && swift test
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add native/AtlasNative/Sources/AtlasCoreClient/CoreClient.swift native/AtlasNative/Sources/AtlasCoreClient/CoreProcess.swift native/AtlasNative/Tests/AtlasCoreClientTests/CoreClientTests.swift native/AtlasNative/Tests/AtlasCoreClientTests/CoreProcessTests.swift
git commit -m "feat(native): add core client and process handshake"
```

---

### Task 9: Add Swift AppStore Reducer

**Files:**
- Create: `native/AtlasNative/Sources/AtlasNativeApp/AppStore.swift`
- Create: `native/AtlasNative/Tests/AtlasNativeAppTests/AppStoreTests.swift`

- [ ] **Step 1: Write failing AppStore tests**

Create `native/AtlasNative/Tests/AtlasNativeAppTests/AppStoreTests.swift`:

```swift
import XCTest
import AtlasCoreClient
@testable import AtlasNativeApp

final class AppStoreTests: XCTestCase {
    @MainActor
    func testLeadDeltaAppendsToMessageText() {
        let store = AppStore(core: nil)
        store.messages[1] = [
            LeadMessageDTO(
                id: 10,
                threadId: 1,
                sessionId: nil,
                turnId: 1,
                role: "assistant",
                kind: "text",
                content: #"{"text":"Hel"}"#,
                status: "streaming",
                createdAt: "2026-06-16T00:00:00Z"
            )
        ]

        store.reduce(.leadChat(.delta(threadId: 1, messageId: 10, text: "lo")))

        XCTAssertEqual(store.messages[1]?.first?.content, #"{"text":"Hello"}"#)
    }

    @MainActor
    func testTurnEventUpdatesBusyState() {
        let store = AppStore(core: nil)
        store.reduce(.leadChat(.turn(threadId: 3, sessionId: 44, state: "busy", queued: 2)))

        XCTAssertEqual(store.workerTurns[44]?.state, "busy")
        XCTAssertEqual(store.workerTurns[44]?.queued, 2)
    }
}
```

- [ ] **Step 2: Run failing AppStore tests**

Run:

```bash
cd native/AtlasNative && swift test --filter AppStoreTests
```

Expected: FAIL because `AppStore` is missing.

- [ ] **Step 3: Implement AppStore reducer**

Create `native/AtlasNative/Sources/AtlasNativeApp/AppStore.swift`:

```swift
import Foundation
import Observation
import AtlasCoreClient

@Observable
@MainActor
public final class AppStore {
    public struct TurnState: Equatable {
        public var state: String
        public var queued: Int
    }

    public var workspaces: [WorkspaceDTO] = []
    public var issues: [IssueDTO] = []
    public var runs: [RunDTO] = []
    public var selectedWorkspaceId: Int?
    public var selectedIssueId: Int?
    public var selectedRunId: Int?
    public var messages: [Int: [LeadMessageDTO]] = [:]
    public var leadTurns: [Int: TurnState] = [:]
    public var workerTurns: [Int: TurnState] = [:]
    public var fatalMessage: String?

    private let core: CoreClient?

    public init(core: CoreClient?) {
        self.core = core
    }

    public func reduce(_ event: CoreEvent) {
        switch event {
        case let .leadChat(push):
            reduce(push)
        case let .fatal(message):
            fatalMessage = message
        case .asksChanged, .needsChanged, .workspaceChanged, .sessionStatus:
            break
        }
    }

    private func reduce(_ push: LeadChatPushDTO) {
        switch push {
        case let .message(threadId, message):
            messages[threadId, default: []].append(message)
        case let .delta(threadId, messageId, text):
            guard var rows = messages[threadId],
                  let idx = rows.firstIndex(where: { $0.id == messageId }) else {
                return
            }
            rows[idx] = rows[idx].appendingTextDelta(text)
            messages[threadId] = rows
        case let .finalize(threadId, messageId, status):
            guard var rows = messages[threadId],
                  let idx = rows.firstIndex(where: { $0.id == messageId }) else {
                return
            }
            rows[idx] = rows[idx].withStatus(status)
            messages[threadId] = rows
        case let .turn(threadId, sessionId, state, queued):
            let turn = TurnState(state: state, queued: queued)
            if let sessionId {
                workerTurns[sessionId] = turn
            } else {
                leadTurns[threadId] = turn
            }
        case .initEvent, .activity:
            break
        }
    }
}

private extension LeadMessageDTO {
    func appendingTextDelta(_ delta: String) -> LeadMessageDTO {
        let old = decodedText()
        return replacingContentText(old + delta)
    }

    func withStatus(_ newStatus: String) -> LeadMessageDTO {
        LeadMessageDTO(
            id: id,
            threadId: threadId,
            sessionId: sessionId,
            turnId: turnId,
            role: role,
            kind: kind,
            content: content,
            status: newStatus,
            createdAt: createdAt
        )
    }

    func replacingContentText(_ text: String) -> LeadMessageDTO {
        let escaped = text
            .replacingOccurrences(of: #"\"#, with: #"\\"#)
            .replacingOccurrences(of: #"""#, with: #"\""#)
        return LeadMessageDTO(
            id: id,
            threadId: threadId,
            sessionId: sessionId,
            turnId: turnId,
            role: role,
            kind: kind,
            content: #"{"text":"\#(escaped)"}"#,
            status: status,
            createdAt: createdAt
        )
    }

    func decodedText() -> String {
        guard let data = content.data(using: .utf8),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let text = object["text"] as? String else {
            return content
        }
        return text
    }
}
```

- [ ] **Step 4: Run AppStore tests**

Run:

```bash
cd native/AtlasNative && swift test --filter AppStoreTests
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add native/AtlasNative/Sources/AtlasNativeApp/AppStore.swift native/AtlasNative/Tests/AtlasNativeAppTests/AppStoreTests.swift
git commit -m "feat(native): add app state reducer"
```

---

### Task 10: Build Native Shell, Sidebar, Issue And Run Views

**Files:**
- Create: `native/AtlasNative/Sources/AtlasNativeApp/AtlasNativeApp.swift`
- Create: `native/AtlasNative/Sources/AtlasNativeApp/Views/AppShell.swift`
- Create: `native/AtlasNative/Sources/AtlasNativeApp/Views/WorkspaceSidebar.swift`
- Create: `native/AtlasNative/Sources/AtlasNativeApp/Views/IssueRunViews.swift`

- [ ] **Step 1: Create SwiftUI app entry**

Create `native/AtlasNative/Sources/AtlasNativeApp/AtlasNativeApp.swift`:

```swift
import SwiftUI
import AtlasCoreClient

@main
struct AtlasNativeApp: App {
    @State private var store = AppStore(core: nil)

    var body: some Scene {
        WindowGroup("Atlas") {
            AppShell()
                .environment(store)
        }
        .commands {
            CommandGroup(replacing: .newItem) {
                Button("New Issue") {}
                    .keyboardShortcut("n", modifiers: [.command])
            }
        }
    }
}
```

- [ ] **Step 2: Create the app shell**

Create `native/AtlasNative/Sources/AtlasNativeApp/Views/AppShell.swift`:

```swift
import SwiftUI

struct AppShell: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        @Bindable var store = store
        NavigationSplitView {
            WorkspaceSidebar()
        } content: {
            IssueListView(selectedIssueId: $store.selectedIssueId)
        } detail: {
            if let issueId = store.selectedIssueId {
                RunListView(issueId: issueId, selectedRunId: $store.selectedRunId)
            } else {
                ContentUnavailableView("Select an Issue", systemImage: "tray")
            }
        }
        .toolbar {
            ToolbarItemGroup {
                Button {
                } label: {
                    Label("New Issue", systemImage: "square.and.pencil")
                }
                Button {
                } label: {
                    Label("Needs", systemImage: "questionmark.bubble")
                }
            }
        }
    }
}
```

- [ ] **Step 3: Create workspace sidebar**

Create `native/AtlasNative/Sources/AtlasNativeApp/Views/WorkspaceSidebar.swift`:

```swift
import SwiftUI

struct WorkspaceSidebar: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        @Bindable var store = store
        List(selection: $store.selectedWorkspaceId) {
            Section("Workspaces") {
                ForEach(store.workspaces) { workspace in
                    Label(workspace.name, systemImage: "rectangle.3.group")
                        .tag(Optional(workspace.id))
                }
            }

            Section("Focus") {
                Label("Needs You", systemImage: "questionmark.circle")
                Label("Issues", systemImage: "list.bullet.rectangle")
                Label("Settings", systemImage: "gearshape")
            }
        }
        .navigationTitle("Atlas")
    }
}
```

- [ ] **Step 4: Create issue and run views**

Create `native/AtlasNative/Sources/AtlasNativeApp/Views/IssueRunViews.swift`:

```swift
import SwiftUI

struct IssueListView: View {
    @Environment(AppStore.self) private var store
    @Binding var selectedIssueId: Int?

    var body: some View {
        List(selection: $selectedIssueId) {
            ForEach(store.issues) { issue in
                VStack(alignment: .leading, spacing: 3) {
                    Text(issue.title)
                        .font(.headline)
                    Text(issue.kind)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .tag(Optional(issue.id))
            }
        }
        .navigationTitle("Issues")
    }
}

struct RunListView: View {
    @Environment(AppStore.self) private var store
    let issueId: Int
    @Binding var selectedRunId: Int?

    var body: some View {
        List(selection: $selectedRunId) {
            ForEach(store.runs.filter { $0.issueId == issueId }) { run in
                HStack {
                    VStack(alignment: .leading, spacing: 3) {
                        Text(run.name)
                            .font(.headline)
                        Text(run.tool)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    Spacer()
                    Text(run.status)
                        .font(.caption)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 3)
                        .background(.thinMaterial, in: Capsule())
                }
                .tag(Optional(run.id))
            }
        }
        .navigationTitle("Runs")
    }
}
```

- [ ] **Step 5: Build the native app package**

Run:

```bash
cd native/AtlasNative && swift build
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add native/AtlasNative/Sources/AtlasNativeApp/AtlasNativeApp.swift native/AtlasNative/Sources/AtlasNativeApp/Views/AppShell.swift native/AtlasNative/Sources/AtlasNativeApp/Views/WorkspaceSidebar.swift native/AtlasNative/Sources/AtlasNativeApp/Views/IssueRunViews.swift
git commit -m "feat(native): add macOS shell"
```

---

### Task 11: Add Chat, Needs, Ask, And Settings MVP Views

**Files:**
- Create: `native/AtlasNative/Sources/AtlasNativeApp/Views/ChatViews.swift`
- Create: `native/AtlasNative/Sources/AtlasNativeApp/Views/NeedsAskViews.swift`
- Create: `native/AtlasNative/Sources/AtlasNativeApp/Views/SettingsViews.swift`
- Modify: `native/AtlasNative/Sources/AtlasNativeApp/Views/AppShell.swift`

- [ ] **Step 1: Create chat views**

Create `native/AtlasNative/Sources/AtlasNativeApp/Views/ChatViews.swift`:

```swift
import SwiftUI
import AtlasCoreClient

struct ChatView: View {
    @Environment(AppStore.self) private var store
    let threadId: Int
    let sessionId: Int?
    @State private var draft = ""

    var rows: [LeadMessageDTO] {
        store.messages[threadId, default: []].filter { message in
            sessionId == nil || message.sessionId == sessionId
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 10) {
                    ForEach(rows) { row in
                        MessageRow(message: row)
                    }
                }
                .padding()
            }

            Divider()

            HStack(alignment: .bottom) {
                TextEditor(text: $draft)
                    .frame(minHeight: 48, maxHeight: 120)
                    .clipShape(RoundedRectangle(cornerRadius: 10))

                Button {
                    draft = ""
                } label: {
                    Label("Send", systemImage: "paperplane.fill")
                }
                .keyboardShortcut(.return, modifiers: [.command])
                .disabled(draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
            .padding()
            .background(.bar)
        }
    }
}

struct MessageRow: View {
    let message: LeadMessageDTO

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(message.role.capitalized)
                .font(.caption)
                .foregroundStyle(.secondary)
            Text(decodedText)
                .textSelection(.enabled)
        }
        .padding(10)
        .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 10))
    }

    private var decodedText: String {
        guard let data = message.content.data(using: .utf8),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let text = object["text"] as? String else {
            return message.content
        }
        return text
    }
}
```

- [ ] **Step 2: Create Needs and Ask views**

Create `native/AtlasNative/Sources/AtlasNativeApp/Views/NeedsAskViews.swift`:

```swift
import SwiftUI

struct NeedsView: View {
    var body: some View {
        ContentUnavailableView("No Items Need You", systemImage: "checkmark.circle")
    }
}

struct PermissionAskSheet: View {
    let title: String
    let action: String
    let onAnswer: (String) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text(title)
                .font(.headline)
            Text(action)
                .font(.body)
                .textSelection(.enabled)
            HStack {
                Button("Deny", role: .destructive) { onAnswer("deny") }
                Spacer()
                Button("Allow") { onAnswer("allow") }
                Button("Always") { onAnswer("always") }
                Button("Full") { onAnswer("full") }
            }
        }
        .padding()
        .frame(width: 460)
    }
}
```

- [ ] **Step 3: Create settings view**

Create `native/AtlasNative/Sources/AtlasNativeApp/Views/SettingsViews.swift`:

```swift
import SwiftUI

struct SettingsView: View {
    @State private var defaultTool = "codex"
    @State private var dangerousMode = false
    @State private var keepAwake = true
    @State private var idleCap = 30.0
    @State private var wallCap = 180.0

    var body: some View {
        Form {
            Picker("Default Tool", selection: $defaultTool) {
                Text("Codex").tag("codex")
                Text("Claude").tag("claude")
                Text("OpenCode").tag("opencode")
            }
            Toggle("Dangerous Mode", isOn: $dangerousMode)
            Toggle("Keep Mac Awake While Running", isOn: $keepAwake)
            LabeledContent("Idle Cap") {
                Slider(value: $idleCap, in: 0...180, step: 5)
                    .frame(width: 180)
            }
            LabeledContent("Wall Cap") {
                Slider(value: $wallCap, in: 0...480, step: 15)
                    .frame(width: 180)
            }
        }
        .formStyle(.grouped)
        .navigationTitle("Settings")
    }
}
```

- [ ] **Step 4: Wire chat into app shell detail**

In `AppShell.swift`, replace the `RunListView` detail branch with:

```swift
if let issueId = store.selectedIssueId, let runId = store.selectedRunId {
    ChatView(threadId: issueId, sessionId: runId)
} else if let issueId = store.selectedIssueId {
    RunListView(issueId: issueId, selectedRunId: $store.selectedRunId)
} else {
    ContentUnavailableView("Select an Issue", systemImage: "tray")
}
```

- [ ] **Step 5: Build native app**

Run:

```bash
cd native/AtlasNative && swift build
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add native/AtlasNative/Sources/AtlasNativeApp/Views/ChatViews.swift native/AtlasNative/Sources/AtlasNativeApp/Views/NeedsAskViews.swift native/AtlasNative/Sources/AtlasNativeApp/Views/SettingsViews.swift native/AtlasNative/Sources/AtlasNativeApp/Views/AppShell.swift
git commit -m "feat(native): add chat needs ask settings views"
```

---

### Task 12: Add Native Preflight And Developer Runbook

**Files:**
- Create: `scripts/native-preflight.sh`
- Modify: `scripts/preflight.sh`
- Create: `docs/native-macos-migration.md`

- [ ] **Step 1: Create native preflight script**

Create `scripts/native-preflight.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

git diff --check
cargo test --manifest-path src-tauri/Cargo.toml --test core_api_contract
cargo build --manifest-path src-tauri/Cargo.toml --bin atlas-core-server

if [ -d native/AtlasNative ]; then
  (cd native/AtlasNative && swift test)
fi
```

- [ ] **Step 2: Make native preflight executable and run it**

Run:

```bash
chmod +x scripts/native-preflight.sh
scripts/native-preflight.sh
```

Expected: PASS.

- [ ] **Step 3: Add opt-in native gate to preflight**

At the end of `scripts/preflight.sh`, before the final success message, add:

```bash
if [ "${ATLAS_NATIVE_PREFLIGHT:-0}" = "1" ]; then
  scripts/native-preflight.sh
fi
```

- [ ] **Step 4: Add migration runbook**

Create `docs/native-macos-migration.md`:

```markdown
# Native macOS Migration Runbook

## Build Core Server

```bash
cargo build --manifest-path src-tauri/Cargo.toml --bin atlas-core-server
```

## Run Core Server Manually

```bash
./src-tauri/target/debug/atlas-core-server
```

The first stdout line is a JSON handshake containing `endpoint`, `api_version`, `token`, and `pid`.

## Test Native Swift Package

```bash
cd native/AtlasNative
swift test
swift run AtlasNativeApp
```

## Run Native Preflight

```bash
scripts/native-preflight.sh
```

## Full Product Verification Path

1. Launch `AtlasNativeApp`.
2. Start or connect to `atlas-core-server`.
3. Confirm `/v1/health` returns API version `1`.
4. List workspaces.
5. Create or select a workspace.
6. Create an issue.
7. Create a run.
8. Start a Claude, Codex, or OpenCode session.
9. Send a message.
10. Observe streaming response and activity updates.
11. Trigger or inspect Needs/Ask handling.
12. Answer a permission ask.
13. Interrupt or stop a session.
14. Quit the app.
15. Confirm no orphaned core or agent child processes remain.
16. Reopen and confirm persisted state hydrates.
```

- [ ] **Step 5: Run final preflight commands**

Run:

```bash
scripts/native-preflight.sh
ATLAS_NATIVE_PREFLIGHT=1 pnpm preflight:quick
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add scripts/native-preflight.sh scripts/preflight.sh docs/native-macos-migration.md
git commit -m "chore(native): add migration preflight"
```

---

## Final Verification

Run these commands after all tasks are complete:

```bash
git diff --check
cargo test --manifest-path src-tauri/Cargo.toml --test core_api_contract
cargo build --manifest-path src-tauri/Cargo.toml --bin atlas-core-server
cd native/AtlasNative && swift test && swift build
cd ../..
scripts/native-preflight.sh
ATLAS_NATIVE_PREFLIGHT=1 pnpm preflight:quick
```

Expected:

- Rust core API contract tests pass.
- `atlas-core-server` builds.
- Swift tests pass.
- Swift package builds.
- Native preflight passes.
- Existing quick preflight passes with native preflight enabled.

Manual verification must follow the product path in `docs/native-macos-migration.md` before the MVP is called complete.
