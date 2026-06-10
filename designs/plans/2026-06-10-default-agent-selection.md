# 检测驱动的默认 agent 选择 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 默认 coding agent 由后端基于本地已安装 CLI 解析(优先 codex);lead 与 worker 都消费它,worker 可在批准卡片上二次选择。

**Architecture:** 后端新增 `app_setting` KV 表存用户显式选择,`detect.rs` 提供纯函数解析(用户选择若已安装则生效,否则按 codex > claude > opencode 取第一个已安装)。`thread` 表加 `lead_tool` 列在创建时盖章;lead 提案不再含 `tool` 字段,direction 的工具由批准时的卡片选择(或批量 confirm 用默认)决定。前端 `defaultTool` 从 localStorage 改为后端命令读写。

**Tech Stack:** Rust (Tauri v2, SeaORM/SQLite, sea-orm-migration), React + TypeScript, i18next。

**Spec:** `designs/specs/2026-06-10-default-agent-selection-design.md`

---

## File Structure

| 文件 | 动作 | 职责 |
|---|---|---|
| `src-tauri/src/detect.rs` | Modify | + `TOOL_PRIORITY`、`pick_default_tool`(纯函数)、`resolve_default_tool` |
| `src-tauri/src/store/entities/app_setting.rs` | Create | KV 设置表 entity |
| `src-tauri/src/store/entities/mod.rs` | Modify | 注册 app_setting 模块 |
| `src-tauri/src/store/entities/thread.rs` | Modify | + `lead_tool` 字段 |
| `src-tauri/src/store/entities/repo_ref.rs` | Modify | − `default_tool` 死字段 |
| `src-tauri/src/store/migration/mod.rs` | Modify | + M0010AppSetting、M0011ThreadLeadTool、M0012DropRepoDefaultTool |
| `src-tauri/src/store/repo.rs` | Modify | + `get_setting`/`set_setting`;`create_thread` 加 `lead_tool` 参数;`add_repo_ref` 删 `default_tool` 参数 |
| `src-tauri/src/tools.rs` | Modify | + `default_tool(db)` 组合 helper |
| `src-tauri/src/commands.rs` | Modify | `create_thread` 盖章;`approve_write_trigger` 加 `tool` 参数;+ `get_default_tool`/`set_default_tool` 命令 |
| `src-tauri/src/lib.rs` | Modify | 注册两个新命令 |
| `src-tauri/src/lead_chat/commands.rs` | Modify | lead_engine 三处 `"claude"` 改读 `thread.lead_tool`;lead prompt 去掉 tool |
| `src-tauri/src/planner.rs` | Modify | `ProposedDirection`/`ResolvedDirection` 删 `tool`;`approve_direction` 加 `tool` 参数;`confirm` 用默认工具 |
| `src-tauri/src/bus/server.rs` | Modify | propose_directions schema 删 `tool` |
| `src-tauri/tests/m2_worktree.rs` | Modify | 适配 `add_repo_ref`/`create_thread` 新签名 |
| `src/lib/types.ts` | Modify | + `DefaultToolInfo`;`ResolvedDirection` 删 `tool` |
| `src/lib/api.ts` | Modify | + `getDefaultTool`/`setDefaultTool`;`approveWriteTrigger` 加 `tool` |
| `src/state/store.tsx` | Modify | defaultTool 改后端来源;+ `configuredTool`/`installedTools` |
| `src/board/NeedsYouView.tsx` | Modify | WriteTriggerRow 加工具选择器 |
| `src/board/ScopeReview.tsx` | Modify | 工具徽章改显示当前默认 |
| `src/nav/SettingsDialog.tsx` | Modify | 选项只列已安装;回退提示;无 CLI 警告 |
| `src/i18n/en.ts` / `src/i18n/zh.ts` | Modify | 新 key + hint 文案更新 |

---

### Task 1: detect.rs 默认工具解析(纯函数,TDD)

**Files:**
- Modify: `src-tauri/src/detect.rs`(实现加在 `meets_min` 之后;测试加在文件尾部既有 `mod tests` 内)

- [ ] **Step 1: 写失败测试**

在 `detect.rs` 的 `#[cfg(test)] mod tests` 内追加:

```rust
    #[test]
    fn default_tool_prefers_user_choice_when_installed() {
        let installed = |t: &str| t == "claude" || t == "codex";
        assert_eq!(pick_default_tool(Some("claude"), installed), "claude");
    }

    #[test]
    fn default_tool_falls_back_when_user_choice_missing() {
        let installed = |t: &str| t == "claude";
        assert_eq!(pick_default_tool(Some("codex"), installed), "claude");
    }

    #[test]
    fn default_tool_detects_by_priority() {
        let installed = |t: &str| t == "codex" || t == "opencode";
        assert_eq!(pick_default_tool(None, installed), "codex");
        let only_oc = |t: &str| t == "opencode";
        assert_eq!(pick_default_tool(None, only_oc), "opencode");
    }

    #[test]
    fn default_tool_codex_when_nothing_installed() {
        assert_eq!(pick_default_tool(None, |_| false), "codex");
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cd src-tauri && cargo test default_tool`
Expected: 编译错误 `cannot find function pick_default_tool`(编译失败即红灯)

- [ ] **Step 3: 最小实现**

在 `detect.rs` 的 `meets_min` 函数后追加:

```rust
/// Preference order when the user hasn't chosen a tool explicitly.
pub(crate) const TOOL_PRIORITY: [&str; 3] = ["codex", "claude", "opencode"];

/// Pure default-tool decision: an explicit user choice wins when that CLI is
/// installed; otherwise the first installed tool by priority; otherwise codex
/// (nothing can spawn anyway — Settings surfaces the "no CLI" warning).
pub(crate) fn pick_default_tool(user: Option<&str>, installed: impl Fn(&str) -> bool) -> String {
    if let Some(u) = user {
        if installed(u) {
            return u.to_string();
        }
    }
    TOOL_PRIORITY
        .iter()
        .copied()
        .find(|t| installed(t))
        .unwrap_or("codex")
        .to_string()
}

/// Resolve the effective default tool against the real PATH (and the Codex
/// app-bundle fallback), honoring the user's explicit choice when present.
pub fn resolve_default_tool(user: Option<&str>) -> String {
    pick_default_tool(user, |t| resolve_tool_path(t).is_some())
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cd src-tauri && cargo test default_tool`
Expected: 4 passed

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/detect.rs
git commit -m "feat(detect): 默认工具解析 — 用户选择优先,否则按 codex>claude>opencode 检测"
```

---

### Task 2: app_setting KV 表 + 读写 helper(TDD)

**Files:**
- Create: `src-tauri/src/store/entities/app_setting.rs`
- Modify: `src-tauri/src/store/entities/mod.rs`、`src-tauri/src/store/migration/mod.rs`、`src-tauri/src/store/repo.rs`

- [ ] **Step 1: 写失败测试**

在 `src-tauri/src/store/repo.rs` 尾部 `mod tests` 内追加(测试模块已有 `mem()` helper,`Db::connect` 自动跑迁移):

```rust
    #[tokio::test]
    async fn app_setting_roundtrip() {
        let db = mem().await;
        assert_eq!(get_setting(&db, "default_tool").await.unwrap(), None);
        set_setting(&db, "default_tool", "codex").await.unwrap();
        assert_eq!(
            get_setting(&db, "default_tool").await.unwrap(),
            Some("codex".to_string())
        );
        // Overwrite, not duplicate.
        set_setting(&db, "default_tool", "claude").await.unwrap();
        assert_eq!(
            get_setting(&db, "default_tool").await.unwrap(),
            Some("claude".to_string())
        );
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cd src-tauri && cargo test app_setting`
Expected: 编译错误 `cannot find function get_setting`

- [ ] **Step 3: 实现**

新建 `src-tauri/src/store/entities/app_setting.rs`(对照 `repo_ref.rs` 的样式):

```rust
use sea_orm::entity::prelude::*;

/// Global key-value app settings (e.g. "default_tool"). One row per key.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, serde::Serialize, serde::Deserialize)]
#[sea_orm(table_name = "app_setting")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub key: String,
    pub value: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
```

`entities/mod.rs` 加一行(按字母序):

```rust
pub mod app_setting;
```

`migration/mod.rs`:第 1-3 行的 `use crate::store::entities::{...}` 列表加入 `app_setting`;`Migrator::migrations()` 的 vec 追加 `Box::new(M0010AppSetting),`;文件尾部追加(对照 M0002 模式):

```rust
/// Adds the global key-value settings table (default-tool selection).
pub struct M0010AppSetting;

impl MigrationName for M0010AppSetting {
    fn name(&self) -> &str {
        "m0010_app_setting"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for M0010AppSetting {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let schema = Schema::new(manager.get_database_backend());
        let mut stmt = schema.create_table_from_entity(app_setting::Entity);
        stmt.if_not_exists();
        manager.create_table(stmt).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Alias::new("app_setting")).to_owned())
            .await?;
        Ok(())
    }
}
```

`store/repo.rs`:顶部 entities 导入列表加 `app_setting`;在 `list_workspaces` 之后追加:

```rust
pub async fn get_setting(db: &Db, key: &str) -> Result<Option<String>> {
    Ok(app_setting::Entity::find_by_id(key)
        .one(&db.0)
        .await?
        .map(|m| m.value))
}

pub async fn set_setting(db: &Db, key: &str, value: &str) -> Result<()> {
    let m = app_setting::ActiveModel {
        key: Set(key.to_string()),
        value: Set(value.to_string()),
    };
    app_setting::Entity::insert(m)
        .on_conflict(
            sea_orm::sea_query::OnConflict::column(app_setting::Column::Key)
                .update_column(app_setting::Column::Value)
                .to_owned(),
        )
        .exec(&db.0)
        .await?;
    Ok(())
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cd src-tauri && cargo test app_setting`
Expected: 1 passed

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/store
git commit -m "feat(store): app_setting KV 表 + get/set helper"
```

---

### Task 3: thread.lead_tool 盖章 + tools::default_tool(TDD)

**Files:**
- Modify: `src-tauri/src/store/entities/thread.rs`、`src-tauri/src/store/migration/mod.rs`、`src-tauri/src/store/repo.rs`、`src-tauri/src/tools.rs`、`src-tauri/src/commands.rs:119-121`、`src-tauri/src/planner.rs:397`、`src-tauri/tests/m2_worktree.rs:41,53`

- [ ] **Step 1: 写失败测试**

`store/repo.rs` 的 `mod tests` 内追加:

```rust
    #[tokio::test]
    async fn create_thread_stamps_lead_tool() {
        let db = mem().await;
        let ws = create_workspace(&db, "w").await.unwrap();
        let t = create_thread(&db, ws.id, "Add feature", "feature", "codex")
            .await
            .unwrap();
        assert_eq!(t.lead_tool, "codex");
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cd src-tauri && cargo test create_thread_stamps`
Expected: 编译错误(`create_thread` 参数个数不符 / `lead_tool` 字段不存在)

- [ ] **Step 3: 实现**

`entities/thread.rs` 的 Model 加字段(`created_at` 之前):

```rust
    pub kind: String,
    /// The coding CLI driving this thread's lead, stamped at creation.
    pub lead_tool: String,
    pub created_at: String,
```

`migration/mod.rs`:vec 追加 `Box::new(M0011ThreadLeadTool),`;尾部追加(对照 M0004 的容错模式;存量行回填 `"claude"`):

```rust
/// Adds thread.lead_tool (the CLI driving the thread's lead), stamped at
/// creation. Existing threads were always claude-led, so backfill "claude".
/// M0001 reflects the current entity, so a FRESH db already has the column;
/// sqlite has no ADD COLUMN IF NOT EXISTS, so tolerate the duplicate.
pub struct M0011ThreadLeadTool;

impl MigrationName for M0011ThreadLeadTool {
    fn name(&self) -> &str {
        "m0011_thread_lead_tool"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for M0011ThreadLeadTool {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let r = manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("thread"))
                    .add_column(
                        ColumnDef::new(Alias::new("lead_tool"))
                            .string()
                            .not_null()
                            .default("claude"),
                    )
                    .to_owned(),
            )
            .await;
        match r {
            Ok(()) => Ok(()),
            Err(e) if e.to_string().to_lowercase().contains("duplicate column") => Ok(()),
            Err(e) => Err(e),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("thread"))
                    .drop_column(Alias::new("lead_tool"))
                    .to_owned(),
            )
            .await
    }
}
```

`store/repo.rs` 的 `create_thread`(67-89 行)加参数并写入:

```rust
pub async fn create_thread(
    db: &Db,
    workspace_id: i32,
    title: &str,
    kind: &str,
    lead_tool: &str,
) -> Result<thread::Model> {
```

ActiveModel 内加:

```rust
        kind: Set(kind.to_string()),
        lead_tool: Set(lead_tool.to_string()),
        created_at: Set(now()),
```

`tools.rs` 尾部(`detect_tools` 之后)追加组合 helper:

```rust
/// The effective default coding tool: the Settings choice when that CLI is
/// installed, else the first installed CLI by priority (codex > claude >
/// opencode). Reads app_setting "default_tool"; resolution is detect.rs's.
pub async fn default_tool(db: &crate::store::Db) -> String {
    let configured = crate::store::repo::get_setting(db, "default_tool")
        .await
        .ok()
        .flatten();
    crate::detect::resolve_default_tool(configured.as_deref())
}
```

`commands.rs:119-121` 的 `create_thread` 命令改为后端盖章:

```rust
#[tauri::command]
pub async fn create_thread(db: State<'_, Db>, workspace_id: i32, title: String, kind: String) -> R<entities::thread::Model> {
    let tool = crate::tools::default_tool(&db).await;
    repo::create_thread(&db, workspace_id, &title, &kind, &tool).await.map_err(e)
}
```

更新其余调用方(全部追加 `"claude"` 作为测试用 lead_tool 实参):
- `src-tauri/src/planner.rs:397`:`repo::create_thread(&db, ws.id, "t1", "feature", "claude")`
- `src-tauri/src/store/repo.rs:559,591,610` 测试内同理
- `src-tauri/tests/m2_worktree.rs:41,53`:`repo::create_thread(&db, ws.id, "t1", "feature", "claude")` / `"t2"` 同理

- [ ] **Step 4: 跑全量测试确认通过**

Run: `cd src-tauri && cargo test`
Expected: 全部 passed(含新 `create_thread_stamps_lead_tool`)

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src src-tauri/tests
git commit -m "feat(store): thread.lead_tool 创建时盖章,默认工具由后端解析"
```

---

### Task 4: lead engine 消费 thread.lead_tool

**Files:**
- Modify: `src-tauri/src/lead_chat/commands.rs:67-102`

- [ ] **Step 1: 改 lead_engine 三处硬编码**

`lead_engine`(57 行起)中,把丢弃 thread 查询结果的语句改为绑定:

```rust
    let t = repo::get_thread(db, thread_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("thread not found"))?;
```

79-80 行注入改用盖章工具:

```rust
    let inj = crate::bus::inject::inject_planner(&base, thread_id, &t.lead_tool, &cwd);
    let ask = crate::bus::inject::inject_ask_hook(&base, thread_id, "lead", &t.lead_tool, &cwd);
```

86 行 engine 字段:

```rust
        tool: t.lead_tool.clone(),
```

- [ ] **Step 2: 编译 + 全量测试**

Run: `cd src-tauri && cargo test`
Expected: 全部 passed

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/lead_chat/commands.rs
git commit -m "feat(lead): lead 会话改用 thread.lead_tool,不再硬编码 claude"
```

---

### Task 5: planner 去掉 tool 字段;approve 带 tool,confirm 用默认

**Files:**
- Modify: `src-tauri/src/planner.rs`、`src-tauri/src/bus/server.rs:352-366`、`src-tauri/src/lead_chat/commands.rs:28-41`、`src-tauri/src/commands.rs:420-426`

- [ ] **Step 1: 先改测试(红灯)**

`planner.rs` 的 `mod tests`:
- 五处 struct 字面量(286、303、331、344-346、404-418 行)删去 `tool: ...` 行。
- 316-325 行的测试改名并保留 JSON 里的 `"tool"`,用来证明存量数据兼容:

```rust
    #[test]
    fn proposal_parses_with_missing_and_legacy_fields() {
        // Legacy proposals carried a "tool" per direction; serde must ignore it.
        let p: Proposal = serde_json::from_str(
            r#"{ "directions": [ { "name": "wip", "tool": "claude" } ] }"#,
        )
        .unwrap();
        assert_eq!(p.rationale, "");
        assert_eq!(p.directions.len(), 1);
        assert_eq!(p.directions[0].repo, "");
        assert_eq!(p.directions[0].reason, "");
    }
```

- 431 行 approve 调用带上工具并断言落库(445 行同步加参数):

```rust
        let id = approve_direction(&db, t.id, 0, "codex").await.unwrap();
        let dirs = repo::list_directions(&db, t.id).await.unwrap();
        assert_eq!(dirs.len(), 1, "exactly one direction created");
        assert_eq!(dirs[0].id, id);
        assert_eq!(dirs[0].repo_id, ra.id);
        assert_eq!(dirs[0].tool, "codex", "card-picked tool lands on the direction");
```

```rust
        let id2 = approve_direction(&db, t.id, 0, "codex").await.unwrap();
```

Run: `cd src-tauri && cargo test planner`
Expected: 编译错误(struct 仍含 tool、approve_direction 仍是 3 参)

- [ ] **Step 2: 实现**

`planner.rs`:
- `ProposedDirection`(20-33 行)删 `pub tool: String,`;`ResolvedDirection`(53-63 行)删 `pub tool: String,`;`resolve()`(67 行起)删 `tool: dir.tool.clone(),`。
- `confirm()`(127 行起)循环前取默认、循环内改用:

```rust
    let mut created = Vec::new();
    let tool = crate::tools::default_tool(db).await;
    for d in &resolved.directions {
```

```rust
        let dir =
            repo::create_direction(
                db, thread_id, &d.name, &tool, d.repo.repo_id, &d.reason, &d.mandate,
            )
            .await?;
```

- `approve_direction`(156 行起)签名加参数,187 行改用参数:

```rust
pub async fn approve_direction(db: &Db, thread_id: i32, index: usize, tool: &str) -> Result<i32> {
```

```rust
    let dir = repo::create_direction(
        db,
        thread_id,
        &resolved.name,
        tool,
        resolved.repo.repo_id,
        &resolved.reason,
        &resolved.mandate,
    )
```

`bus/server.rs:357-364` 的 schema 删 `"tool": str_prop(),`,required 改为:

```rust
                }, "required": ["name", "repo", "reason"] } }
```

`lead_chat/commands.rs` 的 `lead_prompt`(36 行)改:

```text
旧: (name, tool, the ONE repo each writes, reason, mandate); only list repos each direction must WRITE
新: (name, the ONE repo each writes, reason, mandate); only list repos each direction must WRITE
```

`commands.rs:420-426` 的 `approve_write_trigger` 透传:

```rust
#[tauri::command]
pub async fn approve_write_trigger(
    db: State<'_, Db>,
    thread_id: i32,
    index: usize,
    tool: String,
) -> R<i32> {
    crate::planner::approve_direction(&db, thread_id, index, &tool).await.map_err(e)
}
```

- [ ] **Step 3: 跑全量测试确认通过**

Run: `cd src-tauri && cargo test`
Expected: 全部 passed

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src
git commit -m "feat(plan): lead 不再分配工具 — 批准卡片选定,批量 confirm 用默认"
```

---

### Task 6: get/set_default_tool 命令 + 注册

**Files:**
- Modify: `src-tauri/src/commands.rs`(`write_triggers` 附近)、`src-tauri/src/lib.rs:149` 附近

- [ ] **Step 1: 加命令**

`commands.rs` 追加(放在 `approve_write_trigger` 之前):

```rust
/// The resolved default coding tool plus the user's explicit choice (if any).
/// `tool` is what new threads/directions get; `configured != tool` means the
/// configured CLI is missing and we fell back.
#[derive(serde::Serialize)]
pub struct DefaultTool {
    pub tool: String,
    pub configured: Option<String>,
}

#[tauri::command]
pub async fn get_default_tool(db: State<'_, Db>) -> R<DefaultTool> {
    let configured = repo::get_setting(&db, "default_tool").await.map_err(e)?;
    let tool = crate::detect::resolve_default_tool(configured.as_deref());
    Ok(DefaultTool { tool, configured })
}

#[tauri::command]
pub async fn set_default_tool(db: State<'_, Db>, tool: String) -> R<()> {
    repo::set_setting(&db, "default_tool", &tool).await.map_err(e)
}
```

`lib.rs` 的 handler 列表(`tools::detect_tools` 旁)加:

```rust
            commands::get_default_tool,
            commands::set_default_tool,
```

- [ ] **Step 2: 编译 + 测试**

Run: `cd src-tauri && cargo test`
Expected: 全部 passed

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat(settings): get/set_default_tool 命令 — 后端解析默认工具"
```

---

### Task 7: 删除 repo_ref.default_tool 死字段

**Files:**
- Modify: `src-tauri/src/store/entities/repo_ref.rs:13`、`src-tauri/src/store/migration/mod.rs`、`src-tauri/src/store/repo.rs:40-65,556,588`、`src-tauri/src/commands.rs:34`、`src-tauri/src/planner.rs:394`、`src-tauri/tests/m2_worktree.rs:37-38`

- [ ] **Step 1: 实现**

`entities/repo_ref.rs` 删 `pub default_tool: String,`。

`migration/mod.rs`:vec 追加 `Box::new(M0012DropRepoDefaultTool),`;尾部追加(对照 M0009 容错模式):

```rust
/// Drops the dead repo_ref.default_tool column: written once at registration
/// ("claude"), never read — tool selection is now app_setting + per-card. A
/// FRESH db (M0001 reflects the entity) never has it, so tolerate the miss.
pub struct M0012DropRepoDefaultTool;

impl MigrationName for M0012DropRepoDefaultTool {
    fn name(&self) -> &str {
        "m0012_drop_repo_default_tool"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for M0012DropRepoDefaultTool {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let r = manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("repo_ref"))
                    .drop_column(Alias::new("default_tool"))
                    .to_owned(),
            )
            .await;
        match r {
            Ok(()) => Ok(()),
            Err(e) if e.to_string().to_lowercase().contains("no such column") => Ok(()),
            Err(e) => Err(e),
        }
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Irreversible: the dead column is gone for good. No-op.
        Ok(())
    }
}
```

`store/repo.rs` 的 `add_repo_ref`(40-65 行)删 `default_tool: &str` 参数与 `default_tool: Set(...)` 行。

调用方删最后一个 `"claude"` 实参:
- `commands.rs:34`:`repo::add_repo_ref(db, workspace_id, name, path, &base).await`
- `planner.rs:394`、`store/repo.rs:556,588`、`tests/m2_worktree.rs:37-38` 同理

- [ ] **Step 2: 编译 + 测试**

Run: `cd src-tauri && cargo test`
Expected: 全部 passed

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src src-tauri/tests
git commit -m "chore(store): 删除 repo_ref.default_tool 死字段"
```

---

### Task 8: 前端数据层 — defaultTool 改后端来源

**Files:**
- Modify: `src/lib/types.ts:242-248,290-297`、`src/lib/api.ts:147-150,159`、`src/state/store.tsx:96-97,121,262-268,1028-1041,1260-1261`、`src/board/ScopeReview.tsx:157,221-224`

- [ ] **Step 1: types.ts**

`ResolvedDirection`(242 行)删 `tool: string;`。`ToolStatus` 之后加:

```ts
/** The resolved default coding tool plus the user's explicit choice (if any). */
export interface DefaultToolInfo {
  tool: string;
  configured: string | null;
}
```

- [ ] **Step 2: api.ts**

`approveWriteTrigger`(148 行)加参数;`detectTools` 旁加两个命令(types 导入加 `DefaultToolInfo`):

```ts
  approveWriteTrigger: (threadId: number, index: number, tool: string) =>
    invoke<number>("approve_write_trigger", { threadId, index, tool }),
```

```ts
  getDefaultTool: () => invoke<DefaultToolInfo>("get_default_tool"),
  setDefaultTool: (tool: string) => invoke<void>("set_default_tool", { tool }),
```

- [ ] **Step 3: store.tsx**

Store 接口(96-97 行)改为:

```ts
  defaultTool: string;
  setDefaultTool: (t: string) => void;
  /** The user's explicit Settings choice; null = auto-detected. */
  configuredTool: string | null;
  /** detect_tools result, loaded once at startup (for tool pickers). */
  installedTools: ToolStatus[];
```

`approveWriteTrigger`(121 行)签名改为 `(item: WriteTrigger, tool?: string) => Promise<void>`。

262-268 行的 localStorage 实现整体替换为(`ToolStatus` 加入 types 导入;不迁移旧 localStorage 值):

```ts
  const [defaultTool, setDefaultToolState] = useState("codex");
  const [configuredTool, setConfiguredTool] = useState<string | null>(null);
  const [installedTools, setInstalledTools] = useState<ToolStatus[]>([]);
  useEffect(() => {
    void (async () => {
      try {
        const [info, tools] = await Promise.all([api.getDefaultTool(), api.detectTools()]);
        setDefaultToolState(info.tool);
        setConfiguredTool(info.configured);
        setInstalledTools(tools);
      } catch {
        // Pure-vite dev without the Tauri backend: keep the static defaults.
      }
    })();
  }, []);
  const setDefaultTool = useCallback((tl: string) => {
    setDefaultToolState(tl);
    setConfiguredTool(tl);
    void api.setDefaultTool(tl);
  }, []);
```

`approveWriteTrigger`(1028-1041 行)透传工具,缺省用当前默认:

```ts
  const approveWriteTrigger = useCallback(
    async (item: WriteTrigger, tool?: string) => {
      setWriteTriggers((cur) =>
        cur.filter((w) => !(w.thread_id === item.thread_id && w.index === item.index)),
      );
      try {
        const dirId = await api.approveWriteTrigger(item.thread_id, item.index, tool ?? defaultTool);
        void dispatchDirection(dirId);
      } finally {
        await refreshNeeds();
      }
    },
    [dispatchDirection, refreshNeeds, defaultTool],
  );
```

Context value(1260 行附近)补 `configuredTool, installedTools`。

- [ ] **Step 4: ScopeReview.tsx 徽章改显示当前默认**

`ScopeLaneRow`(157 行)读 store,221-224 行徽章替换:

```tsx
function ScopeLaneRow({ lane, index }: { lane: ScopeLane; index: number }) {
  const { defaultTool } = useStore();
```

```tsx
          <span className="flex shrink-0 items-center gap-1.5 rounded-[var(--radius-sm)] bg-bg px-2 py-0.5 text-[11px] text-ink-muted">
            <ToolIcon tool={defaultTool} size={12} />
            {toolFullName(defaultTool)}
          </span>
```

- [ ] **Step 5: 构建验证**

Run: `npm run build`
Expected: 通过。SettingsDialog 此时仍用旧 `TOOLS` 常量渲染(Task 10 改),但 `setDefaultTool` 已写后端,类型兼容。

- [ ] **Step 6: Commit**

```bash
git add src/lib src/state src/board/ScopeReview.tsx
git commit -m "feat(store): defaultTool 改为后端解析来源,approve 透传工具"
```

---

### Task 9: Needs-you 卡片工具选择器

**Files:**
- Modify: `src/board/NeedsYouView.tsx:89-148`、`src/i18n/en.ts`(needs 段)、`src/i18n/zh.ts`(needs 段)

- [ ] **Step 1: i18n key**

`en.ts` 的 `needs:` 段加 `runWith: "Run with"`;`zh.ts` 对应段加 `runWith: "执行工具"`。

- [ ] **Step 2: WriteTriggerRow 加选择器**

顶部导入加 `cn`:

```ts
import { cn } from "../lib/cn";
```

`WriteTriggerRow`(89 行起)改为:

```tsx
export function WriteTriggerRow({ item }: { item: WriteTrigger }) {
  const { approveWriteTrigger, denyWriteTrigger, selectThread, defaultTool, installedTools } =
    useStore();
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);
  // null = follow the workspace default (which loads async at startup);
  // a string = the human explicitly picked a tool on this card.
  const [picked, setPicked] = useState<string | null>(null);
  const tool = picked ?? defaultTool;
  const installed = installedTools.filter((tl) => tl.installed);
  const context = [item.thread_title, item.name].filter(Boolean).join(" · ");
```

底部按钮行(125-145 行)在批准按钮后插入选择器,批准改传 `tool`:

```tsx
      <div className="flex flex-wrap items-center gap-2 border-t border-border bg-bg/40 px-3.5 py-2.5">
        <Button
          variant="primary"
          disabled={busy}
          title={t("needs.approveRunTitle")}
          onClick={() => void act(() => approveWriteTrigger(item, tool))}
        >
          <Check size={13} />
          {t("needs.approveRun")}
        </Button>
        {installed.length > 1 && (
          <div
            title={t("needs.runWith")}
            className="inline-flex items-center gap-0.5 rounded-[var(--radius-md)] bg-bg p-0.5"
          >
            {installed.map((tl) => (
              <button
                key={tl.tool}
                type="button"
                title={toolFullName(tl.tool)}
                onClick={() => setPicked(tl.tool)}
                className={cn(
                  "grid h-6 w-7 place-items-center rounded-[var(--radius-sm)] transition-opacity duration-150",
                  tool === tl.tool ? "bg-raised" : "opacity-40 hover:opacity-80",
                )}
              >
                <ToolIcon tool={tl.tool} size={13} />
              </button>
            ))}
          </div>
        )}
        <Button
          variant="ghost"
          className="ml-auto"
          disabled={busy}
          title={t("needs.denyWriteTitle")}
          onClick={() => void act(() => denyWriteTrigger(item))}
        >
          <X size={13} />
          {t("common.deny")}
        </Button>
      </div>
```

(只装了 0 或 1 个 CLI 时不渲染选择器——没有可选项;`tool` 初值即 defaultTool,批准仍正确。)

- [ ] **Step 3: 构建验证**

Run: `npm run build`
Expected: 通过

- [ ] **Step 4: Commit**

```bash
git add src/board/NeedsYouView.tsx src/i18n
git commit -m "feat(needs): 批准卡片工具选择器 — 子任务可逐条覆盖默认工具"
```

---

### Task 10: SettingsDialog 只列已安装 + 回退提示

**Files:**
- Modify: `src/nav/SettingsDialog.tsx:20-25,152-184`、`src/i18n/en.ts`(settings 段)、`src/i18n/zh.ts`(settings 段)

- [ ] **Step 1: i18n**

`en.ts` settings 段:

```ts
    defaultToolHint: "Default tool for issue leads and new sub-tasks. Each approval card can still override it.",
    noTools: "No coding CLI detected — install codex, claude, or opencode.",
    toolFallback: "{{configured}} is not installed — using {{tool}}.",
```

`zh.ts` settings 段:

```ts
    defaultToolHint: "issue 的 lead 与新子任务的默认工具;批准时可在卡片上逐条覆盖。",
    noTools: "未检测到编程 CLI,请先安装 codex / claude / opencode。",
    toolFallback: "{{configured}} 未安装,当前生效 {{tool}}。",
```

(`defaultToolHint` 为更新既有 key,另两个为新增。)

- [ ] **Step 2: SettingsDialog**

删除 20-25 行的 `TOOLS` / `TOOL_LABEL` 常量,导入 `toolFullName`:

```ts
import { toolFullName } from "../components/ToolIcon";
```

`GeneralSettings` 的 useStore 解构加 `configuredTool, installedTools`,函数体内(`pickDir` 之前)加:

```ts
  const installed = installedTools.filter((tl) => tl.installed);
```

默认工具一行(178-184 行)替换为:

```tsx
        <SettingRow label={t("settings.defaultTool")} hint={t("settings.defaultToolHint")}>
          {installed.length === 0 ? (
            <span className="text-[12px] text-waiting">{t("settings.noTools")}</span>
          ) : (
            <div className="flex flex-col items-end gap-1">
              <Segmented
                value={defaultTool}
                onChange={setDefaultTool}
                options={installed.map((tl) => ({ value: tl.tool, label: toolFullName(tl.tool) }))}
              />
              {configuredTool && configuredTool !== defaultTool && (
                <span className="text-[11px] text-waiting">
                  {t("settings.toolFallback", {
                    configured: toolFullName(configuredTool),
                    tool: toolFullName(defaultTool),
                  })}
                </span>
              )}
            </div>
          )}
        </SettingRow>
```

- [ ] **Step 3: 构建验证**

Run: `npm run build`
Expected: 通过

- [ ] **Step 4: Commit**

```bash
git add src/nav/SettingsDialog.tsx src/i18n
git commit -m "feat(settings): 默认工具只列已安装 CLI,显示回退与无 CLI 提示"
```

---

### Task 11: 收尾验证

- [ ] **Step 1: 后端全量测试**

Run: `cd src-tauri && cargo test`
Expected: 全部 passed

- [ ] **Step 2: 前端构建**

Run: `npm run build`
Expected: 通过

- [ ] **Step 3: 补丁卫生**

Run: `git diff --check && git log --oneline -11`
Expected: 无空白错误;11 个本计划提交按序在列

- [ ] **Step 4: 冒烟(可选,需桌面环境)**

Run: `npm run tauri dev`
手工确认:Settings 默认工具只列已安装项且默认 codex(若装了);新建 issue → lead 正常对话;lead 提案后 Needs-you 卡出现工具选择器,换选后批准,ThreadBoard 卡片显示所选工具。
