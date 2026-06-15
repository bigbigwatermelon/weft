# Agent App Decoding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn Atlas's default experience into a generic local multi-provider Agent App by removing the repo/worktree/diff/check assumptions from the primary path.

**Architecture:** Keep the existing Tauri/React app, store tables, provider adapters, chat engine, Ask Bridge, skills, settings, backup, and IM bridge. Add a repo-less run path on top of the existing `thread` and `direction` tables, then switch the UI and prompts to use that path by default while leaving coding modules present but not called.

**Tech Stack:** Tauri v2, Rust 2021, SeaORM, SQLite/SQLCipher, React 19, TypeScript, Vite, i18next.

---

## File Structure

- Modify `src-tauri/src/paths.rs`: add app-managed run cwd helpers under `~/.atlas/workspaces/...`.
- Modify `src-tauri/src/store/repo.rs`: keep `thread`/`direction`, but add tests proving `repo_id = 0` works as a generic run.
- Modify `src-tauri/src/commands.rs`: add `create_run`, make `create_direction` only materialize worktrees for non-zero repo ids.
- Modify `src-tauri/src/lead_chat/commands.rs`: replace coding lead prompt, stop injecting repo state into the lead prompt, add `chat_open_run`, and let repo-less sessions use app-managed cwd.
- Modify `src-tauri/src/brief.rs`: make repo-less briefs generic instead of saying "write repos" or "checks".
- Modify `src-tauri/src/bus/server.rs`: make lead planner tools generic by default.
- Modify `src-tauri/src/lib.rs`: register new Tauri commands.
- Modify `src/lib/types.ts`: add `Task`/`Run` aliases and make `SessionInfo` carry `cwd`.
- Modify `src/lib/api.ts`: add `createRun` and `chatOpenRun`.
- Modify `src/state/store.tsx`: add generic run creation/opening and remove default auto-check/review dispatch from the main path.
- Modify `src/board/WorkspaceHome.tsx`, `src/board/ThreadBoard.tsx`, `src/nav/WorkspaceNav.tsx`, `src/nav/AppTopBar.tsx`, `src/session/SessionView.tsx`, `src/components/FirstRunOnboarding.tsx`: remove repo-first default UI.
- Modify `src/i18n/en.ts` and `src/i18n/zh.ts`: replace visible coding copy with task/run/agent app copy.
- Add or update Rust tests in `src-tauri/src/paths.rs`, `src-tauri/src/store/repo.rs`, `src-tauri/src/brief.rs`, `src-tauri/tests/lead_prompt.rs`.

---

### Task 1: Add Generic Run CWD Helpers

**Files:**
- Modify: `src-tauri/src/paths.rs`

- [ ] **Step 1: Write failing path tests**

Add these tests inside `#[cfg(test)] mod tests` in `src-tauri/src/paths.rs`:

```rust
#[test]
fn run_home_is_namespaced_under_atlas_home() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = std::env::temp_dir().join(format!(
        "atlas-paths-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::env::set_var("ATLAS_HOME", &tmp);

    let p = run_home("people-ops", "draft-offer", "main").unwrap();
    assert!(p.ends_with("workspaces/people-ops/tasks/draft-offer/runs/main"));
    assert!(p.is_dir(), "run_home should create the directory");

    let _ = std::fs::remove_dir_all(&tmp);
    std::env::remove_var("ATLAS_HOME");
}

#[test]
fn run_home_rejects_empty_segments() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let err = run_home("workspace", "", "run").unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
}
```

- [ ] **Step 2: Run the path tests and verify they fail**

Run:

```bash
cd src-tauri && cargo test paths::tests::run_home --lib
```

Expected: FAIL because `run_home` is not defined.

- [ ] **Step 3: Implement the path helpers**

Add this code after `worktree_home()` in `src-tauri/src/paths.rs`:

```rust
fn checked_segment(segment: &str, label: &str) -> std::io::Result<String> {
    let trimmed = segment.trim();
    if trimmed.is_empty() || trimmed.contains('/') || trimmed.contains('\\') {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid {label} segment"),
        ));
    }
    Ok(trimmed.to_string())
}

/// ~/.atlas/workspaces/<workspace>/tasks/<task>/runs/<run>
pub fn run_home(workspace_slug: &str, task_slug: &str, run_slug: &str) -> std::io::Result<PathBuf> {
    let ws = checked_segment(workspace_slug, "workspace")?;
    let task = checked_segment(task_slug, "task")?;
    let run = checked_segment(run_slug, "run")?;
    let dir = atlas_home()?
        .join("workspaces")
        .join(ws)
        .join("tasks")
        .join(task)
        .join("runs")
        .join(run);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}
```

- [ ] **Step 4: Run the path tests and verify they pass**

Run:

```bash
cd src-tauri && cargo test paths::tests::run_home --lib
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/paths.rs
git commit -m "feat(paths): add generic run home"
```

---

### Task 2: Add Repo-Less Run Commands and Session Startup

**Files:**
- Modify: `src-tauri/src/store/repo.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lead_chat/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add a store regression test for repo-less runs**

Add this test inside the existing `#[cfg(test)] mod tests` in `src-tauri/src/store/repo.rs`:

```rust
#[tokio::test]
async fn repo_less_direction_can_back_a_generic_session() {
    let db = Db::connect("sqlite::memory:").await.unwrap();
    let ws = create_workspace(&db, "People Ops").await.unwrap();
    let t = create_thread(&db, ws.id, "Draft offer email", "task", "codex")
        .await
        .unwrap();

    let d = create_direction(&db, t.id, "Main run", "codex", 0, "", "plan+impl")
        .await
        .unwrap();
    assert_eq!(d.repo_id, 0);
    assert!(direction_repo_of(&db, d.id).await.unwrap().is_none());

    let s = create_session(&db, d.id, 0, "codex", "/tmp/atlas-run")
        .await
        .unwrap();
    let latest = latest_session_for(&db, d.id, 0).await.unwrap().unwrap();
    assert_eq!(latest.id, s.id);
    assert_eq!(latest.cwd, "/tmp/atlas-run");
}
```

- [ ] **Step 2: Run the store test**

Run:

```bash
cd src-tauri && cargo test store::repo::tests::repo_less_direction_can_back_a_generic_session --lib
```

Expected: PASS. If it fails, the failure should identify the current repo-bound assumption to remove before continuing.

- [ ] **Step 3: Add the generic `create_run` command**

In `src-tauri/src/commands.rs`, add this command after `create_direction`:

```rust
#[tauri::command]
pub async fn create_run(
    db: State<'_, Db>,
    thread_id: i32,
    name: String,
    tool: String,
    reason: Option<String>,
) -> R<entities::direction::Model> {
    repo::create_direction(
        &db,
        thread_id,
        &name,
        &tool,
        0,
        reason.as_deref().unwrap_or(""),
        "plan+impl",
    )
    .await
    .map_err(e)
}
```

- [ ] **Step 4: Make the coding `create_direction` command skip materialization for `repo_id = 0`**

Change `create_direction` in `src-tauri/src/commands.rs` to:

```rust
#[tauri::command]
pub async fn create_direction(
    db: State<'_, Db>,
    thread_id: i32,
    name: String,
    tool: String,
    repo_id: i32,
    reason: String,
    mandate: Option<String>,
) -> R<entities::direction::Model> {
    let dir = repo::create_direction(
        &db,
        thread_id,
        &name,
        &tool,
        repo_id,
        &reason,
        mandate.as_deref().unwrap_or("plan+impl"),
    )
    .await
    .map_err(e)?;
    if repo_id != 0 {
        materialize::materialize_direction(&db, dir.id)
            .await
            .map_err(e)?;
    }
    Ok(dir)
}
```

- [ ] **Step 5: Add `cwd` to `SessionInfo`**

In `src-tauri/src/lead_chat/commands.rs`, change `SessionInfo` to:

```rust
#[derive(serde::Serialize, Clone)]
pub struct SessionInfo {
    pub session_id: i32,
    pub repo: String,
    pub worktree: String,
    pub cwd: String,
    pub branch: String,
    pub tool: String,
    pub resumed: bool,
    pub native_id: Option<String>,
}
```

- [ ] **Step 6: Add a repo-less `chat_open_run` command**

In `src-tauri/src/lead_chat/commands.rs`, add this public command above `chat_open_worker`:

```rust
#[tauri::command]
pub async fn chat_open_run(
    app: AppHandle,
    db: State<'_, Db>,
    direction_id: i32,
    lang: Option<String>,
) -> Result<SessionInfo, String> {
    chat_open_worker_impl(
        &app,
        &db,
        direction_id,
        0,
        lang.as_deref().unwrap_or("en"),
    )
    .await
    .map_err(|e| e.to_string())
}
```

- [ ] **Step 7: Let `chat_open_worker_impl` use app-managed cwd for `repo_id = 0`**

Replace the beginning of `chat_open_worker_impl` through session creation with this structure:

```rust
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

    let (cwd, branch) = if repo_id == 0 {
        (
            crate::paths::run_home(&workspace.slug, &thread.slug, &dir.slug)?,
            String::new(),
        )
    } else {
        let wt = repo::worktree_for(db, direction_id, repo_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("no materialized worktree for that direction+repo"))?;
        (std::path::PathBuf::from(&wt.path), wt.branch)
    };
    let cwd_str = cwd.to_string_lossy().to_string();

    let prior = repo::latest_session_for(db, direction_id, repo_id).await?;
    let native = prior.as_ref().and_then(|s| s.native_session_id.clone());
    let resumed = native.is_some();
    let sess = match prior {
        Some(s) if s.native_session_id.is_some() => s,
        _ => repo::create_session(db, direction_id, repo_id, &dir.tool, &cwd_str).await?,
    };
```

At the end of `chat_open_worker_impl`, return:

```rust
    Ok(SessionInfo {
        session_id: sess.id,
        repo: cwd_str.clone(),
        worktree: cwd_str.clone(),
        cwd: cwd_str,
        branch,
        tool: dir.tool,
        resumed,
        native_id: native,
    })
```

- [ ] **Step 8: Register the new commands**

In `src-tauri/src/lib.rs`, add these entries to the `tauri::generate_handler!` list:

```rust
commands::create_run,
lead_chat::commands::chat_open_run,
```

Place `commands::create_run` next to `commands::create_direction`, and `chat_open_run` next to `chat_open_worker`.

- [ ] **Step 9: Run targeted Rust checks**

Run:

```bash
cd src-tauri && cargo test store::repo::tests::repo_less_direction_can_back_a_generic_session --lib
cd src-tauri && cargo test paths::tests::run_home --lib
```

Expected: PASS.

- [ ] **Step 10: Commit**

```bash
git add src-tauri/src/store/repo.rs src-tauri/src/commands.rs src-tauri/src/lead_chat/commands.rs src-tauri/src/lib.rs
git commit -m "feat(agent): support repo-less runs"
```

---

### Task 3: Replace Coding Prompts and Briefs With Generic Agent Prompts

**Files:**
- Modify: `src-tauri/src/lead_chat/commands.rs`
- Modify: `src-tauri/src/brief.rs`
- Add: `src-tauri/tests/lead_prompt.rs`

- [ ] **Step 1: Add a prompt regression test**

Create `src-tauri/tests/lead_prompt.rs`:

```rust
use atlas_app_lib::lead_chat::commands::lead_prompt;

#[test]
fn lead_prompt_is_generic_agent_app_copy() {
    let prompt = lead_prompt();
    assert!(prompt.contains("local Agent App"));
    assert!(prompt.contains("get_task"));
    assert!(prompt.contains("ask_human"));
    assert!(!prompt.contains("get_repo_map"));
    assert!(!prompt.contains("propose_directions"));
    assert!(!prompt.contains("worktree"));
    assert!(!prompt.contains("PR"));
}
```

- [ ] **Step 2: Run the prompt test and verify it fails**

Run:

```bash
cd src-tauri && cargo test --test lead_prompt
```

Expected: FAIL because the current prompt is coding-specific.

- [ ] **Step 3: Replace the lead prompt**

In `src-tauri/src/lead_chat/commands.rs`, replace `BASE_PROMPT`, `SENTINEL_DIRECTIVES`, and `lead_prompt()` with:

```rust
const BASE_PROMPT: &str = "You are the coordinator for this task in atlas, a local Agent App. \
Start by calling get_task to read what the human is asking. Discuss the goal, constraints, \
and next step with the human. You may answer directly, ask a concise clarifying question, \
or suggest a named run for a focused agent session. Do not assume the workspace contains code \
repositories. Do not mention repo maps, worktrees, diffs, pull requests, or pre-PR checks unless \
the human explicitly asks for coding work. Use ask_human when a decision belongs to the human. \
Keep the conversation practical and grounded in the current task.";

pub fn lead_prompt() -> String {
    BASE_PROMPT.to_string()
}
```

- [ ] **Step 4: Stop appending repo state to the lead system prompt**

In `lead_engine`, replace the non-concierge `system_prompt` branch with:

```rust
    let system_prompt = if is_concierge {
        concierge_prompt(lang)
    } else {
        format!("{}{}", lead_prompt(), lang_directive(lang))
    };
```

- [ ] **Step 5: Add generic brief tests**

In `src-tauri/src/brief.rs`, add this test inside the existing tests module:

```rust
#[test]
fn generic_run_brief_has_no_repo_or_check_contract() {
    let s = format_generic_brief("Draft offer email", "task", "Main run", "plan+impl");
    assert!(s.contains("# Run: Main run"));
    assert!(s.contains("Task (task): Draft offer email"));
    assert!(s.contains("Use this run to work with the human"));
    assert!(s.contains("set_task_status(\"working\")"));
    assert!(!s.contains("write repos"));
    assert!(!s.contains("checks pass"));
    assert!(!s.contains("PR"));
}
```

- [ ] **Step 6: Implement `format_generic_brief` and use it for repo-less runs**

In `src-tauri/src/brief.rs`, add this function above `format_brief`:

```rust
pub fn format_generic_brief(task: &str, kind: &str, run: &str, mandate: &str) -> String {
    let mut s = String::new();
    s.push_str(&format!("# Run: {run}\n\n"));
    s.push_str(&format!("Task ({kind}): {task}\n"));
    s.push_str(
        "\n## Work\n\
         Use this run to work with the human on the task. Ask concise questions \
         when requirements are missing, use enabled skills when they match, and \
         report concrete progress in the chat.\n",
    );
    s.push_str(
        "\n## Coordinate\n\
         Use the atlas_bus tools to post updates, read your inbox, and call \
         ask_human when the human must decide something. Do not assume this run \
         is editing a git repository.\n",
    );
    if mandate == "impl-only" {
        s.push_str(
            "\n## Status contract\n\
             This run starts in **working**. When the requested output is ready \
             for the human, call set_task_status(\"review\"). If the human asks \
             for changes, set it to \"working\" again.\n",
        );
    } else {
        s.push_str(
            "\n## Status contract\n\
             This run starts in **planning**. First write a short plan in chat. \
             When you start doing the work, call set_task_status(\"working\"). \
             When the requested output is ready for the human, call \
             set_task_status(\"review\"). If the human asks for changes, set it \
             to \"working\" again.\n",
        );
    }
    s
}
```

Then in `assemble`, after loading `thread` and before computing repo graph, add:

```rust
    if dir.repo_id == 0 {
        return Ok(format_generic_brief(
            &thread.title,
            &thread.kind,
            &dir.name,
            repo::normalize_mandate(&dir.mandate),
        ));
    }
```

- [ ] **Step 7: Run targeted prompt and brief tests**

Run:

```bash
cd src-tauri && cargo test --test lead_prompt
cd src-tauri && cargo test brief::tests::generic_run_brief_has_no_repo_or_check_contract --lib
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/lead_chat/commands.rs src-tauri/src/brief.rs src-tauri/tests/lead_prompt.rs
git commit -m "feat(agent): use generic task prompts"
```

---

### Task 4: Make Planner MCP Generic by Default

**Files:**
- Modify: `src-tauri/src/bus/server.rs`

- [ ] **Step 1: Add planner spec tests**

Add this test module at the bottom of `src-tauri/src/bus/server.rs`:

```rust
#[cfg(test)]
mod planner_tests {
    use super::planner_specs;

    #[test]
    fn planner_specs_expose_generic_task_tools_only() {
        let specs = planner_specs().to_string();
        assert!(specs.contains("get_task"));
        assert!(!specs.contains("get_repo_map"));
        assert!(!specs.contains("propose_directions"));
        assert!(!specs.contains("repo map"));
    }
}
```

- [ ] **Step 2: Run the test and verify it fails**

Run:

```bash
cd src-tauri && cargo test bus::server::planner_tests::planner_specs_expose_generic_task_tools_only --lib
```

Expected: FAIL because the current planner exposes repo map and direction proposal tools.

- [ ] **Step 3: Remove coding planner tools from the default spec**

Replace `planner_specs()` with:

```rust
fn planner_specs() -> Value {
    json!([
        {
            "name": "get_task",
            "description": "Read this task's title and type.",
            "inputSchema": { "type": "object", "properties": {} }
        }
    ])
}
```

- [ ] **Step 4: Make unknown coding planner calls explicit**

In `call_planner`, remove the `get_repo_map` and `propose_directions` match arms from the default path. Keep `get_task`, and let the fallback return `unknown tool: {name}`.

The remaining match body should be:

```rust
    match name {
        "get_task" => match crate::store::repo::get_thread(db, thread).await {
            Ok(Some(t)) => text_result(json!({ "title": t.title, "type": t.kind }).to_string()),
            Ok(None) => text_result("error: thread not found".into()),
            Err(e) => text_result(format!("error: {e}")),
        },
        _ => text_result(format!("unknown tool: {name}")),
    }
```

- [ ] **Step 5: Run the planner test**

Run:

```bash
cd src-tauri && cargo test bus::server::planner_tests::planner_specs_expose_generic_task_tools_only --lib
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/bus/server.rs
git commit -m "feat(agent): expose generic planner tools"
```

---

### Task 5: Add Generic Frontend API and Store Run Path

**Files:**
- Modify: `src/lib/types.ts`
- Modify: `src/lib/api.ts`
- Modify: `src/state/store.tsx`

- [ ] **Step 1: Update TypeScript session types**

In `src/lib/types.ts`, add aliases after `Thread` and `Direction`:

```ts
export type Task = Thread;
export type Run = Direction;
```

Update `SessionInfo`:

```ts
export interface SessionInfo {
  session_id: number;
  repo: string;
  worktree: string;
  cwd: string;
  branch: string;
  tool: string;
  resumed: boolean;
  native_id: string | null;
}
```

- [ ] **Step 2: Add API calls**

In `src/lib/api.ts`, add:

```ts
  createRun: (
    threadId: number,
    name: string,
    tool: string,
    reason?: string,
  ) =>
    invoke<Direction>("create_run", { threadId, name, tool, reason }),
```

Place it next to `createDirection`.

Add:

```ts
  chatOpenRun: (directionId: number, lang: string) =>
    invoke<SessionInfo>("chat_open_run", { directionId, lang }),
```

Place it next to `chatOpenWorker`.

- [ ] **Step 3: Add store methods for repo-less runs**

In the `Store` interface in `src/state/store.tsx`, add:

```ts
  createRun: (threadId: number, name: string, tool: string, reason?: string) => Promise<void>;
  driveRun: (directionId: number, focus: boolean) => Promise<void>;
```

- [ ] **Step 4: Implement `driveRun`**

Add this callback near `driveDirection`:

```ts
  const driveRun = useCallback(
    async (directionId: number, focus: boolean) => {
      const existing = Object.values(sessionsRef.current).find(
        (s) => s.directionId === directionId && s.repoId === 0 && s.status !== "exited",
      );
      if (existing) {
        if (focus) {
          setActiveSessionId(existing.info.session_id);
          setShowNeeds(false);
          setHomeTab("board");
        }
        return;
      }
      const info = await api.chatOpenRun(directionId, currentLang());
      setSessions((m) => ({
        ...m,
        [info.session_id]: {
          info,
          status: "running",
          directionId,
          repoId: 0,
          threadId: activeThreadId ?? -1,
          nativeId: info.native_id,
        },
      }));
      if (focus) {
        setActiveSessionId(info.session_id);
        setShowNeeds(false);
        setHomeTab("board");
      }
    },
    [activeThreadId],
  );
```

- [ ] **Step 5: Implement `createRun`**

Add this callback near `createDirection`:

```ts
  const createRun = useCallback(
    async (threadId: number, name: string, tool: string, reason?: string) => {
      const run = await api.createRun(threadId, name, tool, reason);
      await loadThreadChildren(threadId);
      void driveRun(run.id, false);
    },
    [loadThreadChildren, driveRun],
  );
```

- [ ] **Step 6: Remove default auto-check and auto-review effects from the generic path**

Keep the functions available, but disable these two effects by guarding them with a local constant:

```ts
  const codingAutomationEnabled = false;
```

Then update the auto-check effect condition:

```ts
      if (
        codingAutomationEnabled &&
        wt?.state === "idle" &&
        Date.now() - wt.lastAt > 1200 &&
        !autoCheckedRef.current.has(sess.directionId)
      ) {
```

Update the auto-review effect loop:

```ts
      if (!codingAutomationEnabled || !autoReview || autoReviewedRef.current.has(d.id)) continue;
```

- [ ] **Step 7: Expose new store methods**

In the `value` object, add:

```ts
    createRun,
    driveRun,
```

- [ ] **Step 8: Run TypeScript build and expect type failures from UI callers**

Run:

```bash
pnpm build
```

Expected: FAIL with missing UI wiring or unused imports. The next task fixes those callers.

- [ ] **Step 9: Commit backend-facing frontend API**

Commit only after Task 6 also builds. If implementing task-by-task strictly, leave this staged until the UI changes compile.

---

### Task 6: Switch Default UI From Repo Board to Generic Task/Run Board

**Files:**
- Modify: `src/board/WorkspaceHome.tsx`
- Modify: `src/board/ThreadBoard.tsx`
- Modify: `src/nav/WorkspaceNav.tsx`
- Modify: `src/nav/AppTopBar.tsx`
- Modify: `src/session/SessionView.tsx`
- Modify: `src/session/ObserveView.tsx`

- [ ] **Step 1: Remove repo map from workspace home**

Replace `src/board/WorkspaceHome.tsx` with:

```tsx
import { WorkspaceKanban } from "./WorkspaceKanban";

export function WorkspaceHome() {
  return (
    <section className="flex min-w-0 flex-1 flex-col overflow-hidden bg-bg">
      <WorkspaceKanban />
    </section>
  );
}
```

- [ ] **Step 2: Hide repo add and repo map navigation**

In `src/nav/WorkspaceNav.tsx`:

- Remove `FolderGit2` and `FolderPlus` imports.
- Remove `AddRepoDialog` import.
- Remove `repos`, `writeTriggers`, and repo dialog state from the destructuring and local state.
- Change `needsCount` to:

```ts
  const needsCount = needs.length + asks.length;
```

- Remove the "Add repo" button.
- Remove the `workspace.tabRepos` nav item.
- Remove `<AddRepoDialog ... />`.

- [ ] **Step 3: Make thread board cards open repo-less runs**

In `src/board/ThreadBoard.tsx`:

- Remove `GitBranch`, `GitCompare`, `ScanEye`, `ScopeReview`, and repo/check-specific imports when no longer used.
- Replace `DirectionCard` with a generic version that uses `driveRun`.

Use this body for the primary action area:

```tsx
  const { sessions, driveRun, needs, asks, openNeeds } = useStore();
  const hasNeed =
    needs.some((n) => n.direction_id === direction.id) ||
    asks.some((a) => a.dir === String(direction.id));
  const sess = Object.values(sessions).find(
    (s) => s.directionId === direction.id && s.repoId === 0 && s.status !== "exited",
  );
  const action = hasNeed
    ? { label: t("thread.handle"), variant: "primary" as const }
    : { label: sess ? t("thread.openSession") : t("thread.startRun"), variant: "default" as const };
```

The button should call:

```tsx
onClick={() => (hasNeed ? openNeeds() : void driveRun(direction.id, true))}
```

- [ ] **Step 4: Remove ScopeReview from the default thread route**

In `ThreadBoard`, replace:

```tsx
        ) : reviewingProposal && proposal && proposal.status === "proposed" ? (
          <ScopeReview ... />
        ) : dirs.length === 0 ? (
```

with:

```tsx
        ) : dirs.length === 0 ? (
```

Also remove `proposal`, `reviewingProposal`, and `setReviewingProposal` from the destructuring if unused.

- [ ] **Step 5: Hide diff button for repo-less sessions**

In `src/session/SessionView.tsx`, define:

```ts
  const hasDiff = info.branch.trim().length > 0;
```

Wrap the diff button and `DiffPanel` with `hasDiff`:

```tsx
          {hasDiff && (
            <button
              onClick={() => setShowDiff(true)}
              title={t("diff.tab")}
              aria-label={t("diff.tab")}
              className="grid h-7 w-7 shrink-0 place-items-center rounded-[var(--radius-md)] border border-border text-ink-muted transition-colors hover:bg-surface hover:text-ink"
            >
              <GitCompare size={13} />
            </button>
          )}
```

```tsx
      {hasDiff && (
        <DiffPanel
          cwd={info.worktree}
          open={showDiff}
          onClose={() => setShowDiff(false)}
          onAsk={(text) => void api.chatSend(info.session_id, text)}
        />
      )}
```

- [ ] **Step 6: Keep ObserveView for legacy coding only**

In `src/session/ObserveView.tsx`, add an early guard after `if (viewing == null) return null;`:

```tsx
  if (viewing.repoId === 0) return null;
```

The generic path opens `SessionView`, not observe/diff.

- [ ] **Step 7: Run TypeScript build**

Run:

```bash
pnpm build
```

Expected: PASS after unused imports and type errors are removed.

- [ ] **Step 8: Commit**

```bash
git add src/lib/types.ts src/lib/api.ts src/state/store.tsx src/board/WorkspaceHome.tsx src/board/ThreadBoard.tsx src/nav/WorkspaceNav.tsx src/nav/AppTopBar.tsx src/session/SessionView.tsx src/session/ObserveView.tsx
git commit -m "feat(ui): make default workspace generic"
```

---

### Task 7: Replace First-Run and Visible Copy

**Files:**
- Modify: `src/components/FirstRunOnboarding.tsx`
- Modify: `src/i18n/en.ts`
- Modify: `src/i18n/zh.ts`
- Modify: `src/nav/dialogs.tsx`

- [ ] **Step 1: Simplify first-run onboarding to workspace creation**

In `src/components/FirstRunOnboarding.tsx`:

- Remove `REPOS`, `NODES`, `EDGES`, `OnboardingGraph`, and `ScopeLane` usage.
- Keep only three stages: welcome, workspace, start.
- Default `workspaceName` to `"My workspace"` in English UI and `"我的工作区"` in Chinese UI by using i18n copy instead of hard-coded `"结算改版"`.

Use this steps lookup:

```ts
  const steps = t("onboarding.steps", { returnObjects: true }) as string[];
  const [workspaceName, setWorkspaceName] = useState(t("onboarding.defaultWorkspaceName"));
```

- [ ] **Step 2: Make CreateThreadDialog create a task**

In `src/nav/dialogs.tsx`, keep the function name `CreateThreadDialog` for compatibility, but change labels to task copy:

```tsx
<DialogContent title={t("dialog.newTaskTitle")} description={t("dialog.newTaskDesc")}>
```

Use `t("dialog.taskTitle")`, `t("dialog.taskTitleHint")`, and `t("dialog.createTask")` for the title field and submit button.

- [ ] **Step 3: Update English navigation and onboarding copy**

In `src/i18n/en.ts`, update these keys:

```ts
nav: {
  home: "Workspace",
  threads: "Tasks",
  newThread: "New task",
  deleteThread: "Delete task",
  renameThread: "Rename task",
  noThreads: "No tasks yet. Create one to start.",
  createWorkspaceFirst: "Create a workspace to begin.",
  needsYou: "Needs you",
  otherWorkspaceNeeds: "Another workspace needs you",
  newWorkspace: "New workspace",
  noWorkspace: "No workspace",
  local: "Local · no server",
},
palette: {
  issue: "Task",
  board: "Workspace",
  repos: "Repos",
},
workspace: {
  threadsCount: "{{count}} tasks",
  emptyTitleHas: "No tasks yet",
  emptyBodyHas: "A task is one conversation or agent run. Create one, pick a provider, and Atlas keeps the session, skills, and permission requests in one place.",
  emptyBodyNoWs: "Create a workspace to begin.",
},
onboarding: {
  steps: ["Welcome", "Workspace", "Start"],
  defaultWorkspaceName: "My workspace",
  heroSubtitle: "Local-first multi-agent desktop app",
  heroBody: "Chat with local Claude, Codex, or OpenCode sessions through one desktop UI, with skills and permission requests built in.",
  workspaceTitle: "Create a workspace",
  workspaceBody: "A workspace groups tasks, skills, and local agent sessions. It does not need any git repositories.",
  startTitle: "Start with a task",
  startBody: "Create a task from the sidebar, choose a provider, and start chatting.",
},
dialog: {
  newTaskTitle: "New task",
  newTaskDesc: "A task is one conversation or agent run.",
  taskTitle: "Title",
  taskTitleHint: "Draft an onboarding email",
  createTask: "Create task",
},
```

Also update the existing palette search prompt string to `"Search tasks · jump · act"`. Keep unrelated existing keys unless TypeScript or runtime access requires them.

- [ ] **Step 4: Update Chinese navigation and onboarding copy**

In `src/i18n/zh.ts`, update equivalent keys:

```ts
nav: {
  home: "工作区",
  threads: "任务",
  newThread: "新建任务",
  deleteThread: "删除任务",
  renameThread: "重命名任务",
  noThreads: "还没有任务。新建一个开始。",
  createWorkspaceFirst: "先创建一个工作区。",
  needsYou: "待你处理",
  otherWorkspaceNeeds: "其他工作区有待办",
  newWorkspace: "新建工作区",
  noWorkspace: "无工作区",
  local: "本地 · 无服务端",
},
palette: {
  issue: "任务",
  board: "工作区",
  repos: "仓库",
},
workspace: {
  threadsCount: "{{count}} 个任务",
  emptyTitleHas: "还没有任务",
  emptyBodyHas: "任务是一段对话或一次 agent 运行。新建任务后选择 provider，Atlas 会管理会话、skills 和权限请求。",
  emptyBodyNoWs: "先创建一个工作区。",
},
onboarding: {
  steps: ["欢迎", "工作区", "开始"],
  defaultWorkspaceName: "我的工作区",
  heroSubtitle: "本地优先的多 Agent 桌面应用",
  heroBody: "在一个桌面界面里对话并运行本地 Claude、Codex 或 OpenCode，会话、skills 和权限请求都集中管理。",
  workspaceTitle: "创建工作区",
  workspaceBody: "工作区用于组织任务、skills 和本地 agent 会话，不需要添加任何 git 仓库。",
  startTitle: "从任务开始",
  startBody: "在侧边栏新建任务，选择 provider，然后开始对话。",
},
dialog: {
  newTaskTitle: "新建任务",
  newTaskDesc: "任务是一段对话或一次 agent 运行。",
  taskTitle: "标题",
  taskTitleHint: "起草一封入职邮件",
  createTask: "创建任务",
},
```

Also update the existing palette search prompt string to `"搜索任务 · 跳转 · 动作"`。

- [ ] **Step 5: Search for visible coding copy**

Run:

```bash
rg -n "repo|Repos|worktree|working copy|diff|PR|pre-PR|checks|scope|issue|sub-task|仓库|工作副本|改动|检查|跨仓|子任务" src
```

Expected: remaining hits are either hidden legacy components, type names, or settings for non-default coding features. Remove visible default-route hits.

- [ ] **Step 6: Run TypeScript build**

Run:

```bash
pnpm build
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/components/FirstRunOnboarding.tsx src/i18n/en.ts src/i18n/zh.ts src/nav/dialogs.tsx
git commit -m "polish(copy): remove coding-first app language"
```

---

### Task 8: End-to-End Verification and Cleanup

**Files:**
- Modify only if verification exposes issues.

- [ ] **Step 1: Run Rust targeted tests**

Run:

```bash
cd src-tauri && cargo test paths::tests::run_home --lib
cd src-tauri && cargo test store::repo::tests::repo_less_direction_can_back_a_generic_session --lib
cd src-tauri && cargo test brief::tests::generic_run_brief_has_no_repo_or_check_contract --lib
cd src-tauri && cargo test bus::server::planner_tests::planner_specs_expose_generic_task_tools_only --lib
cd src-tauri && cargo test --test lead_prompt
```

Expected: PASS.

- [ ] **Step 2: Run broader automated checks**

Run:

```bash
pnpm build
cd src-tauri && cargo test
git diff --check
```

Expected: PASS. If `cargo test` fails only in legacy coding tests that assert repo-first UI behavior, update those tests to call the legacy command path explicitly and keep the new generic tests passing.

- [ ] **Step 3: Start the desktop app**

Run:

```bash
pnpm tauri dev
```

Expected: app opens with no repo requirement.

- [ ] **Step 4: Manual UI verification**

In the running app:

1. Clear or use a fresh `ATLAS_HOME` test directory.
2. Launch Atlas.
3. Confirm first-run does not ask to add repos.
4. Create a workspace.
5. Create a task.
6. Confirm the task opens to chat, not repo scope review.
7. Start a run with an installed provider.
8. Confirm the session opens and streams output.
9. Confirm no Diff button appears for a repo-less run.
10. Confirm Settings -> Skills still opens.
11. Confirm Needs-you still opens.

- [ ] **Step 5: Search final default-route coding leakage**

Run:

```bash
rg -n "Add repo|Repo map|worktree|working copy|View changes|pre-PR|Run review|仓库地图|添加仓库|工作副本|查看改动|自动 review" src
```

Expected: no hits in components used by the default workspace/task/session route. Hits in legacy coding components are acceptable when those components are not mounted by default.

- [ ] **Step 6: Commit verification fixes**

If Step 2-5 required fixes:

```bash
git add <changed-files>
git commit -m "fix(agent): complete generic default path"
```

If no fixes were needed, do not create an empty commit.

---

## Self-Review

### Spec Coverage

- No repo requirement: Task 2 adds repo-less runs; Task 6 removes repo nav; Task 7 changes first-run.
- Multi provider retained: Task 2 and Task 5 keep existing provider field and call existing chat engine.
- Skills retained: no task removes skill source/enable/inject; Task 8 verifies Settings -> Skills.
- Ask / Needs-you retained: Task 2 keeps injection; Task 6 leaves Needs-you route; Task 8 verifies it.
- Coding defaults removed: Task 3 removes coding prompts; Task 4 removes repo planner tools; Task 6 hides diff and repo board; Task 7 removes visible coding copy.
- No HR/Pack/Scenario expansion: no tasks add HR data or plugin model.

### Placeholder Scan

This plan intentionally avoids open-ended implementation instructions. Each code-changing task names files, commands, expected results, and the concrete snippets to add or replace.

### Type Consistency

- Backend keeps `thread` and `direction`; frontend aliases them as `Task` and `Run`.
- New command names are `create_run` and `chat_open_run`; frontend wrappers are `createRun` and `chatOpenRun`.
- Repo-less sessions consistently use `repo_id = 0`.
- `SessionInfo.cwd` is added while `worktree` remains as a compatibility field for existing UI utilities.
