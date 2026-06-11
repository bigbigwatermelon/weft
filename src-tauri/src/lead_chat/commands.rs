//! Tauri commands for the chat engine. The lead's engine is keyed by
//! `-thread_id`; chat-mode workers (phase 2) key by `session_id`.

use super::engine::{self, EngineRef, LeadChatState};
use crate::store::{repo, Db};
use tauri::{AppHandle, Manager, State};

fn lead_key(thread_id: i32) -> i64 {
    -(thread_id as i64)
}

/// What a (re)dispatched worker session looks like to the frontend.
#[derive(serde::Serialize, Clone)]
pub struct SessionInfo {
    pub session_id: i32,
    pub repo: String,
    pub worktree: String,
    pub branch: String,
    pub tool: String,
    pub resumed: bool,
    pub native_id: Option<String>,
}

const BASE_PROMPT: &str = "You are the lead for this thread in weft — the human's main collaborator. \
Start by greeting briefly and using the weft_planner MCP tools to orient: call get_task to read \
what's being asked, and get_repo_map to learn each repo's role and the cross-repo dependency graph. \
Then DISCUSS the requirement and approach with the human; ask clarifying questions when it matters. \
You do not write code, and you do not plan the directions' implementations — each worker plans its \
own direction. Your job is to converge the scope and ASSIGN ROLES. When you and the human have \
converged on how to split the work, call propose_directions with a short rationale and the directions \
(name, the ONE repo each writes, reason, mandate); only list repos each direction must WRITE \
(reads are free). Pick mandate per direction: plan+impl (default — the worker plans first) or \
impl-only (small/fully-specified — build straight away). The human reviews and confirms in weft; you \
can re-propose after more discussion. Prefer splitting frontend/backend/shared work to run in \
parallel, owner of a shared contract first.";

/// Sentinel usage directives appended to the lead prompt. Each subsequent task
/// (Task 3-5) keeps growing this block, so it lives as its own const for easy
/// editing — raw string keeps quotes/JSON readable.
const SENTINEL_DIRECTIVES: &str = r#"When the user has no suitable repo for the work, render a single-line action card by outputting exactly:
<weft:action_card>{"title":"...","body":"...","actions":[{"id":"...","label":"...","kind":"add"|"new"|"clone"}]}</weft:action_card>
Each action's kind must be one of "add" (import existing folder), "new" (create a new repo), or "clone" (clone a remote URL). Use language matching the user's locale for title/body/label. To query the full repo list when the <repo_state> hint is truncated, emit on its own line: <weft:list_repos/> You will receive the reply as <weft:list_repos_result>{...}</weft:list_repos_result>. After a user finishes an action, you will receive <weft:repo_action>{...}</weft:repo_action> with status: ok/error/cancelled."#;

/// The conversational lead prompt. The lead is the human's main collaborator for
/// the thread: it discusses the work, and the plan EMERGES from that conversation
/// rather than from a one-shot propose-and-exit. It proposes when (and only when)
/// the human has converged with it, and may re-propose after more discussion.
pub fn lead_prompt() -> String {
    format!("{BASE_PROMPT}\n\n{SENTINEL_DIRECTIVES}")
}

/// Agent-output language directive (ARCHITECTURE §4.8, layer 2). Appended to the
/// lead prompt / worker brief so prose follows the operator's UI language; code
/// and identifiers always stay English. Empty for English (the default).
pub fn lang_directive(lang: &str) -> &'static str {
    if lang == "zh" {
        "\n\n用中文撰写所有自然语言产出(计划、摘要、bus 消息、PR/commit 文案);代码、标识符与技术约定始终用英文。"
    } else {
        ""
    }
}

/// Get-or-create the lead's engine for a thread: scratch cwd, planner MCP +
/// ask bridge injections, conversational lead prompt as the system prompt.
/// Mirrors the retired PTY `plan_with_lead` wiring (spec §2).
async fn lead_engine(
    app: &AppHandle,
    db: &Db,
    thread_id: i32,
    lang: &str,
) -> anyhow::Result<EngineRef> {
    let state = app.state::<LeadChatState>();
    if let Some(e) = state.get(lead_key(thread_id)) {
        return Ok(e);
    }
    let t = repo::get_thread(db, thread_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("thread not found"))?;
    let cwd = crate::paths::weft_home()?.join("leads").join(thread_id.to_string());
    std::fs::create_dir_all(&cwd)?;
    // git-init so claude's session store (keyed by cwd) behaves like any other
    // cwd; harmless if it already exists.
    let _ = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(&cwd)
        .status();
    let base = app.state::<crate::BusBase>().0.clone();
    let inj = crate::bus::inject::inject_planner(&base, thread_id, &t.lead_tool, &cwd);
    let ask = crate::bus::inject::inject_ask_hook(&base, thread_id, "lead", &t.lead_tool, &cwd);
    crate::skills::inject_for(db, t.workspace_id, &cwd).await;
    let mut extra = ask.args;
    extra.extend(inj.args);
    let system_prompt = {
        let repo_state =
            crate::lead_chat::repo_state::render_repo_state(db, Some(t.workspace_id)).await?;
        format!("{}{}\n\n{}", lead_prompt(), lang_directive(lang), repo_state)
    };
    let inner = engine::EngineInner {
        thread_id,
        tool: t.lead_tool.clone(),
        session_id: None,
        cwd,
        extra_args: extra,
        system_prompt,
        native_id: repo::lead_native_id(db, thread_id).await.ok().flatten(),
        slash_commands: vec![],
        turn: Default::default(),
        turn_id: repo::next_turn_id(db, thread_id).await.unwrap_or(1) - 1,
        ask_dir: "lead".into(),
        clock: Default::default(),
        child: None,
        stdin: None,
        current: None,
        interrupting: false,
        generation: 0,
        pending_skill_refresh: false,
    };
    let eng: EngineRef = std::sync::Arc::new(tokio::sync::Mutex::new(inner));
    Ok(state.get_or_insert(lead_key(thread_id), eng))
}

/// One inbound image attachment from the composer (pasted or picked).
#[derive(serde::Deserialize)]
pub struct ImageIn {
    pub media_type: String,
    /// base64 payload, no data-URI prefix.
    pub data: String,
}

fn to_pairs(images: Option<Vec<ImageIn>>) -> Vec<(String, String)> {
    images
        .unwrap_or_default()
        .into_iter()
        .map(|i| (i.media_type, i.data))
        .collect()
}

#[tauri::command]
pub async fn lead_send(
    app: AppHandle,
    db: State<'_, Db>,
    thread_id: i32,
    text: String,
    lang: Option<String>,
    images: Option<Vec<ImageIn>>,
    files: Option<Vec<String>>,
) -> Result<(), String> {
    let eng = lead_engine(&app, &db, thread_id, lang.as_deref().unwrap_or("en"))
        .await
        .map_err(|e| e.to_string())?;
    engine::send(&app, &db, &eng, &text, to_pairs(images), files.unwrap_or_default())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn lead_interrupt(app: AppHandle, thread_id: i32) -> Result<(), String> {
    if let Some(eng) = app.state::<LeadChatState>().get(lead_key(thread_id)) {
        engine::interrupt(&app, &eng).await.map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Make sure the lead engine exists + its process runs (called on console open
/// so the init event delivers slash_commands without waiting for a first send).
#[tauri::command]
pub async fn lead_ensure(
    app: AppHandle,
    db: State<'_, Db>,
    thread_id: i32,
    lang: Option<String>,
) -> Result<(), String> {
    let eng = lead_engine(&app, &db, thread_id, lang.as_deref().unwrap_or("en"))
        .await
        .map_err(|e| e.to_string())?;
    engine::ensure_running(&app, &db, &eng).await.map_err(|e| e.to_string())
}

/// Stop the lead engine (terminal takeover: the session must have one writer).
#[tauri::command]
pub async fn lead_stop(app: AppHandle, thread_id: i32) -> Result<(), String> {
    if let Some(eng) = app.state::<LeadChatState>().get(lead_key(thread_id)) {
        engine::stop(&app, &eng).await;
    }
    Ok(())
}

#[derive(serde::Serialize)]
pub struct LeadStateInfo {
    pub state: String,
    pub queued: usize,
    pub native_id: Option<String>,
    pub slash_commands: Vec<String>,
    pub cwd: String,
}

#[tauri::command]
pub async fn lead_state(
    app: AppHandle,
    db: State<'_, Db>,
    thread_id: i32,
) -> Result<LeadStateInfo, String> {
    let eng = app.state::<LeadChatState>().get(lead_key(thread_id));
    match eng {
        None => Ok(LeadStateInfo {
            state: "stopped".into(),
            queued: 0,
            native_id: repo::lead_native_id(&db, thread_id).await.ok().flatten(),
            slash_commands: vec![],
            cwd: crate::paths::weft_home()
                .map(|h| h.join("leads").join(thread_id.to_string()).to_string_lossy().into_owned())
                .unwrap_or_default(),
        }),
        Some(e) => {
            let mut i = e.lock().await;
            let alive = i
                .child
                .as_mut()
                .map(|c| c.try_wait().ok().flatten().is_none())
                .unwrap_or(false);
            Ok(LeadStateInfo {
                state: if !alive {
                    "stopped"
                } else if i.turn.busy {
                    "busy"
                } else {
                    "idle"
                }
                .into(),
                queued: i.turn.queue.len(),
                native_id: i.native_id.clone(),
                slash_commands: i.slash_commands.clone(),
                cwd: i.cwd.to_string_lossy().into_owned(),
            })
        }
    }
}

#[tauri::command]
pub async fn list_lead_messages(
    db: State<'_, Db>,
    thread_id: i32,
) -> Result<Vec<crate::store::entities::lead_message::Model>, String> {
    let msgs = repo::list_lead_messages(&db, thread_id).await.map_err(|e| e.to_string())?;
    if !msgs.iter().any(|m| m.kind != "meta") {
        // Legacy thread: lazily import the old PTY lead's jsonl transcript once.
        if let Ok(n) = import_legacy(&db, thread_id).await {
            if n > 0 {
                return repo::list_lead_messages(&db, thread_id).await.map_err(|e| e.to_string());
            }
        }
    }
    Ok(msgs)
}

// ───────────────────── chat-mode workers ─────────────────────
//
// Every worker (claude/codex/opencode) runs on the engine: a weft-owned chat
// timeline in the SessionView, with per-tool wire dialects (engine::per_turn).
// Each session remains takeover-able in the user's own terminal via its
// native id.

/// Spawn (or resume) a chat-mode worker for a (direction, repo) slot: worktree
/// cwd, thread-bus MCP + ask bridge, the assembled brief as the first user
/// message of a weft-owned conversation.
#[tauri::command]
pub async fn chat_open_worker(
    app: AppHandle,
    db: State<'_, Db>,
    direction_id: i32,
    repo_id: i32,
    lang: Option<String>,
) -> Result<SessionInfo, String> {
    chat_open_worker_impl(&app, &db, direction_id, repo_id, lang.as_deref().unwrap_or("en"))
        .await
        .map_err(|e| e.to_string())
}

async fn chat_open_worker_impl(
    app: &AppHandle,
    db: &Db,
    direction_id: i32,
    repo_id: i32,
    lang: &str,
) -> anyhow::Result<SessionInfo> {
    use sea_orm::EntityTrait;
    let wt = repo::worktree_for(db, direction_id, repo_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("no materialized worktree for that direction+repo"))?;
    let dir = crate::store::entities::direction::Entity::find_by_id(direction_id)
        .one(&db.0)
        .await?
        .ok_or_else(|| anyhow::anyhow!("direction not found"))?;
    let cwd = std::path::PathBuf::from(&wt.path);

    // Resume an earlier conversation when this slot already captured one.
    let prior = repo::latest_session_for(db, direction_id, repo_id).await?;
    let native = prior.as_ref().and_then(|s| s.native_session_id.clone());
    let resumed = native.is_some();
    let sess = match prior {
        Some(s) if s.native_session_id.is_some() => s,
        _ => repo::create_session(db, direction_id, repo_id, &dir.tool, &wt.path).await?,
    };

    let base = app.state::<crate::BusBase>().0.clone();
    let inj = crate::bus::inject::inject(&base, dir.thread_id, &direction_id.to_string(), &dir.tool, &cwd);
    let ask = crate::bus::inject::inject_ask_hook(&base, dir.thread_id, &direction_id.to_string(), &dir.tool, &cwd);
    if let Ok(Some(th)) = repo::get_thread(db, dir.thread_id).await {
        crate::skills::inject_for(db, th.workspace_id, &cwd).await;
    }
    let mut extra = ask.args;
    extra.extend(inj.args);

    let state = app.state::<LeadChatState>();
    let key = sess.id as i64;
    let eng = match state.get(key) {
        Some(e) => e,
        None => {
            let inner = engine::EngineInner {
                thread_id: dir.thread_id,
                tool: dir.tool.clone(),
                session_id: Some(sess.id),
                cwd,
                extra_args: extra,
                system_prompt: String::new(),
                native_id: native.clone(),
                slash_commands: vec![],
                turn: Default::default(),
                turn_id: repo::next_turn_id(db, dir.thread_id).await.unwrap_or(1) - 1,
                ask_dir: direction_id.to_string(),
                clock: Default::default(),
                child: None,
                stdin: None,
                current: None,
                interrupting: false,
                generation: 0,
                pending_skill_refresh: false,
            };
            let e: EngineRef = std::sync::Arc::new(tokio::sync::Mutex::new(inner));
            state.get_or_insert(key, e)
        }
    };
    engine::ensure_running(app, db, &eng).await?;

    // A fresh conversation gets its brief as the opening message (the brief is
    // a message, not a system prompt).
    if !resumed {
        let mut brief = crate::brief::assemble(db, direction_id).await.unwrap_or_default();
        if !brief.trim().is_empty() {
            brief.push_str(lang_directive(lang));
            engine::send(app, db, &eng, &brief, vec![], vec![]).await?;
        }
    }
    // Dispatch enters the mandate's first phase: plan+impl workers start by
    // planning their direction (the brief says so); impl-only build right away.
    // Resume keeps whatever status the agent last reported.
    if !resumed {
        let phase = if repo::normalize_mandate(&dir.mandate) == "impl-only" {
            "working"
        } else {
            "planning"
        };
        let _ = repo::set_direction_status(db, direction_id, phase).await;
    }

    Ok(SessionInfo {
        session_id: sess.id,
        repo: wt.path.clone(),
        worktree: wt.path,
        branch: wt.branch,
        tool: dir.tool,
        resumed,
        native_id: native,
    })
}

/// Get-or-rebuild a worker's engine from its session row — so a chat worker
/// survives app restarts the same way the lead does: sending resumes it.
async fn worker_engine(app: &AppHandle, db: &Db, session_id: i32) -> anyhow::Result<EngineRef> {
    let state = app.state::<LeadChatState>();
    if let Some(e) = state.get(session_id as i64) {
        return Ok(e);
    }
    use sea_orm::EntityTrait;
    let sess = repo::get_session(db, session_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("no such session"))?;
    let dir = crate::store::entities::direction::Entity::find_by_id(sess.direction_id)
        .one(&db.0)
        .await?
        .ok_or_else(|| anyhow::anyhow!("direction not found"))?;
    let cwd = std::path::PathBuf::from(&sess.cwd);
    let base = app.state::<crate::BusBase>().0.clone();
    let inj = crate::bus::inject::inject(&base, dir.thread_id, &sess.direction_id.to_string(), &sess.tool, &cwd);
    let ask = crate::bus::inject::inject_ask_hook(&base, dir.thread_id, &sess.direction_id.to_string(), &sess.tool, &cwd);
    if let Ok(Some(th)) = repo::get_thread(db, dir.thread_id).await {
        crate::skills::inject_for(db, th.workspace_id, &cwd).await;
    }
    let mut extra = ask.args;
    extra.extend(inj.args);
    let inner = engine::EngineInner {
        thread_id: dir.thread_id,
        tool: sess.tool.clone(),
        session_id: Some(sess.id),
        cwd,
        extra_args: extra,
        system_prompt: String::new(),
        native_id: sess.native_session_id.clone(),
        slash_commands: vec![],
        turn: Default::default(),
        turn_id: repo::next_turn_id(db, dir.thread_id).await.unwrap_or(1) - 1,
        ask_dir: sess.direction_id.to_string(),
        clock: Default::default(),
        child: None,
        stdin: None,
        current: None,
        interrupting: false,
        generation: 0,
        pending_skill_refresh: false,
    };
    let e: EngineRef = std::sync::Arc::new(tokio::sync::Mutex::new(inner));
    Ok(state.get_or_insert(session_id as i64, e))
}

#[tauri::command]
pub async fn chat_send(
    app: AppHandle,
    db: State<'_, Db>,
    session_id: i32,
    text: String,
    images: Option<Vec<ImageIn>>,
    files: Option<Vec<String>>,
) -> Result<(), String> {
    let eng = worker_engine(&app, &db, session_id).await.map_err(|e| e.to_string())?;
    engine::send(&app, &db, &eng, &text, to_pairs(images), files.unwrap_or_default())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn chat_interrupt(app: AppHandle, session_id: i32) -> Result<(), String> {
    if let Some(eng) = app.state::<LeadChatState>().get(session_id as i64) {
        engine::interrupt(&app, &eng).await.map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn chat_stop(app: AppHandle, session_id: i32) -> Result<(), String> {
    if let Some(eng) = app.state::<LeadChatState>().get(session_id as i64) {
        engine::stop(&app, &eng).await;
    }
    Ok(())
}

/// idle-time skill refresh (worker): re-inject the workspace's enabled skills
/// into the live session's cwd and flag the engine so the next send silently
/// restarts the resident process to pick them up. No-op if the engine is gone.
#[tauri::command]
pub async fn flag_session_skill_refresh(app: AppHandle, db: State<'_, Db>, session_id: i32) -> Result<(), String> {
    let Some(eng) = app.state::<LeadChatState>().get(session_id as i64) else {
        return Ok(());
    };
    let (thread_id, cwd) = {
        let g = eng.lock().await;
        (g.thread_id, g.cwd.clone())
    };
    if let Ok(Some(th)) = repo::get_thread(&db, thread_id).await {
        crate::skills::inject_for(&db, th.workspace_id, &cwd).await;
    }
    eng.lock().await.pending_skill_refresh = true;
    Ok(())
}

/// idle-time skill refresh (lead). Same as the worker variant, keyed by thread.
#[tauri::command]
pub async fn flag_lead_skill_refresh(app: AppHandle, db: State<'_, Db>, thread_id: i32) -> Result<(), String> {
    let Some(eng) = app.state::<LeadChatState>().get(lead_key(thread_id)) else {
        return Ok(());
    };
    let cwd = { eng.lock().await.cwd.clone() };
    if let Ok(Some(th)) = repo::get_thread(&db, thread_id).await {
        crate::skills::inject_for(&db, th.workspace_id, &cwd).await;
    }
    eng.lock().await.pending_skill_refresh = true;
    Ok(())
}

/// One-shot import of a legacy PTY-lead transcript (the tool's own jsonl,
/// parsed by the sidecar) into lead_message rows. Best-effort: any failure
/// leaves the timeline empty — history remains reachable in a terminal.
async fn import_legacy(db: &Db, thread_id: i32) -> anyhow::Result<usize> {
    let cwd = crate::paths::weft_home()?.join("leads").join(thread_id.to_string());
    if !cwd.exists() {
        return Ok(0);
    }
    let events = crate::sidecar::read_transcript(&cwd, "claude").await;
    let mut n = 0usize;
    for e in events {
        match e {
            crate::sidecar::NormEvent::Message { role, text, .. } => {
                let content = serde_json::json!({ "text": text }).to_string();
                repo::insert_lead_message(db, thread_id, None, 1, &role, "text", &content, "complete")
                    .await?;
                n += 1;
            }
            crate::sidecar::NormEvent::Tool { name, summary, .. } => {
                let content = serde_json::json!({ "name": name, "summary": summary }).to_string();
                repo::insert_lead_message(db, thread_id, None, 1, "assistant", "tool", &content, "complete")
                    .await?;
                n += 1;
            }
        }
    }
    Ok(n)
}

/// Frontend callback after a repo onboarding action card finishes (add /
/// new / clone). Wraps the payload in `<weft:repo_action>…</weft:repo_action>`
/// and delivers it as an invisible user turn so the agent can react without
/// the result polluting the visible timeline. Respects the turn machine:
/// mid-turn clicks get queued and flush at the next boundary instead of
/// shoving JSON between in-flight protocol lines. Does NOT ensure_running —
/// a click into a dead lead is a no-op (we don't want a card click to
/// resurrect a stopped engine behind the user's back).
#[tauri::command]
pub async fn post_lead_tool_result(
    app: AppHandle,
    thread_id: i32,
    payload: serde_json::Value,
) -> Result<(), String> {
    let json = serde_json::to_string(&payload).map_err(|e| e.to_string())?;
    let text = format!("<weft:repo_action>{json}</weft:repo_action>");
    let key = lead_key(thread_id);
    match app.state::<LeadChatState>().get(key) {
        Some(eng) => {
            // TODO: frontend currently can't distinguish delivered vs queued vs
            // no-engine. Acceptable now — action cards are visual + ephemeral
            // — revisit if "card click did nothing" debugging gets noisy.
            let mut inner = eng.lock().await;
            let out = engine::Outgoing { text, images: vec![], tracked: false };
            if inner.turn.try_begin_send() {
                inner.turn_id += 1;
                inner.clock.begin_turn();
                engine::write_user(&mut inner, &out).await;
            } else {
                inner.turn.queue.push_back(out);
            }
        }
        None => {
            eprintln!("[weft] post_lead_tool_result: no lead engine for thread {thread_id}");
        }
    }
    Ok(())
}
