# 通用本地 Agent App 剥离设计

## 目标

将 Atlas 从当前的「Coding Agent 交付工作台」剥离为「通用本地 Agent App」。第一阶段只移除默认架构里的 coding 假设，不引入 HR、Pack、Scenario、业务插件系统或新的领域模型。

完成后的应用应支持用户在本地桌面 App 中创建普通任务，与 Claude / Codex / OpenCode 对话，查看会话输出，处理权限请求，管理 skills，并保留 settings、backup、notifications、IM bridge 等现有基础能力。应用默认不要求用户添加 repo，不创建 git worktree，不展示 diff，不运行 pre-PR checks。

## 非目标

- 不重写一套新架构。
- 不做 HR 业务能力。
- 不引入 Pack / Scenario 插件体系。
- 不实现动态 UI 插件系统。
- 不保留 coding flow 作为默认体验。
- 不在第一阶段删除 Claude / Codex / OpenCode provider 切换。
- 不要求兼容已有 Atlas coding 数据迁移到新模型；第三方仓库克隆分析场景下可优先支持 fresh DB。

## 当前架构中应保留的部分

### 桌面壳与基础设施

- Tauri v2 + React 前端壳。
- SQLite / SQLCipher 本地存储。
- OS Keychain 密钥管理。
- app settings、backup、i18n、notifications、updater。
- 当前 Settings dialog 的基础结构。

这些能力不依赖 repo/worktree/diff，可以作为通用 Agent App 的基础。

### 多 Provider Agent Runtime

保留现有 `AgentAdapter` 方向：

- Claude
- Codex exec
- Codex app-server
- OpenCode

前端继续提供 provider 切换。剥离目标不是 Codex-only，而是让多 provider runtime 脱离 coding delivery flow。

### Conversation Runtime

保留现有会话运行能力：

- message timeline
- streaming output
- native session id
- resume / inspect 能力
- tool call 状态展示
- interrupt / kill / protocol interrupt
- image attachment 能力

需要改变的是会话的宿主对象。现在会话绑定 `thread -> direction -> repo/worktree`，剥离后应绑定 `workspace -> task -> run`。

### Human Control

保留：

- Ask Bridge
- Needs-you queue
- permission answer
- auto decision
- dangerous/full access handling

这些能力对通用 Agent App 仍然关键。剥离后 Ask 仍需能关联到当前 task/run/session，而不是 repo/direction。

### Skills

保留现有能力：

- git-backed skill source
- skill sync
- global / workspace enable
- skill injection into `.agents/skills` and `.claude/skills`
- idle-time skill refresh

第一阶段只继续把 skill 当作工作方式和 agent 能力注入，不扩展成 Pack。

### Optional IM Bridge

IM bridge 可保留为可选入口。需要把文案从 Issue 绑定逐步泛化为 Task 绑定，但第一阶段可以保留内部表名并改 UI 文案。

## 当前架构中应剥离的 coding 假设

### Repo 作为 workspace 核心对象

当前 workspace 默认围绕 `repo_ref` 运转。通用 Agent App 不应要求 workspace 里必须有 repo。

剥离要求：

- 新用户可以创建空 workspace。
- workspace 首页不再默认引导 add/clone/create repo。
- repo map 不再是默认首页 tab。
- repo 相关操作从通用 navigation 中移除或隐藏。

### Direction 绑定单一 write repo

当前 direction 的语义是「一个执行方向写一个 repo」。通用 App 里 run 不应绑定 repo。

剥离要求：

- 新的 run 不需要 `repo_id`。
- run 的 cwd 可以是 workspace-level app managed cwd，或用户选择的普通文件夹。
- lead/planner prompt 不再要求 `ONE repo` 或 `write repo`。
- worker 启动不再依赖 materialized worktree。

### Git worktree / branch / diff

以下能力应从默认 core flow 移除：

- `materialize_direction`
- `git worktree add/remove`
- namespaced branch
- worktree path layout
- diff panel
- repo patch
- diff annotation

第一阶段可以保留源码文件但不从默认 UI 和 task flow 调用。后续若需要 coding extension，再把这些能力重新隔离。

### Checks / pre-PR / review skill 语义

当前 verification 以 repo manifest 推断 lint/typecheck/build/test。这是 coding-specific。

剥离要求：

- task/run 完成不自动跑 repo checks。
- board 不再把 review column 和 pre-PR review 作为通用流程。
- global review skill setting 不作为默认通用 App 主流程暴露。
- review skill 可保留为普通 skill，不自动绑定到 run 状态流转。

### Repo profile / dependency graph / scope review

以下能力应从默认体验剥离：

- repo profile
- repo graph
- dependency edges
- scope review
- write/read repo lanes
- frontend/backend/shared split hints

这些属于 coding delivery planner，不属于通用 Agent App。

## 目标信息架构

第一阶段采用最小通用模型：

```text
Workspace
  Task
    Run
      Session
        Message / Event
```

### Workspace

表示一个本地工作空间。它不再等价于「一组代码仓库」。它可以只是一个用户管理任务、skills、settings、IM routes 的容器。

### Task

替代当前用户可见的 Issue / Thread。Task 是用户想让 agent 处理的一件事。

第一阶段 Task 需要：

- title
- kind
- status
- created_at / updated_at
- default provider
- optional workspace notes

### Run

替代当前用户可见的 Direction。Run 是某个 task 下的一次 agent 执行或工作流分支。

第一阶段 Run 需要：

- name
- tool/provider
- status
- optional instruction
- optional cwd
- optional reason/summary

Run 不绑定 repo，不绑定 worktree，不要求 diff。

### Session

保留现有 session 语义，但 foreign key 逐步从 `direction_id` 迁移到 `run_id`。

### Message / Event

保留现有 lead_message 的时间线能力，用户可见文案改成 generic chat/event。

## 命名过渡策略

为了降低改造风险，第一阶段不强制一次性重命名所有后端表。

建议分两层推进：

1. 用户可见层先泛化。
   - Issue -> Task
   - Direction -> Run
   - Repo Map -> hidden or removed from default navigation
   - Diff / Checks -> hidden from default task flow

2. 后端实体再逐步迁移。
   - `thread` 可先作为 Task 的存储表继续存在。
   - `direction` 可先作为 Run 的存储表继续存在，但必须移除新建 run 时对 `repo_id` 的依赖。
   - `repo_ref` / `repo_profile` / `worktree` 保留为 unused legacy tables，直到后续 migration 再删除。

这样可以减少一次性大迁移风险，并让前端先形成正确产品形态。

## 用户流程

### First run

当前 first-run 以 repo onboarding 为中心。新流程应改成：

1. 检测 Claude / Codex / OpenCode 可用性。
2. 选择默认 provider。
3. 创建或选择 workspace。
4. 可选添加 skill source。
5. 进入空白任务列表。

### Create Task

用户创建普通 task：

1. 输入标题。
2. 选择 provider。
3. 可选选择工作目录或不选择目录。
4. 进入 task chat。

不要求 repo，不要求 scope review。

### Run Agent

用户在 task 中发消息：

1. App 根据 provider 启动或续接 session。
2. 注入 enabled skills。
3. 注入 Ask Bridge / bus capability。
4. 将输出写入 timeline。
5. 若 agent 请求权限，进入 Needs-you。

### Needs-you

Needs-you 继续作为全局待处理入口，但卡片文案应关联 task/run，而不是 issue/direction/repo。

### Skill Management

Settings 中保留 Skills 页面。scope 第一阶段仍可沿用 `global` 和 `ws:<id>`，不引入 task/run scope。

## 后端设计

### Store

第一阶段可以保留现有表，但改变新路径对字段的依赖。

保留：

- `workspace`
- `thread` as task-compatible storage
- `direction` as run-compatible storage
- `session`
- `lead_message`
- `skill_source`
- `skill_enable`
- `app_setting`
- `backup_config`
- `im_route`

停止默认依赖：

- `repo_ref`
- `repo_profile`
- `worktree`

需要改造：

- 创建 run 时允许无 repo。
- 打开 session 时允许无 worktree cwd。
- session cwd 从 app-managed task/run directory 或 user-selected cwd 获取。
- task overview 不再按 direction worktree/check 状态聚合。

### CWD 策略

通用 Agent App 仍需要 cwd，因为底层 CLI 通常需要一个工作目录。

第一阶段推荐：

```text
~/.atlas/workspaces/<workspace-slug>/tasks/<task-slug>/runs/<run-slug>
```

该目录由 Atlas 管理，不是 git repo。skills 和 ask injection 可以写入该目录下的 provider config 目录。若用户主动选择一个本地文件夹，则该 run 使用用户指定 cwd。

### Bus / Planner

保留 bus 的消息能力，移除 planner MCP 的 repo map / propose directions 默认依赖。

第一阶段可以提供更简单的 planner tools：

- `get_task`
- `set_task_status`
- `ask_human`
- `bus_post`
- `bus_inbox`

不默认提供：

- `get_repo_map`
- `propose_directions`
- repo scope tools

### Lead Prompt

Lead prompt 应从 coding planner 改成 generic coordinator：

- 理解用户 task。
- 必要时澄清。
- 可以建议创建 run。
- 可以直接对话。
- 不提 repo、worktree、diff、PR。
- 不要求 frontend/backend/shared 分工。
- 保留 permission/Needs-you 说明。

## 前端设计

### Navigation

保留：

- top bar
- workspace switcher
- settings
- needs dock
- chat/session view

改造：

- Workspace home 从 repo/issue board 改为 task list。
- Thread board 改为 task detail。
- Direction card 改为 run card。
- Session view 去掉默认 diff button。
- Repo map tab 从默认 UI 移除。

### Task UI

第一阶段只需要：

- task list
- task detail
- chat timeline
- composer
- run status
- provider icon
- needs-you badge

不需要 HR 专用 UI，也不需要 artifact 专用编辑器。

### Settings UI

保留：

- general
- appearance
- automation if generic
- skills
- IM
- backup

需要隐藏或改写：

- review skill 的 pre-PR 文案
- auto review 文案
- repo effective config 文案

## 错误处理

- Provider 未安装：沿用 tool detection，UI 显示不可用 provider。
- CWD 创建失败：task/run 创建失败并显示具体路径错误。
- Skill injection 失败：保持 best-effort，但在 session event 中记录 warning。
- Ask Bridge 注入失败：允许 session 继续，但显示权限桥接不可用 warning。
- Legacy repo data 存在：不阻止启动，只在 coding legacy UI 隐藏后不展示。
- Provider 启动失败：写入 timeline error event，run 标记 failed。

## 验证策略

### 自动化验证

第一阶段应覆盖：

- Rust tests for adapter selection still pass.
- Rust tests for skill parsing/enabling/injection still pass.
- New store tests for run without repo.
- New tests for app-managed cwd path generation.
- TypeScript build passes.
- Existing tests that assume worktree-required direction should be updated or scoped to legacy coding behavior.

### 手动验证

需要在桌面 App 中验证：

1. First run 不要求添加 repo。
2. 创建 workspace。
3. 创建 task。
4. 选择 Claude / Codex / OpenCode 中至少一个已安装 provider。
5. 发起对话。
6. 能看到 streaming output。
7. 权限请求进入 Needs-you。
8. 启用 skill 后，新会话能注入 skill。
9. Settings / backup 页面仍可打开。
10. 默认 UI 不出现 repo map、worktree、diff、checks、PR review 主流程。

## 实施顺序建议

### Phase 1: 行为剥离

- 改 first-run 和 workspace home，不再要求 repo。
- 创建 task/run 时允许无 repo。
- session 使用 app-managed cwd。
- 隐藏 repo map、scope review、diff、checks 的默认入口。
- 改 lead prompt。

### Phase 2: 文案与类型泛化

- 前端用户可见文案从 Issue/Direction 泛化为 Task/Run。
- TypeScript 类型逐步引入 `Task` / `Run` alias，降低直接重命名风险。
- Settings 文案去 coding。

### Phase 3: 后端模型清理

- 新增 task/run 表或迁移 thread/direction。
- 移除新路径对 repo/worktree 表的依赖。
- 将 repo/worktree/diff/check 相关模块标记为 legacy coding extension or remove.

第一阶段实现时优先完成 Phase 1 和必要的 Phase 2。Phase 3 可以在行为稳定后继续。

## 成功标准

- 用户可以在没有任何 git repo 的情况下使用 App。
- 用户可以创建 workspace、task、run，并与任意可用 provider 对话。
- Claude / Codex / OpenCode provider 切换仍存在。
- Skills 管理和注入仍可用。
- Ask / Needs-you 仍可用。
- 默认 UI 和默认 prompt 不再出现 repo/worktree/diff/check/PR 作为核心流程。
- 现有 coding 相关源码可以暂时存在，但不参与默认通用路径。
