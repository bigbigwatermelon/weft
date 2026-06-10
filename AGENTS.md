# Codex 启动包 — Weft(coding-agent 驱动的多仓需求交付中心)

配套架构文档:`多仓多工具会话编排器-架构设计与可行性.md`(完整设计与可行性)。本启动包是它的"开工版",只保留落地必需信息,并已同步最新决策(lead/worker、主 agent 为家、surface 解耦、两级看板、i18n、产品化屏蔽)。

工作名 **Weft**(字标可换);定位:**本地优先、无服务端、coding-agent 驱动的多仓需求交付中心**。

<<<<<<< Updated upstream
---
||||||| Stash base
- `src/`: React + TypeScript UI. Key areas: `board/` for workspace/thread boards, `session/` for chat/observe/diff surfaces, `components/` for shared UI, `i18n/` for English/Chinese strings.
- `src-tauri/src/`: Rust backend. Key modules: `lead_chat/` for headless agent sessions, `store/` for SQLite/SeaORM entities and migrations, `bus/` for local MCP/thread bus, `git.rs` and `materialize.rs` for worktree handling.
- `src-tauri/tests/`: Rust integration tests.
- `assets/`, `public/`, `designs/`: screenshots, icons, generated diagrams, and design references.
=======
- `src/`: React + TypeScript UI. Key areas: `board/` for workspace/issue boards, `session/` for chat/observe/diff surfaces, `components/` for shared UI, `i18n/` for English/Chinese strings.
- `src-tauri/src/`: Rust backend. Key modules: `lead_chat/` for headless agent sessions, `store/` for SQLite/SeaORM entities and migrations, `bus/` for local MCP/thread bus, `git.rs` and `materialize.rs` for worktree handling.
- `src-tauri/tests/`: Rust integration tests.
- `assets/`, `public/`, `designs/`: screenshots, icons, generated diagrams, and design references.
>>>>>>> Stashed changes

## 0. 可直接粘贴给 Codex 的 kickoff prompt

> 在一个空仓库里启动 Codex,把架构文档放成 `ARCHITECTURE.md`、本启动包的约束放成 `AGENTS.md`,然后贴下面这段。

```
你将从零实现 Weft:一个本地优先、无服务端的桌面应用——coding-agent 驱动的多仓需求交付中心。
完整设计见 ARCHITECTURE.md,这份 AGENTS.md 是开工约束。

技术栈(已锁定,不要更换):
- Tauri v2(Rust 后端 + 前端 React + TypeScript + Vite)
- 终端:xterm.js(前端) + portable-pty(Rust),PTY 双向
- 本地状态:SQLite(tauri-plugin-sql 或 sqlx)
- git:直接调用系统 git 的 worktree 子命令
- i18n:react-i18next(或 FormatJS),中/英从第一天内建

不可违背的理念:
1. 原生驱动各 CLI(Codex / codex / opencode),不走 ACP,保全 hooks/skills 等原生能力。
2. 不重绘 agent 输出:原生 TUI 跑在 PTY 里;能嵌就嵌,工具有更好的 app 就深链跳过去。
3. 跨仓接线只用临时启动参数,绝不写进 canonical 仓的受控配置。
4. 物化用 git worktree;分支命名空间含 thread 维度。
5. 产品化:屏蔽机制(worktree/PTY/MCP/add-dir),呈现决策与结果(scope/分支/PR/diff/工具)。机制退到 Inspect。
6. 层级:Workspace ⊃ Thread(工作线/需求)⊃ Direction(方向)⊃ Session(工具×worktree)。
   会话有 role=curator|lead|worker:curator=workspace 维护仓库地图,lead=主 agent(只读纵览+规划+驱动 worker),worker=方向执行体。
7. Automation-first:lead 默认自动分解→spawn→派发→驱动到交付,不自加审批关;唯一阻塞来自工具自身权限(透传)。质量靠"可执行验证 + 确定性升级判据 + 跑飞护栏",不是人点头。
8. 入口抽象 = Task(任意粒度意图:PRD/bug/重构/spike/链接,PRD 只是一种);交付是**分阶段**的——当前止于 Task→PR,北极星是 Task→上线(开 PR → 合并 → 跨环境部署 staging→production)。合并/部署**不重造 CI/CD**,而是**编排仓库现有流水线**,并受可配置的不可逆边界(合并受保护分支、生产部署)把关。规划下沉给 plan skill(superpowers 等),编排借 Dynamic Workflows 原理但不重实现。

第一步只做 M1 垂直切片(见下"构建顺序"):单工具在一个 git worktree 里创建可交互会话,
能打字、能 Ctrl-C、关闭后能 resume 回同一会话。跑通并过验收后再往后做。

每个 milestone:补测试、自测通过、给我改动说明。先规划再动手。所有面向用户的字符串走 i18n,不要硬编码。
```

---

## 1. 锁定的技术决策(不要重新讨论)

- 外壳 Tauri v2;前端 React+TS+Vite;PTY 用 portable-pty;状态 SQLite;i18n react-i18next。
- 工具驱动:**spawn 原生 CLI 进 PTY(交互)** + **旁路读会话日志/SSE(结构化,只读)**。
- 物化:**git worktree**,分支 `ws/<workspace>/<thread>/<direction>`。
- 三家(Codex / Codex / OpenCode)**同为第一批**,统一 `ToolDriver` 抽象。
- 会话 **role = curator | lead | worker**;主 agent(lead)默认绑 **Codex**,thread 可覆盖;curator 默认绑快/省模型。
- **Automation-first**:lead 默认自动分解→spawn→派发→驱动到交付,**Weft 不自加审批关**。唯一阻塞来自工具自身权限(透传)+ 可配置的不可逆边界(合并受保护分支、生产部署等)。人随时可介入,非必经 gate。
- surface 与 observation 解耦;**中/英 i18n 两层**(UI 文案 + agent 产出语言)。
- 本地优先、无服务端、无身份;团队共享走配置下发(git/marketplace),低优先级。
- **入口 = Task**(PRD/bug/重构/spike,PRD 只是一种);**交付分阶段:当前 Task→PR,北极星 Task→上线**(开 PR → 合并 → 跨环境部署 staging→production);合并/部署通过**编排仓库现有流水线**达成,不重造 CI/CD,受不可逆边界把关。
- **质量闭环**:Weft 内只做轻量 pre-PR 检查(lint/type/unit/contract);权威 review/CI = 仓库现有 PR harness,不重造。合并后的部署同样**编排现有 CD 流水线**(预发→生产),Weft 驱动 + 观测,不重写发布系统。
- 规划下沉给 plan skill(superpowers 等);编排借 Dynamic Workflows 原理(编排即代码+结构化 handoff),不重实现;Codex worker 叶子层可自用 DW。

---

## 2. MVP 范围

**In**
- 数据模型 + SQLite:Workspace / Thread(type, leadAgent)/ Direction(write/read repos, tool, workerMandate)/ Session(role, surface, nativeSessionId)。
- worktree 编排:创建/列出/删除,分支命名空间化,按仓 diff。
- 三家 ToolDriver:spawn + resume + PTY 双向 + 旁路事件归一化 + open_surfaces。
- **curator/lead/worker**:curator 维护仓库地图(Repo Profile + 依赖图);lead 只读纵览 → 出 scope + 方向 brief → **自动拉起 worker(automation-first)**;worker 按 mandate 执行。
- **质量闭环**:acceptance 可执行化 → worker 完成=检查绿(非自报)→ 验证阶梯(lint/type/unit/contract/review-agent)→ 有界自动重试 → 确定性升级判据 → 跑飞护栏(预算/loop detection/爆炸半径)。
- **主 agent 为家**的主界面 + 会话面板交互(4.3:焦点/键位/审批/注入/composer)。
- scope 确认步(全仓 write/read/none → 懒物化)。
- **agent-first 看板,两级**:Workspace 板(cards=thread + 仓争用)+ Thread 板(cards=direction);Needs-you 重心;卡自动流转、人只做动作。
- thread bus(本地 MCP)+ coordinator 注入队列(基础版)。
- 配置物化(team skills/rules + PLAN.md 注入)。
- **i18n 中/英**(UI 文案 + agent 产出语言偏好)。
- 产品化屏蔽(机制进 Inspect,产品词在台前)。

**Out(MVP 不做)**
- 复杂的全自动跨工具编排引擎做基础版即可(automation-first,但 DAG/重试/契约传播先做够用;不必一步到位)。
- 团队实时协作 / 团队看板 / 服务端 / 遥测。
- **合并 → 跨环境部署(staging→production)/ release 的全自动驱动**——属北极星路线图,但**不在 MVP**;MVP 仍止于 Task→PR。权威 review/CI 用仓库现有 harness;落地时 Weft **编排**现有 CD 流水线驱动 + 观测,绝不自建发布系统。
- 远程项目;合并冲突自动解;RTL。

---

## 3. 仓库脚手架(建议)

```
/src-tauri            Rust 后端
  /git                worktree 管理、diff
  /pty                PTY 会话管理、输入仲裁
  /drivers            ToolDriver: Codex.rs / codex.rs / opencode.rs + sidecar 解析 + open_surfaces
  /roles              lead 编排(survey/scope/brief/dispatch)、worker 执行
  /bus                thread bus 的本地 MCP server + coordinator 注入队列
  /materialize        scope → worktree + add-dir 参数 + 资产注入
  /store              SQLite schema + 仓储
/src                  React 前端
  /home               主 agent(lead)对话为家
  /panels             xterm 会话面板 + 交互层(4.3) + Open in…(surface)
  /board              agent-first 看板(workspace 级 + thread 级)
  /scope              scope 确认步
  /diff               按仓 diff/PR 视图
  /inspect            机制逃生舱(worktree 路径/开终端/ACP-style 日志)
  /i18n               en / zh 资源 + 运行时切换
ARCHITECTURE.md / AGENTS.md
```

---

## 4. 核心抽象:ToolDriver(三家差异收敛于此)

```rust
struct SpawnSpec {
  cwd: PathBuf,                 // = 某 worktree(worker)或只读视图根(lead)
  read_dirs: Vec<PathBuf>,     // 只读挂载兄弟仓(临时参数,绝不写进仓)
  role: Role,                  // Lead | Worker
  resume_id: Option<String>,   // None=新建; Some=resume
  mcp_servers: Vec<McpRef>,    // 含 thread bus;lead 另挂 planner MCP
  lang: Lang,                  // 注入 agent 产出语言偏好(zh|en)
}

trait ToolDriver {
  fn id(&self) -> Tool;                               // Codex | Codex | OpenCode
  fn spawn(&self, s:&SpawnSpec) -> Result<PtyHandle>;
  fn capture_native_session_id(&self, h:&PtyHandle) -> Option<String>; // 回流 CLI + 深链
  fn sidecar(&self, s:&SpawnSpec) -> Box<dyn EventStream>; // 旁路结构化事件
  fn open_surfaces(&self, s:&Session) -> Vec<Surface>;     // Open in… 能力
}

enum Surface { AppDeepLink(Url), WebUI(Url), Editor(PathBuf) }
// Codex: codex://threads/<nativeSessionId>(best-effort,archived 会失败,需兜底)
// OpenCode: 本机 server 的会话页;Codex: 无会话级 app-link → 降级 Editor(worktree)
```

各家命令映射:
- **Codex**:`Codex` (+ `--add-dir <read_dirs>`,resume `--resume <id>`);sidecar=tail `~/.Codex/projects/<编码cwd>/*.jsonl`。lead 首选(多目录 + subagents 强)。
- **Codex**:`codex` (+ `--add-dir`/`-C`,resume `codex resume <id>`);sidecar=tail `~/.codex/sessions/.../*.jsonl`;**别用 CODEX_HOME 隔离(resume bug #5247)**。
- **OpenCode**:`opencode --dir <cwd>`(resume `--session`/`--continue`);sidecar=订阅本机 `/event` SSE。多根弱 → 当 worker,不当 lead。

事件归一化:`NormEvent { Started{id}, Message, ToolCall, FileChanged{repo,path,+,-}, ApprovalRequested{cmd}, BusMessage, Idle, Exited }`。

lead/worker 协作:lead 用 planner MCP(`survey_repos`/`declare_scope`/`propose_directions`)产出结构化 scope+brief;worker 经 thread bus 回报**结构化摘要 + diff stat**(lead 绝不吞 worker 原始 transcript)。

PTY 输入仲裁(4.3):人输入直通;程序注入入队,人空闲且非回合中才 flush,bracketed paste 整块写入。

---

## 5. 构建顺序(每步带验收标准)

### M1 — 垂直切片:单工具端到端
worktree(`ws/demo/t1/main`)→ ClaudeDriver spawn 交互会话 → xterm 渲染。
- **验收**:面板里打字改文件;Ctrl-C;关闭后 `--resume` 在同一 cwd 接回历史继续;worktree 内见改动。

### M2 — worktree 编排 + 数据模型
Workspace/Thread/Direction/Session 落 SQLite;worktree 建/列/删 + 按仓 diff。
- **验收**:一个 thread 下 2 个 direction(不同仓不同分支)互不干扰;删 thread 清理全部 worktree;同仓被两个 thread 各开 worktree 不冲突。

### M3 — 三家 ToolDriver + 旁路归一化 + Surface
补 Codex/OpenCode driver;sidecar → NormEvent;实现 open_surfaces。
- **验收**:同一 thread 并排跑三家会话,各自可交互;右栏从 NormEvent 聚合按仓 diff/状态;ApprovalRequested 触发审批态;Codex 卡能 `codex://threads/<id>` 跳 app 且 Weft 经 sidecar 仍同步(跳走时 PTY 脱挂)。

### M4 — 会话面板交互层(4.3)
焦点模型、键位归属(只截 ⌘ 前缀)、审批快捷条(按钮回写 y/n)、composer(bracketed paste)、注入队列 banner。
- **验收**:多面板焦点唯一可见;斜杠/↑历史透传 TUI 正常;点 Approve 等于敲 y;长多行 prompt 经 composer 一次性送入不乱码。

### M5 — lead/worker + scope 懒物化 + 主 agent 为家 UI
lead 只读纵览 → scope 确认步 → 仅 write 仓建 worktree、read 只读挂载、none 不挂;lead 出 brief →(人确认)→ 拉起 worker(mandate:plan+impl / impl-only);主界面以 lead 对话为家,worker 在执行车间,impl-only 默认折叠成 diff。
- **验收**:plan 标 none 的仓零 worktree;主对话里能审 scope/方向并一键拉起 worker;impl-only worker 折叠为 diff/状态、可展开;**automation-first(默认自动 spawn/驱动,不插 Weft 审批;只透传工具自身权限请求)**。

### M6 — agent-first 看板(两级)+ 配置下发 + i18n
Workspace 板(cards=thread + 仓争用 + Needs-you 聚合)+ Thread 板(cards=direction);卡随 session/git 状态自动流转,人只做动作;跨 thread 重叠告警;thread bus + coordinator 唤醒;按仓 PR/合并/清理;配置下发(Codex `/plugin marketplace`、Codex `codex marketplace`、OpenCode `opencode-remote-config`/npm);中/英 i18n 全量。
- **验收**:两级看板缩放联动;Needs-you 在 workspace 级聚合所有 thread 阻塞;两个 thread 改同仓时仓争用条与卡片给出重叠告警;切到 EN 后 UI 全量翻译、agent 新产出按所选语言;有效配置预览标出 skill/rule 来自团队/个人/仓哪层。

---

## 6. 验证要求(每个 milestone)

- 单元:git/worktree、scope→物化映射、事件归一化、注入仲裁状态机、open_surfaces 构造。
- 集成:各 CLI 真实二进制跑 spawn/resume 冒烟;Codex 深链跳转冒烟。
- i18n:中/英全量切换无漏译;状态枚举内部保持英文、仅 UI 映射;agent 产出语言随偏好。
- 手动验收:逐条过每个 milestone 的"验收"。
- 关键回归:Codex resume 的 cwd 一致;不要 CODEX_HOME 隔离;接线不落 canonical 仓;lead 上下文不吞 worker 原文。

---

## 7. 已知坑(提前规避)

- Codex `--resume` 依赖 cwd 编码一致 → worktree 路径必须稳定。
- Codex `CODEX_HOME` + `codex resume` 有 bug(#5247)→ 回流的会话用标准 home + `--add-dir`。
- Codex 深链契约未完全稳定、archived thread 静默失败 → 当 best-effort,失败要兜底提示。
- TUI(Ink / Bubble Tea)高频重绘 → PTY 输出做合帧/批处理(参考 Codeman 管线),否则 xterm.js 闪。
- 同一分支不能在两个 worktree 同时检出 → 分支必须含 thread 维度。
- 每 worktree 一份依赖 → 懒装/链接共享,控制磁盘。
- **lead 上下文爆炸**:跨仓 scope 分解靠 survey 工具(file-tree/grep/定向读)+ 轻量索引,别 ingest 全部——这是"跨仓 scope 自动分解"这个核心 wow 的成败点,早做原型。
- **brief 质量 = 产品天花板**:总 plan → 方向 brief 的翻译当一等产物打磨,颗粒度匹配 mandate。
- i18n 别只做 UI:agent 产出语言是第二层;代码/标识符始终英文。
- 隐藏机制必须配"失败可读 + 就地逃生舱",否则抽象一漏用户就卡死。
- 别重造 PR review/CI/CD:worker 开 PR 时仓库现有 hooks/CI 自然触发(Weft 驱动原生 CLI 不绕 hooks),Weft 只做轻量 pre-PR 检查 + 观测 PR/CI 状态。**北极星的合并 + 跨环境部署同理——编排仓库现有 CD 流水线(预发→生产),绝不自建发布系统。**
- automation-first 但要有"跑飞护栏":每 thread/direction 预算上限 + 相同失败 loop detection + 不可逆边界可配置,否则全自动会烧钱/失控。
