//! Tauri commands for the chat engine. The lead's engine is keyed by
//! `-thread_id`; chat-mode runs key by `session_id`.

use super::engine::{self, EngineRef, LeadChatState};
use crate::store::{repo, Db};
use tauri::{AppHandle, Manager, State};

fn lead_key(thread_id: i32) -> i64 {
    -(thread_id as i64)
}

/// What a (re)opened run session looks like to the frontend.
#[derive(serde::Serialize, Clone)]
pub struct SessionInfo {
    pub session_id: i32,
    pub run_dir: String,
    pub cwd: String,
    pub tool: String,
    pub resumed: bool,
    pub native_id: Option<String>,
}

const BASE_PROMPT: &str = "You are the coordinator for this task in Atlas, a local Agent App. \
Start by calling get_task to read what the human is asking. Discuss the goal, constraints, \
and next step with the human. You may answer directly, ask a concise clarifying question, \
or suggest a named run for a focused agent session. Treat Atlas as a general task and agent \
conversation base. When a decision belongs to the human, ask it directly in chat. Keep the \
conversation practical and grounded in the current task.";

/// The conversational lead prompt. The lead coordinates the current task with
/// the human and does not assume the workspace is a code repository.
pub fn lead_prompt() -> String {
    BASE_PROMPT.to_string()
}

/// Agent-output language directive (ARCHITECTURE §4.8, layer 2). Appended to the
/// lead prompt / worker brief so prose follows the operator's UI language while
/// preserving domain-specific terms as written. Empty for English (the default).
pub fn lang_directive(lang: &str) -> &'static str {
    if lang == "zh" {
        "\n\n用中文撰写所有自然语言产出(计划、摘要、bus 消息和给用户的说明);保留产品名、工具名、文件名、命令和用户提供的专有术语。"
    } else {
        ""
    }
}

/// System prompt for the IM Concierge engine. Concierge is scoped to the
/// current IM conversation, not to one task lead.
/// It never plans or writes; it only reads Atlas state via the `atlas_global` MCP
/// and answers / triggers actions on the human's behalf. Bilingual: language
/// follows the caller's lang (defaults to zh — IM bridge fixes it that way).
pub fn concierge_prompt(lang: &str) -> String {
    let body = if lang == "zh" {
        "你是 Atlas 桌面端的助理（Concierge），用户从某个飞书会话找你。Atlas 桌面端正在运行，\
真实状态都在 atlas_global MCP 工具里——回答任何关于任务列表、任务、待办、agent 提问的问题前，\
必须先用工具核实（list_workspaces / list_tasks / pending_needs_you / task_status），不要凭印象作答。\n\
如果用户消息里带有 feishu_chat_id，那就是当前飞书会话的 chat_id；只有用户语义明确要求为某个已有任务创建、打开或继续飞书 topic 时，才可把这个 chat_id 传给 ensure_task_topic。\n\
\n\
工具一览：\n\
- list_workspaces / list_tasks / task_status / pending_needs_you —— 只读，先用它们摸清状态。\n\
- answer_permission(ask_id, verdict) —— 用户明确告诉你判决时才代答；不确定就先用 pending_needs_you 列出再问用户。\n\
- answer_question(thread_id, ask_id, text) —— 转达用户对 agent 提问的回答。\n\
- message_lead(thread_id, text) —— 把用户的话喂给某个任务的 lead。\n\
- ensure_task_topic(thread_id, chat_id) —— 当用户语义明确要为某个已有任务创建/打开/继续飞书 topic 时调用；普通聊天不要调用。\n\
- create_task(workspace_id, title, kind) —— 新建任务；kind 默认 task。\n\
\n\
不要做的事：\n\
- 不要替用户决定需要桌面确认或高风险的事——把状态报清楚，请用户去桌面处理。\n\
- 不要臆造任务列表、任务、ask 的细节；找不到就说没找到，不要编。\n\
- 不要在不可逆动作之前自行批准权限请求（answer_permission allow/full）——除非用户在这条消息里明确同意。\n\
\n\
回复风格：简短中文，用 markdown 列表/编号；引用任务时带 thread_id；引用 ask 时带 ask_id。"
    } else {
        "You are Atlas's desktop Concierge, reached by the user through one Feishu conversation. Atlas is \
running on the user's desktop and authoritative state lives behind the `atlas_global` MCP \
tools — ALWAYS verify with the tools before answering anything about task lists, tasks, \
pending asks, or agent questions (list_workspaces / list_tasks / pending_needs_you / \
task_status). Never answer from your imagination.\n\
If the user's message includes feishu_chat_id, that is the current Feishu chat_id; only pass it to ensure_task_topic when the user semantically asks to create, open, or continue a Feishu topic for an existing task.\n\
\n\
Tools:\n\
- list_workspaces / list_tasks / task_status / pending_needs_you — read-only; lead with these.\n\
- answer_permission(ask_id, verdict) — only when the user explicitly tells you the verdict; otherwise list pending asks and ask.\n\
- answer_question(thread_id, ask_id, text) — relay the user's answer to an agent's open question.\n\
- message_lead(thread_id, text) — deliver the user's message into a specific task's lead.\n\
- ensure_task_topic(thread_id, chat_id) — call only when the user semantically asks to create/open/continue a Feishu topic for an existing task; do not call for ordinary chat.\n\
- create_task(workspace_id, title, kind) — create a new task (kind defaults to task).\n\
\n\
Do not:\n\
- Decide things that require the desktop or carry high risk — report the state and ask the user to go to the desktop.\n\
- Invent task-list / task / ask details; if you can't find it, say so.\n\
- Pre-approve irreversible permission asks (answer_permission allow/full) unless the user explicitly consents in this message.\n\
\n\
Style: short, markdown bullets / numbered lists; mention thread_id when citing a task, ask_id when citing an ask."
    };
    format!("{}{}", body, lang_directive(lang))
}

/// Get-or-create the lead's engine for a thread: scratch cwd, planner MCP +
/// ask bridge injections, conversational lead prompt as the system prompt.
/// Mirrors the retired PTY `plan_with_lead` wiring.
/// Public so the IM bridge can drive the same lead engine when a飞书 thread
/// message lands on a bound task thread.
///
/// Concierge branch (`t.kind == "concierge"`): swap planner MCP →
/// `atlas_global` MCP and the lead prompt → `concierge_prompt(lang)`. Everything
/// else (cwd, ask hook, skills) stays identical so this engine survives
/// app restarts and obeys per-task permissions the same way.
pub async fn lead_engine(
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
    let cwd = crate::paths::lead_home(thread_id)?;
    std::fs::create_dir_all(&cwd)?;
    // git-init so claude's session store (keyed by cwd) behaves like any other
    // cwd; harmless if it already exists.
    let _ = crate::git::command()
        .args(["init", "-q"])
        .current_dir(&cwd)
        .status();
    let base = app.state::<crate::BusBase>().0.clone();
    let is_concierge = t.kind == "concierge";
    let inj = if is_concierge {
        crate::bus::inject::inject_global(&base, &t.lead_tool, &cwd)
    } else {
        crate::bus::inject::inject_planner(&base, thread_id, &t.lead_tool, &cwd)
    };
    let ask = crate::bus::inject::inject_ask_hook(&base, thread_id, "lead", &t.lead_tool, &cwd);
    crate::skills::inject_for(db, t.workspace_id, &cwd).await;
    let native_id = repo::lead_native_id(db, thread_id).await.ok().flatten();
    let computer_use_enabled = if native_id.is_some() {
        repo::lead_computer_use_enabled(db, thread_id)
            .await
            .ok()
            .flatten()
            .unwrap_or(false)
    } else {
        let enabled = crate::computer_use::settings::enabled(db)
            .await
            .unwrap_or(false);
        let _ = repo::set_lead_computer_use_enabled(db, thread_id, enabled).await;
        enabled
    };
    let computer = crate::computer_use::inject::inject_for_enabled(
        app,
        &t.lead_tool,
        &cwd,
        computer_use_enabled,
    );
    let mut extra = ask.args;
    extra.extend(inj.args);
    extra.extend(computer.args);
    let system_prompt = if is_concierge {
        concierge_prompt(lang)
    } else {
        format!(
            "{}{}",
            lead_prompt(),
            lang_directive(lang),
        )
    };
    let inner = engine::EngineInner {
        thread_id,
        tool: t.lead_tool.clone(),
        session_id: None,
        cwd,
        extra_args: extra,
        system_prompt,
        native_id,
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
    engine::send(
        &app,
        &db,
        &eng,
        &text,
        to_pairs(images),
        files.unwrap_or_default(),
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn lead_interrupt(app: AppHandle, thread_id: i32) -> Result<(), String> {
    if let Some(eng) = app.state::<LeadChatState>().get(lead_key(thread_id)) {
        engine::interrupt(&app, &eng)
            .await
            .map_err(|e| e.to_string())?;
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
    engine::ensure_running(&app, &db, &eng)
        .await
        .map_err(|e| e.to_string())
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
    pub slash_commands: Vec<crate::lead_chat::proto::SlashCmd>,
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
            cwd: crate::paths::atlas_home()
                .map(|h| {
                    h.join("leads")
                        .join(thread_id.to_string())
                        .to_string_lossy()
                        .into_owned()
                })
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

/// Discover the slash commands a session's CLI actually supports — never
/// hardcoded. claude: the live `initialize` list the engine already holds;
/// opencode: GET /command off a lazily-started `opencode serve`, keyed by the
/// session's project cwd; codex: none (headless `exec` has no slash surface).
/// `session_id` selects a worker; `thread_id` selects the (claude) lead.
#[tauri::command]
pub async fn discover_slash(
    app: AppHandle,
    db: State<'_, Db>,
    thread_id: Option<i32>,
    session_id: Option<i32>,
) -> Result<Vec<crate::lead_chat::proto::SlashCmd>, String> {
    let state = app.state::<LeadChatState>();
    if let Some(sid) = session_id {
        let Some(sess) = repo::get_session(&db, sid).await.map_err(|e| e.to_string())? else {
            return Ok(vec![]);
        };
        return Ok(match sess.tool.as_str() {
            "opencode" => crate::opencode::discover_commands(&sess.cwd).await,
            "claude" => match state.get(sid as i64) {
                Some(eng) => eng.lock().await.slash_commands.clone(),
                None => vec![],
            },
            _ => vec![], // codex: no headless slash commands
        });
    }
    // Lead console (always claude): the engine's live initialize list.
    if let Some(tid) = thread_id {
        if let Some(eng) = state.get(lead_key(tid)) {
            return Ok(eng.lock().await.slash_commands.clone());
        }
    }
    Ok(vec![])
}

#[tauri::command]
pub async fn list_lead_messages(
    db: State<'_, Db>,
    thread_id: i32,
) -> Result<Vec<crate::store::entities::lead_message::Model>, String> {
    repo::list_lead_messages(&db, thread_id)
        .await
        .map_err(|e| e.to_string())
}

// ───────────────────── chat-mode runs ─────────────────────
//
// Every run (claude/codex/opencode) runs on the engine: an Atlas-owned chat
// timeline in the SessionView, with per-tool wire dialects (engine::per_turn).
// Each session remains takeover-able in the user's own terminal via its
// native id.

/// Spawn (or resume) a chat-mode run with thread-bus MCP + ask bridge and the
/// assembled brief as the first user message of an Atlas-owned conversation.
#[tauri::command]
pub async fn chat_open_run(
    app: AppHandle,
    db: State<'_, Db>,
    direction_id: i32,
    lang: Option<String>,
) -> Result<SessionInfo, String> {
    chat_open_run_impl(
        &app,
        &db,
        direction_id,
        lang.as_deref().unwrap_or("en"),
    )
    .await
    .map_err(|e| e.to_string())
}

async fn chat_open_run_impl(
    app: &AppHandle,
    db: &Db,
    direction_id: i32,
    lang: &str,
) -> anyhow::Result<SessionInfo> {
    use sea_orm::EntityTrait;
    let dir = crate::store::entities::direction::Entity::find_by_id(direction_id)
        .one(&db.0)
        .await?
        .ok_or_else(|| anyhow::anyhow!("direction not found"))?;
    let thread = repo::get_thread(db, dir.thread_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("thread not found"))?;
    let workspace = crate::store::entities::workspace::Entity::find_by_id(thread.workspace_id)
        .one(&db.0)
        .await?
        .ok_or_else(|| anyhow::anyhow!("workspace not found"))?;
    let cwd = crate::paths::run_home(&workspace.slug, &thread.slug, &dir.slug)?;
    let cwd_str = cwd.to_string_lossy().to_string();

    // Resume an earlier conversation when this run already captured one.
    let prior = repo::latest_session_for(db, direction_id).await?;
    let native = prior.as_ref().and_then(|s| s.native_session_id.clone());
    let resumed = native.is_some();
    let new_session_computer_use = if resumed {
        false
    } else {
        crate::computer_use::settings::enabled(db)
            .await
            .unwrap_or(false)
    };
    let sess = match prior {
        Some(s) if s.native_session_id.is_some() => s,
        _ => {
            repo::create_session_with_computer_use(
                db,
                direction_id,
                &dir.tool,
                &cwd_str,
                new_session_computer_use,
            )
            .await?
        }
    };

    let base = app.state::<crate::BusBase>().0.clone();
    let inj = crate::bus::inject::inject(
        &base,
        dir.thread_id,
        &direction_id.to_string(),
        &dir.tool,
        &cwd,
    );
    let ask = crate::bus::inject::inject_ask_hook(
        &base,
        dir.thread_id,
        &direction_id.to_string(),
        &dir.tool,
        &cwd,
    );
    crate::skills::inject_for(db, thread.workspace_id, &cwd).await;
    let computer = crate::computer_use::inject::inject_for_enabled(
        app,
        &dir.tool,
        &cwd,
        sess.computer_use_enabled,
    );
    let mut extra = ask.args;
    extra.extend(inj.args);
    extra.extend(computer.args);

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
        let mut brief = crate::brief::assemble(db, direction_id)
            .await
            .unwrap_or_default();
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
        run_dir: cwd_str.clone(),
        cwd: cwd_str,
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
    let inj = crate::bus::inject::inject(
        &base,
        dir.thread_id,
        &sess.direction_id.to_string(),
        &sess.tool,
        &cwd,
    );
    let ask = crate::bus::inject::inject_ask_hook(
        &base,
        dir.thread_id,
        &sess.direction_id.to_string(),
        &sess.tool,
        &cwd,
    );
    if let Ok(Some(th)) = repo::get_thread(db, dir.thread_id).await {
        crate::skills::inject_for(db, th.workspace_id, &cwd).await;
    }
    let computer = crate::computer_use::inject::inject_for_enabled(
        app,
        &sess.tool,
        &cwd,
        sess.computer_use_enabled,
    );
    let mut extra = ask.args;
    extra.extend(inj.args);
    extra.extend(computer.args);
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
    let eng = worker_engine(&app, &db, session_id)
        .await
        .map_err(|e| e.to_string())?;
    engine::send(
        &app,
        &db,
        &eng,
        &text,
        to_pairs(images),
        files.unwrap_or_default(),
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn chat_interrupt(app: AppHandle, session_id: i32) -> Result<(), String> {
    if let Some(eng) = app.state::<LeadChatState>().get(session_id as i64) {
        engine::interrupt(&app, &eng)
            .await
            .map_err(|e| e.to_string())?;
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
pub async fn flag_session_skill_refresh(
    app: AppHandle,
    db: State<'_, Db>,
    session_id: i32,
) -> Result<(), String> {
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
pub async fn flag_lead_skill_refresh(
    app: AppHandle,
    db: State<'_, Db>,
    thread_id: i32,
) -> Result<(), String> {
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
