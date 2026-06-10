# 多仓 · 多工具 · 会话编排器 —— 架构设计与可行性评估

> 一句话定位:一个**本地优先、无服务端**的桌面端(Tauri / 本地 Web)产品,把分散的代码仓库按**逻辑工作区**组织起来,用 **git worktree** 做物化与隔离,在每个仓/每个执行方向上 **headless 驱动原生的 Claude Code / Codex / OpenCode**(允许异构),以**产品自有的会话界面**实时呈现;任何会话都可随时在用户自己的终端接管;团队共享通过**配置下发**(git / plugin marketplace)实现。

本文档整合了多轮讨论的全部结论,并对关键技术假设做了可行性核验(见第 7 节)。

---

## 1. 设计原则(锚定决策)

1. **本地优先、无服务端、无用户身份**。每个人跑自己的本地实例,本地状态库(SQLite/JSON)保存工作区、worktree、会话映射。没有中心账号/同步服务。
2. **原生驱动,保全能力**。直接跑各 CLI 本体(而非走 ACP 统一层),从而保留 hooks / skills / subagents / 审批等全部原生能力。
3. **headless 驱动,产品自有会话 UI**。经各工具官方的结构化流接口驱动(claude stream-json;codex `exec --json`;opencode `run --format json`),产品渲染自己的会话时间线——不内嵌终端、不重绘 TUI。原生 TUI 体验经"在终端接管"逃生舱保留(停引擎 + 复制 resume 命令到用户自己的终端)。
4. **分组是逻辑的,不是物理的**。工作区是一份引用清单,不是一个 git 父仓 / submodule 集合;仓可被多个工作区重叠引用。
5. **共享靠下发,不靠后端**。团队基线(skills/rules/清单)作为版本化产物经 git 或 plugin marketplace 分发;个人配置留在本机。
6. **异构可接受**。不强求"单会话内多工具";不同仓/不同方向用不同工具,靠共享文件系统协同。
7. **接线只活在物化层,绝不写进 canonical 仓**(见 2.1)。工作区的跨仓挂载与会话配置只通过临时参数/一次性 worktree 注入,保证单仓独立、工作区互不污染、零累积。
8. **产品化:屏蔽机制,呈现决策与结果**(见 4.7)。隐藏 plumbing(worktree/headless 进程/MCP/add-dir/旁路),用产品词表达;但用户负责的"决策与结果"(scope、分支/PR/diff、工具选择)留在台前。隐藏三件套:抽象 + 逃生舱(真路径/开终端)+ 失败可读。
9. **多语言(中/英)从第一天就内建**(见 4.8)。UI 文案外置 + agent 产出语言两层都要,绝不硬编码字符串。
10. **Automation-first,Weft 不自加审批关**。产品北极星是自动化:lead 默认自动分解→spawn→派发→驱动到交付。**唯一的阻塞性人工来自工具自身的权限/审批习惯**(Codex/Claude 按用户自己的配置弹出),Weft 只透传(见 4.3),不新增 gate。人是监督/随时介入/异常处理,不是必经关卡。仅"不可逆/爆炸半径大"的动作(合并受保护分支、破坏性/资金操作)留**可配置**边界,默认由用户定;git/CI/工具权限已是安全网。
11. **交付边界 = Task → PR(当前阶段只到代码交付;入口是 Task,PRD 只是一种)**。merge / CI-CD / release 交给人 + 仓里现有 harness。因 Weft 驱动原生 CLI(不绕 hooks),worker 开/更新 PR 时**仓库自身的 PR 触发 hooks/CI 自然触发**——交付即自动接上现有 PR harness,接缝在 PR,无需 Weft 协调。CI/CD 反应式、更高维 harness 为**未来维度**(架构经 git/forge+bus 已可容纳),当前 out of scope。

---

## 2. 核心概念与数据模型

```
Workspace (逻辑清单, 可重叠)
├── repos: [ RepoRef ]                  # 按 .git 引用, 非拷贝
│     ├── id / name
│     ├── gitUrl 或 localGitPath        # 指向对象库(.git)
│     ├── baseRef (branch/commit)       # 可复现"钉版本", 替代 submodule
│     └── defaultTool (claude|codex|opencode|none)
├── assets                              # 工作区资产层(产品管理, 物化时注入)
│     ├── teamSkills[]  (下发基线)
│     ├── rules (AGENTS.md / CLAUDE.md 内容源)
│     └── blackboardTemplate (PLAN.md / WORKSPACE_CONTEXT.md 模板)
├── leadAgentDefault (tool+model+mode)   # 主 agent 默认绑定(默认 Claude Code, thread 可覆盖, 见 4.4)
├── threads: [ Thread ]                  # 工作线(并行单元), 可并行多个(见 5.2)
│     ├── id / title / status (active|paused|archived)
│     ├── task (seed 意图: PRD | bug | 重构 | spike | 链接… — 入口抽象, PRD 只是一种)
│     ├── type (feature | bugfix | refactor | spike | ...)  # 由 task 分类, 决定规划仪式轻重
│     ├── leadAgent (override?)           # 该 thread 的主 agent(默认继承 workspace)
│     ├── plan?: Plan                     # 可选: feature task 走完整规划; bug task 可跳过
│     │     ├── body (该 thread 自己的 PLAN.md 黑板)
│     │     └── scope: { [repoId]: write | read | none }   # 每仓角色, 人可改
│     └── directions: [ Direction ]      # 0..N: 大 feature 多方向; 小改一个甚至直接一个 session
│           ├── name
│           ├── writeRepoIds[]           # 确定要改 → 开 worktree+分支
│           ├── readRepoIds[]            # 只读参考 → 只读挂载, 不建 worktree
│           ├── tool                     # 该方向用什么工具(可异构)
│           ├── workerMandate (plan+implement | implement-only)  # 见 4.4
│           └── branch                   # ws/<workspace>/<thread>/<direction>
└── distribution                        # 团队下发来源
      ├── marketplaceRef (git)          # plugin marketplace 仓
      └── version / pinned

Materialization (按 thread×方向 运行时生成)
├── viewRoot (伞形根: 该 thread×方向的临时目录)
│     ├── <repoA-worktree>/             # git worktree, 分支含 thread 维度
│     ├── <repoB-worktree>/
│     ├── PLAN.md / WORKSPACE_CONTEXT.md  # 该 thread 的黑板
│     └── (注入的 team skills / rules)
└── 说明: 同一 .git 可派生多个 worktree → 跨 thread 并行 + 仓重叠都零拷贝解决

Session (会话叶子: 工具 × worktree)
├── id (本地)
├── workspaceId / threadId / directionId
├── role (curator | lead | worker)      # curator=维护地图; lead=规划+驱动; worker=可写 worktree(见 4.4/4.11)
├── surface (chat | external-app | external-terminal)  # 交互界面在哪(见 4.5)
├── tool  +  cwd(= 某 worktree, 稳定唯一)
├── nativeSessionId                     # 各工具自己的会话 id(回流 CLI + 深链)
├── engine (chat 引擎回合态)  +  sidecar(旁路结构化通道)
└── status / 关联的 worktree 分支
```

**配置分层(无身份, 靠来源+作用域优先级)**

| 层 | 来源 | 落到哪个作用域 | 谁维护 |
|---|---|---|---|
| 团队基线 | 下发(git/marketplace,只读) | 项目作用域(物化进 viewRoot 的 `.claude/skills`、`AGENTS.md` 等) | 团队仓 + PR |
| 个人覆盖 | 本机 | 用户作用域(`~/.claude/skills`、`~/.config/opencode/skills` 等) | 每个人自己 |
| 仓内既有 | 各仓自带 | 仓目录层级 | 仓 owner |

有效配置 = 团队基线 ⊕ 个人覆盖 ⊕ 仓内既有,由**工具自带的作用域合并规则**在本地解析(无需服务端对账)。产品提供"有效配置预览"让用户看清最终生效了哪些、来自哪层。

### 2.1 关键隔离原则:接线绝不写进 canonical 仓

跨仓挂载(add-dir)有"临时"与"持久"两种形态,**编排器只用临时形态**,否则会"负重前行":

| 工具 | 临时形态(用这个) | 持久形态(别写进仓) |
|---|---|---|
| Claude Code | `--add-dir <path>`(启动 flag)、`/add-dir`(会话内)——仅当前会话,不落盘 | `additionalDirectories` 写进仓的 `.claude/settings.json`(提交后永久跟随该仓) |
| Codex | `--add-dir` / `-C` / `-c key=val`(启动参数)——临时 | `writable_roots` 写进仓的 `config.toml` |

**规则**:工作区的跨仓接线与会话级配置,只活在 **(a) 临时启动参数 / `--settings` / env**,或 **(b) 一次性 worktree 内的 gitignore local 配置**(随 worktree 用完即删)。**永远不进 canonical 仓的受版本管理配置。**

由此得到三条保证:

1. **单独改一个仓**:直接打开 canonical 仓,只加载该仓自身 + 用户配置,看不到任何工作区接线 → 干净。
2. **推进 workspace B**:B 物化自己的 worktree + 自己的启动参数,不继承 A 的任何接线。
3. **零累积**:没有东西被持久化进仓,不会越积越多。

> 注意坑:Codex 想用 `CODEX_HOME` 做每-workspace 配置隔离时,会与 `codex resume` 冲突(已知 bug #5247)。需要回流 CLI 的会话改用 `--add-dir` flag + 标准 home,不走 `CODEX_HOME` 隔离。

---

## 3. 关键架构决策小结

| 维度 | 决策 | 理由 |
|---|---|---|
| 统一层 vs 原生 | **原生驱动各 CLI** | ACP 会丢 hooks(Claude 经 ACP 完全不跑)、Codex hooks 覆盖窄;原生才保全能力 |
| 物化机制 | **git worktree** | 共享对象库零拷贝、分支隔离并行安全、每 worktree 稳定唯一 cwd(利于 resume) |
| 分组单位 | **逻辑清单引用,非 submodule** | 解决"仓重叠被迫再建大仓";worktree+submodule 组合本就难受 |
| 多工具 | **异构,每方向一个工具** | 单原生会话无法跨引擎;协同改走共享文件系统 + 黑板 |
| UI | **headless 驱动 + 产品自有会话 UI;sidecar 旁路观测** | 三家都有官方结构化流;审批/排队/i18n 可做一等公民;原生 TUI 体验经终端接管保留 |
| 共享上下文 | **文件系统 + 黑板文件 + 分层 skills** | 跨异构引擎无法共享上下文窗口,只能共享磁盘产物 |
| 团队共享 | **配置下发(git / plugin marketplace)** | 无服务端、带版本与治理 |

---

## 4. 交互与编排层

### 4.0 双通道:控 + 看

每个会话切片同时开两条结构化通道,指向**同一个原生会话**,互不打架:

- **驱动通道(chat 引擎,双向)**:在 worktree 的 cwd 下 headless 驱动原生 CLI——claude 每 timeline 一个长驻 `claude -p`(stream-json 双向,stdin 收 JSON user 消息,`--include-partial-messages` 流式输出);codex / opencode 每回合一进程(`codex exec --json` / `opencode run --format json`,消息走 argv,EOF 即回合结束)。事件由 proto 解析、落 SQLite、经 Tauri 事件增量推送,前端渲染 weft 自有会话时间线。人可发消息、打断(协议 `control_request`,kill 兜底)、用斜杠命令(initialize 握手取命令清单);程序(coordinator/thread bus)的注入与人类消息**同走回合队列**实现"唤醒"。
  - **回合排队**:回合进行中收到的输入整条入队,回合结束按序送出——不丢、不混插、不抢回合。
- **观测通道(sidecar,只读,无需活进程)**:读各工具自己的会话存档,归一化为 NormEvent:
  - Claude Code → `~/.claude/projects/<编码cwd>/*.jsonl`
  - Codex → `~/.codex/sessions/<date>/` rollout jsonl(按 session_meta 的 cwd 定位)
  - OpenCode → 本地 SQLite(`~/.local/share/opencode/opencode.db`,只读打开,WAL 安全)

观测通道用于:"看 agent 干活"的 observe 视图、跨仓 git diff 聚合、黑板更新、进度/状态、轻量编排触发——即使引擎进程已死也能看。

### 4.1 OpenCode 一等公民接入(三家里最 API-first,优先级与 Claude/Codex 同等)

OpenCode 是三家里最 API-first 的(REST/SSE 齐全);当前集成走 `run --format json` 驱动 + 本地库只读观测:

- **会话**:REST(OpenAPI 3.1 + 生成 SDK)支持 list/create/get/update/delete/**fork**;`opencode run --session <id>` / `--continue` resume;`opencode run` 非交互。
- **cwd**:以 worktree 为进程工作目录逐回合精确落位(规避"切会话不换 cwd"的已知问题 #6697)。
- **控 + 看**:驱动走 `opencode run --format json`(per-turn 方言,消息走 argv);观测经 sidecar 只读其本地 SQLite(`opencode.db`,按 session.directory 定位)。`/event` SSE 与 REST 仍是可选升级面(三家里唯一真 API),当前实现不依赖常驻 `opencode serve`。
- **配置下发**:`opencode.json` 的 `plugin`(npm 包,Bun 自动装)+ `.opencode/plugins/`(提交进 git)+ **`opencode-remote-config` 插件从 git 仓同步 skills/agents/plugins**(OpenCode 版"marketplace 下发",无服务端)。
- **skills/rules 复用**:读 `~/.claude/skills`、`~/.config/opencode/skills`、`~/.agents/skills` 的 SKILL.md;读 AGENTS.md/CLAUDE.md。
- **唯一短板不影响本设计**:原生不支持"单会话多根",但在 worktree + 每仓一工具模型下每会话本就单根,用不到。

### 4.2 Thread 内通信:共享黑板 + 本地 MCP thread bus

让 thread 内不同方向/不同工具的会话互相通信,两层机制,均跨工具(三家都支持 MCP):

- **被动层(零集成,异步)**:thread 级共享文件——`PLAN.md` + 结构化 `.thread/` 状态目录。一方写、另一方下回合读;robust、tool-agnostic,但只在对方下次读/被提示时感知,非推送。
- **主动层(结构化消息总线)**:产品起一个**本机 MCP server**("thread bus")挂到该 thread 所有 session,暴露工具:`post_message(to)`/`broadcast`、`inbox()`、`ask(target,q)`(请求-响应)、`get/update_thread_state`、`announce_interface_change`(契约变更)。agent 调 MCP 工具收发,无需共享可写目录。
- **诚实约束**:agent 按回合行动,MCP 对其是"主动 poll";真正"推送/唤醒"目标需 coordinator(监听旁路通道)在消息到达时给目标**注入一个新 turn**。`bus(agent 主动)+ coordinator 唤醒(推送)= 准实时`。无法打断进行中的推理。
- **用途是协调非并发写**:thread 内各方向各拥不同仓的 worktree,不互相改文件,总线主要传契约/接口/进度/请求,天然避开写冲突。
- **人与 UI 同在总线上**:消息可在 UI 呈现,人也能往 bus 发;这是日后做自动编排的接入点。
- **本地优先**:MCP server 是本机进程,不引入服务端。

### 4.3 会话交互规范(weft 自有会话 UI + Ask Bridge)

全部会话(lead 与 worker,三家工具)跑在 headless chat 引擎上,产品渲染自己的会话时间线;不内嵌终端,原生 TUI 留给"在终端接管"逃生舱。开发按此实现。

**时间线**
- 消息归 weft 所有:持久化在 SQLite,经 `lead-chat` Tauri 事件增量推送(message / delta / finalize / turn / init / activity)。
- assistant 输出流式渲染(delta);当前执行的工具调用以 Activity 行内呈现——transient,被下一个替换、回合结束清除,不落库。
- 引擎死进程不丢会话:native id 在手,下次发送自动 `--resume` 无损续上。

**composer**
- 多行输入、`⌘↵` 发送;`@` 插入文件路径(产品文件选择器 → 路径文本,agent 用自己的工具读)。
- 粘贴图片:claude 走 inline base64 块;codex/opencode 不收 inline → 落临时文件传路径。
- `/` 唤起 slash 命令面板,命令清单来自引擎对 CLI 的 initialize 握手——原生斜杠命令照用。
- 打断:Stop 按钮 → claude 走协议 `control_request`(3s 未停 kill 兜底);per-turn 方言直接结束当前回合进程。

**审批流:Ask Bridge(结构化拦截,不刮终端)**
- 来源:三家各在自己的结构化拦截点汇入同一 weft 端点——Claude `PreToolUse` hook、Codex approval-request、OpenCode `/event`。**这是工具自身的权限习惯(用户在自己 CLI 里配的),Weft 尊重并透传,非 Weft 新增的 gate。**
- 呈现:Needs-you 卡 + 会话内审批条,从看板或会话都能答。
- 动作:`Allow` / `Always`(记住该动作)/ `Full`(该任务全放行)/ `Deny`,决定回流给被阻塞的工具。`Always`/`Full` 是 weft 侧按 (thread, task) 的内存级透传规则,非工具原生。

**注入仲裁(回合队列)** —— 程序(thread bus / coordinator)发往会话的消息与人类消息同走引擎回合队列:

```
idle ──消息到达──▶ 直接送入(开新回合)
busy ──消息到达──▶ queued(整条入队)──回合结束按序 flush──▶ busy/idle
```

- 规则:回合进行中绝不混插;队列深度对 UI 可见(queued 计数);coordinator 的 nudge 是不可见 plumbing,不占时间线行。
- 镜像各 TUI 自身语义:输入排队整条送出,不丢、不拆。

**会话状态枚举**:`starting | running | idle | exited`;审批不是会话状态——开放的 Ask 独立呈现(Needs-you / 审批条),与会话生命周期解耦。来源 = 引擎回合事件 + 进程存活;驱动头部 chip 与列表标识。

### 4.4 角色模型:lead / worker + 主 agent 为家

所有会话同一底座,靠 `role` 区分:

- **lead(主 agent)= 用户的主对话方 + 控制塔**。只读多根挂载纵览全仓 + planner/bus/dispatch MCP,**不写代码**。**入口是一个 Task**(任意粒度意图:PRD/bug/重构/spike/链接,PRD 只是一种);lead 先把 task 分类(→ Thread.type)再按 type 决定规划仪式轻重。需求讨论、总 plan、scope 分解都在它这儿;产出每个方向的 **brief**(scope + 任务 + 验收 + 文件指针),再驱动 worker、收汇总。绑定用户选的 CLI(**默认 Claude Code**,thread 可覆盖,如 bugfix 直接用 Codex 当 lead 跳过重规划)。
- **worker = 方向执行体**,可写 worktree。`workerMandate` 决定自治度:`plan+implement`(给意图+约束,自己细化再实现)或 `implement-only`(总 plan 已够细,只落码)。
- **两级规划**:lead 出"总 plan + 方向 brief"(战略),worker 可选 sub-plan(战术)。**brief 质量 = 产品天花板**,颗粒度匹配 mandate。
- **lead 上下文必须瘦**:worker 经 bus 回报**结构化摘要 + diff stat**,lead 只读这些,绝不吞 worker 原始 transcript。
- **lead 每 thread 一个、彼此独立(no shared brain, shared map)**:各 lead 有自己的上下文/黑板/bus,并行安全;但都读同一份 workspace 级 Repo Profile + 跨仓依赖图 + 看板。跨 thread 重叠由产品从各 thread scope 算出、在看板展示,不需要共享 lead。
- **Automation-first(默认自动)**:lead 默认自动出 brief、自动 spawn/派发/驱动,不等 Weft 审批。人随时可介入/改/停(opt-in),但不是必经 gate。"保守度旋钮"可调升级阈值;唯一阻塞来自工具自身权限(透传)+ 可配置的不可逆边界(见原则 10)。
- **真正的工程难点**:lead 在大仓上做跨仓 scope 分解时不能爆上下文 → 靠持久化的 **Repo Profile + 跨仓依赖图(见 4.9)** 当紧凑地图,只在需要时定向读码,而非 ingest 全部。这是"跨仓 scope 自动分解"这个 wow 的成败点,建议早做原型。

### 4.5 Surface 解耦 + Open in app

**交互界面(surface)与观测(observation)解耦**:`session.surface ∈ {chat | external-app | external-terminal}`,但无论哪种,Weft **始终经 sidecar 观测同一会话**,diff/状态/bus 实时同步——跳走不致盲。

按工具能力出逃生舱:

- **三家通用 → 在终端接管**:`chat_stop` 停下引擎 + 复制 resume 命令(`claude --resume <id>` / `codex resume <id>` / `opencode . --session <id>`),在用户自己的终端以原生 TUI 续同一会话。
- **Codex → 原生 app 深链**:`codex://threads/<nativeSessionId>`(UUID 就是我们抓的 native id,打开同一本地会话)。best-effort:契约未完全稳定、archived 会静默失败,需兜底。
- **Claude → 无会话级 app-link**,终端接管即逃生舱;另可"在 IDE 打开该 worktree"。

实现要点:跳外部(app 或终端)前**先停引擎**(同一原生会话只能一个 writer),仅保留观测;回来后向会话发消息即自动 resume 接回。

### 4.6 Agent-first 看板(可视化追踪)

借 vibe-kanban 的"任务分类可视化追踪",但 **agent-first**:看板是 agent + git 状态的**实时投影**,lead 建卡、worker 推进、**人只读和"动作",几乎不拖卡**。

**Task 是贯穿线**:输入一个 Task → 成为看板上一张卡 → 自动流转到交付。**workspace 板的卡 = 顶层 Task(= thread);thread 板的卡 = 子 Task(= direction)**;卡的移动 = Task/子 Task 在自动状态机里前进。入口(Task)→ 看板卡 → automation → Delivered,首尾相连。

- **看板可缩放两级(都要)**:
  - **Workspace 板(cards = thread)**:portfolio 视图——手里所有在飞的需求/bugfix,各自状态、涉及哪些仓、进度(如方向 2/3 已合)、跨 thread 重叠。**按仓 swimlane 时直接暴露"热点仓"**(被多个 thread 同时改 → 争用/冲突高发区),这是 Weft 独有的全局视角。thread 级列:`Planning → In progress → Needs you → In review → Delivered`。
  - **Thread 板(cards = direction/worker 任务)**:钻进一个 thread 后的执行视图(即上一节那张)。
  - 缩放链:Workspace 板 → Thread 板 → Session,镜像数据模型层级。`Needs you` 异常队列在每一级聚合(workspace 级 = 全部 thread 的阻塞;thread 级 = 本 thread 阻塞)。
- **列 = 自动推导的生命周期**:`Proposed → Queued → In progress → Needs you → In review → Delivered(PR opened)`,来自 session-state + git + 审批/bus 信号,**卡自己移动**。`Delivered` 之后(merge/CI)= 交人 + 仓库现有 harness,Weft 可观测不驱动(见原则 11)。
- **人是"动作"不是"拖动"**:`Approve / Answer / Open / Review / Merge` → 卡随之移动;手动改顺序是例外。
- **分类 = 元数据派生**:按 thread / 仓 / 工具 / 方向类型 / mandate / 状态 分组筛选;**按仓 swimlane 直接暴露跨 thread 仓重叠告警**(vibe-kanban 单仓画不出)。
- **核心原则(automation-first)**:agent 自动驱动日常流转 → 人只做异常处理 → **看板第一要务是把"卡在你这儿(Needs you)"顶到最显眼**。Needs-you **只装真正的异常**:① 工具自身弹的权限请求(透传)② agent 主动升级 ③ 硬冲突;**日常流转不进 Needs-you**(它们自动跑)。
- **板亦是 agent 的输入**:lead 读板决定下一步,coordinator 用 `Needs you` 知道去哪升级。看板 = thread bus + 会话状态 + git 的共享投影;与"主 agent 为家"并存,提供 **Board ⇄ Session** 两个视图。

### 4.7 产品化:屏蔽机制,呈现决策与结果

切法:**藏"机制",留"决策与结果"**。

- **机制 → 重隐藏 + inspect 入口**:worktree、headless 引擎进程、MCP bus、`--add-dir`/`CODEX_HOME`、jsonl/rollout 旁路。
- **决策与结果 → 一等公民**:scope(动了哪些仓)、分支/PR/diff、工具选择、有效 skills/rules、handoff。
- **不隐藏用户要负责的 git 结果**:worktree 藏,但**分支/diff/PR/merge 在交付环节可见**;inspect 入口要"真"(真路径/开终端)——power user 比小白更需要逃生舱。
- **不隐藏"哪个工具在干"**:异构是用户决策,不是 plumbing。
- **抽象三件套**:抽象 + 逃生舱 + **失败可读**(出错用产品语言说清并就地递上逃生舱)。

机制 → 产品词映射:worktree→"隔离的工作副本"(⋯里看路径/开终端);分支→只在交付时露出;headless 引擎进程→"会话";thread bus/MCP→"任务间消息/handoff";sidecar/jsonl→"活动"(调试进 Inspect 高级);add-dir/只读挂载→"这个任务可读 X";物化→点"开始"时自然发生。

### 4.8 多语言(i18n):中 / 英,两层

Weft 的 i18n 比普通 app 多一层——agent 产出的自然语言也要本地化:

- **第一层:UI 文案**。所有界面字符串外置到 locale 资源(`en` / `zh`),运行时切换,默认跟随系统 locale、可手动覆盖。用成熟框架(如 react-i18next / FormatJS)。**内部状态枚举保持英文**(`waiting-approval` 等),UI 层只做"枚举 → 本地化标签"的映射。日期/时间按 locale 格式化。第 4.7 的"产品词映射"本身也要两套。
- **第二层:agent 产出语言**。lead/worker 生成的 plan、brief、`PLAN.md` 黑板、bus 消息、commit/PR 文案,语言由一个**语言偏好**(workspace/用户级,默认跟随 UI locale,可覆盖)控制——产品把它作为一条 rule / 注入到 lead/worker 的提示里。
  - **边界**:只管自然语言;**代码、标识符、技术约定保持英文**,与语言设置无关。
  - lead 要能**接受任一语言的 PRD 输入**,产出按偏好语言。
- **不做**:RTL(中英都 LTR)。

> 注:本对话里导出的模型图 PNG 之所以用英文,是渲染沙箱缺中英全覆盖字体所致,与产品 i18n 无关——产品内中文是一等公民。

### 4.9 仓库理解:Repo Profile + 跨仓依赖图(scope 分解的燃料)

lead 要做跨仓 scope 分解,必须理解"每个仓的职责"。把它做成**持久化产物**,而不是每个 PRD 现场重推(否则爆上下文)。

**Repo Profile(每仓一份,存 Weft 本地库,可被用户改):**
- 一句话职责 / 角色(service|app|library|infra|docs)/ 技术栈 / 入口点 / 对外接口 / 依赖与被依赖 / 约定(build·test·run)/ owner。
- **来源按权威性**:用户手写一句话 > 仓内 `AGENTS.md`/`CLAUDE.md` > README / 包清单(package.json·Cargo.toml·go.mod·OpenAPI)> 目录结构推断。**人写盖过推断**。

**跨仓依赖图(真正的引擎,workspace 级):**
- 知道"`api` 的 `/cart` 被 `web-app` 消费" → 才能推出"改 api 契约 → 前端也要改"。
- MVP 廉价版:从包清单显式依赖 + 共享库引用连边;后续用 import-graph / API client / 生成 SDK 补隐式边。
- 图留在 Weft(没有单仓能装下它)。

**时序模型(什么时机识别):**
- **添加仓时(eager,主力)**:跑一次轻量 onboarding profiling——只读 README/清单/AGENTS.md/目录,**不读全量代码**,产出 Profile,给用户**确认/纠正卡**。默认开启(代价:加仓多一步 + 一次 agent 调用)。
- **增量维护**:Profile 记录构建时 commit;关键文件或仓显著推进 → 标 stale。
- **规划时 delta**:lead 用缓存 Profile,只对"与本 PRD 相关且 stale"的仓定向小补扫——**这是 scope 分解不爆上下文的关键**(平时握紧凑 Profile+图,需要时才定向读码)。
- **按需**:用户在仓的 Inspect 里手动 re-profile。

**用户中途加新仓**(专门事件):① profile 新仓;② **图 reconcile**(双向查清单,连新边,刷新争用/scope 画面);③ lead 感知,之后 PRD 自动纳入;④ 在飞 thread 不自动拉入,但检测到依赖时 lead **提示**"新仓可能与 thread X 相关"。

**canonical 归宿**:主存 Weft 本地库(快、含跨仓图);**单仓一句话职责可选写回该仓 AGENTS.md**(可移植、裸 CLI 也吃得到);跨仓图始终在 Weft。

**flywheel**:用户纠正持久化,plan 越用越准。Profile + 图 = lead 的紧凑"地图",让"跨仓 scope 自动分解"这个核心 wow 在大仓上可行而不爆上下文。

> 谁产出这张图?见 4.11 —— workspace 级的 **Curator** 角色负责语义 profiling,图的边与争用检测是确定性产品逻辑(非 agent)。

### 4.10 主 agent 运行机制:Brief 契约 + 控制环 + 依赖编排

**Brief = 结构化契约(不是传字符串)**,lead 经 planner MCP 产出、人 dispatch 前可改:

| 字段 | 说明 |
|---|---|
| objective | 该方向要达成的结果(1–3 句) |
| scope | write 仓(本 worker 拥有的 worktree)+ read 仓(上下文) |
| **interface-contract** | 要**产出/遵守**的跨仓契约(从依赖图推出)——让独立异构 worker 不漂移的握手 |
| tasks | 颗粒度按 mandate:implement-only 给精确清单;plan+impl 给目标+约束 |
| acceptance | 怎么算 done(测试/端点/类型)——驱动看板 review |
| pointers | 相关文件/符号(来自 Profile,**给指针不给全代码**) |
| deps / order | 哪些方向先完成(来自图) |
| non-goals | 明确不做什么(防 worker 跑去动 untouched 仓) |

**lead 控制环(事件驱动,输入全结构化,绝不吞 worker 原文):**
- 输入:worker 状态(session-state)+ diff stat + bus 消息(回报/提问/契约变更)+ acceptance 结果。
- lead 维护**紧凑 thread 状态**(总 plan + 各方向状态 + 未决问题 + 契约变更)落黑板,持久、可重载(resume 不必重读一切)。
- 环:`观测(结构化)→ 更新 thread 状态 → 决策(结构化动作)→(人确认)→ 执行`。触发点:worker 提问→能答则答否则升级 Needs-you;worker 广播契约变更→按图传播给下游;worker 完成→核 acceptance→进 review→判断下游解锁→提议派发;现实偏离→提议 re-plan。

**依赖编排**:契约先行(后端先发 `/cart` 契约),独立方向并行,依赖方向**挂起到上游经 bus 发布契约**。

**Automation-first(默认自动,见原则 10)**:lead 默认自动执行整条编排(分解/派发/传播契约/驱动到交付),**Weft 不插审批卡**。只在三种情况进 Needs-you:① 工具自身弹的权限请求(透传)② agent 真卡住/含糊主动升级 ③ 硬冲突。保守度旋钮可调升级阈值;不可逆动作(合并受保护分支等)按可配置边界处理。人随时可介入,但非必经 gate。

### 4.11 Workspace 级 Curator:有 agent,但角色窄(只管"地图")

**澄清一个看似矛盾点**:lead 每 thread 独立(no shared brain),但 Repo Profile/图是 workspace 级 —— 所以 workspace 层**确实有一个 agent,但它不是共享 lead,而是一个职责很窄的 Curator**。三层角色:

| role | 作用域 | 干什么 | 不干什么 |
|---|---|---|---|
| **curator** | workspace | 语义 profiling(读 README/清单/AGENTS.md → 一句话职责/接口);维护地图 | 不做需求讨论、不驱动 worker |
| **lead** | thread | 需求讨论、总 plan、scope、驱动 worker | 不跨 thread 共享 |
| **worker** | direction | 执行方向(按 mandate) | 不规划全局 |

**Curator 的关键性质:**
- **窄、背景、短命**:由事件触发(加/删仓、显著变更),read-only 短会话跑完即退(headless,只在 UI 露出"分析中…"+ 确认卡),不需要常驻 home。
- **agentic 与确定性分开**:**语义部分**(职责/接口推断)才需要 agent;**图的边**(从包清单)、**跨 thread 争用检测**(比对各 thread scope)是**确定性产品逻辑,不用 agent**。所以"workspace agent"其实只承担语义 profiling 这一小块。
- **同一底座 + 独立绑定**:Curator 也是"一个原生 CLI 会话",`role=curator`。**默认单独绑一个"快/省"模型**(profiling 简单且跑得频繁,顶配又慢又浪费),独立于 lead,可覆盖为与 lead 同一套。
- **与 lead 的关系**:Curator **建地图**,各 thread 的 lead **读地图**。职责分离 → "no shared brain, shared map" 成立:共享的是 Curator 维护的那张图,不是一个共享的需求大脑。

> 数据模型对应:`Session.role = curator | lead | worker`。

### 4.12 规划下沉 + 三层嵌套编排(不重复造,借 Dynamic Workflows 原理)

**规划不自造**:lead 是原生会话,直接用用户装的 plan skill(如 superpowers 的 brainstorming/planning/execute-plan)。Weft 只补 skill 没有的**跨仓地图(Repo Profile + 图,见 4.9)**,并把 skill 产出的 plan **结构化捕获**成 scope + 跨仓 brief(planner MCP)。skill 负责"怎么拆",Weft 负责"跨哪些仓、怎么落地执行"——互补不竞争。

**Claude Code Dynamic Workflows(DW)是什么**:Claude 现场写 JS 编排脚本,把编排计划**移出上下文窗口进代码**,runtime 后台跑、拉起子 agent(≤1000/run、并发 16),控制流确定性、`agent()` 叶子模型驱动、JSON Schema 约束 handoff。本质=**单工具(Claude)+ 进程内**的大规模子 agent 编排。

**判断:不重实现 DW(Claude-only、进程内、覆盖不到跨工具/跨仓/人在环/worktree),但借它的原理** —— `编排即代码(出上下文窗口)+ 只有叶子模型驱动 + 结构化 schema handoff`。这正强化既有原则:lead 上下文瘦、orchestration 用确定性代码。

**三层嵌套编排(各司其职):**

| 层 | 范围 | 谁来编排 |
|---|---|---|
| worker 内部 | 进程内 / 单工具 | **工具自带**(DW / subagents / agent teams);Claude worker 接大方向可自起 DW,Weft 经 sidecar 观测,白嫖 |
| 方向之间 | thread 内 / 跨仓 / 可跨工具 | **Weft 引擎**:确定性依赖 DAG 调度 + 结构化 brief/bus handoff + 人在环 gate;借 DW 原理但 tool-agnostic。lead 只做 agentic 提议,"谁先跑/谁挂起/契约怎么传/失败怎么重试"是 Weft 确定性代码 |
| thread 之间 | workspace | 看板 + Curator 地图,基本确定性逻辑 |

一句话:**Weft 坐在工具自带编排能力之上,不重造,把"编排即代码 + 结构化 handoff"用在自己跨仓跨工具的引擎里,叶子层让 Claude worker 自由用 DW。**

### 4.13 自动化下的质量闭环(没有人工关,靠"可执行验证"立信)

automation-first 的前提是:**用可执行验证替代人的审批**。看板因此是信任仪表盘而非审批队列。

1. **acceptance = 可执行契约**:brief 的验收尽量机器可判(测试/typecheck/lint/build 绿、端点可响应、schema 匹配)。
2. **worker 完成 = 检查绿,不是自报**:Weft 跑验收检查,绿才进 review/done,红进重试环。看板状态由自动检查驱动。
3. **验证阶梯(便宜→贵)**:lint/typecheck → 单测 → 集成/契约测试 →(可选)**review-agent**(沿用现成 `security-review`/`review`/superpowers 两段 review,**不自造**,Weft 只跑它读结果)= 用 agent review 默认替代 human review。
4. **跨仓契约一致性**:验"生产方产出了 interface-contract、消费方符合";不符 → 按依赖**自动回退/重派**问题侧。
5. **有界自动重试/自纠**:红 → 失败喂回 worker / 起 fix-worker → 重试,**有上限**(2–3 次)→ 仍红才升级(借 DW"迭代到收敛")。
6. **升级判据(确定性,automation 何时叫人)**:① 验收 N 次仍红 ② 契约满足不了(真设计冲突)③ 硬 git 冲突 ④ 工具弹权限(透传)⑤ PRD 没覆盖、lead 无法推断的歧义 ⑥ 预算超限(token/时间/重试)⑦ 触到不可逆边界。其余全自动流过。
7. **跑飞护栏**:每 thread/direction 预算上限 + 相同失败 loop detection + 爆炸半径边界(默认自动到 PR,merge 可配置)。
8. **看板呈现信任信号**:每卡显示 acceptance ✓/✗、测试 x/y、契约一致性、review-agent 结论 + 可展开 provenance(跑了什么/改了什么)。绿就信,红/升级才钻进去。

**诚实天花板:自动化可信度 = 仓库可验证性**。无测试则"绿"无意义 → **优雅降级**:有可执行检查就 gate 其上;没有则退到 review-agent + **卡上标低置信度**(更易升级 / 让 lead 指示 worker 补测试)。把"哪里不可信"也产品化为可见信息。

**交付边界(见原则 11)**:以上验证是 Weft 内的**轻量 pre-PR 检查**(为收敛、不开垃圾 PR);**权威 review/CI = 仓库现有 PR 触发 harness**(用户的 hooks+CI),Weft 不重造、不替代。看板终点 = Delivered(PR opened);merge/CI 交人或观测。

---

## 5. 端到端工作流(对齐你现在的做法并改进)

![Task 到交付的自动化状态机](docs/diagrams/weft-automation-flow.svg)

1. **选工作区**:选一个逻辑工作区(清单),产品按 `baseRef` 把引用的仓拉齐到位。
2. **规划(全仓只读,零物化)**:在基线视图把**所有子仓以只读挂载**(各自 baseRef,不建 worktree、不占分支)起规划会话,输入 **Task**(PRD/bug/重构…)→ lead 分类 → 产出带 scope 的 plan。
3. **scope 确认**:plan 落 `PLAN.md` 黑板,并标注每仓角色(write/read/none);人在 scope 确认步微调(见 5.1)。
4. **按方向懒物化执行**(替代你现在手动开会话):
   - 仅对 **write-target** 仓在 `ws/<workspace>/<direction>` 分支开 worktree;**read-context** 仓只读挂载;**untouched** 仓不挂。
   - 注入团队 skills + PLAN.md → 起该方向指定工具的会话(可异构);weft 会话界面实时看,sidecar 聚合。
5. **收尾**:按仓/按 worktree 分支分别看 diff → 各自 PR/合并 → 清理 worktree。
6. **回流 CLI**:每个会话本就是原生会话,用户也可在终端 `claude --resume` / `codex resume` / `opencode . --session <id>` 在**同一 worktree cwd** 接着干(产品里"在终端接管"一键复制该命令)。

### 5.1 规划期 scope:plan 决定改哪些仓(不预设、不全开)

核心:**哪些仓被改是 plan 的输出,不是人提前指定**;且通常只改一部分。

- **规划只读纵览**:规划会话只读挂载全仓,能看不能改 → 零 worktree、零污染。
- **plan = 结构化 scope**:plan 显式声明每仓 `write | read | none` + 建议方向分组与顺序。
- **物化从确认后的 scope 派生且懒加载**:只有 write 仓建 worktree,read 仓只读挂载,none 仓完全不进上下文(省 token)。
- **scope 可演进**:执行中若需动到原 none 仓,支持"按需提升为 writable"(临时建 worktree),并记一条可见的 scope 变更事件。初始范围是起点而非死框。
- **UI 对应**:规划后出现"全仓清单 + 自动勾选 write/read/none"的 scope 确认步,人改完再 create directions,此刻才批量建 worktree。

### 5.2 并行 Thread:一个 workspace 同时跑多条工作线

workspace 是**多工作线容器**(并行开发是常态)。层级:`Workspace ⊃ Thread(工作线) ⊃ Direction(方向) ⊃ Session(工具×worktree)`。

**Thread 是通用工作单元,不等于 PRD**。它有 `type`,规划仪式随类型伸缩:

| type | 规划仪式 | 典型形态 |
|---|---|---|
| feature | 完整:PRD → 只读规划 → plan+scope → 多方向 | 多仓、多 direction |
| bugfix | 轻量:跳过 plan,直接选 1~2 个仓 | 常一个 direction、甚至直接一个 session |
| refactor / spike | 介于之间,按需 | 视范围 |

> 命名注意:本产品的 **Thread = 工作线(含多个原生 session)**,与各 CLI 内部的 "session/thread" 不是一回事;Session 仍指"工具×worktree"的叶子。

并行带来的硬约束与处理:

- **分支必须含 thread 维度**:`ws/<workspace>/<thread>/<direction>`(单方向 thread 可简化为 `ws/<workspace>/<thread>`)。否则两 thread 撞同一分支 → 同一分支不能在两个 worktree 同时检出,直接报错。
- **同仓多 worktree**:一个仓被多个"thread×方向"同时写时各派生一个 worktree(共享对象库,零拷贝);产品做命名空间管理 + 控制磁盘/依赖成本(每 worktree 一份依赖 → 懒装/链接共享)。
- **黑板按 thread**:每个 thread 有自己的 `PLAN.md`/scope;团队 skills/rules 仍 workspace 级共享。
- **跨 thread 重叠感知(差异化点)**:每 thread 的 write-scope 已知 → 产品可算出"thread A、B 都写 web-app"并提前告警,展示两分支分叉、建议合并顺序、一键 rebase。
- **诚实边界**:并行改同一仓 → 最终大概率有合并冲突,这是 git 固有属性,工具只能提供**可见性 + 协调**,语义冲突仍需人解。
- **生命周期**:thread 有 `active|paused|archived`;空闲 thread 不常驻引擎进程,按需唤起(per-turn 方言本就无常驻;claude 进程死后 resume 无损),控制资源。

### 5.3 首用流(onboarding,~5 分钟到 wow)

![5 分钟首用流](docs/diagrams/weft-first-use.svg)

目标:让新用户 ~5 分钟内体验到核心 wow(跨仓 scope 自动分解)。步骤:① 新建 workspace + 加第一个仓 → ② Curator 自动 profile(确认/改一行)→ ③ 再加仓,依赖图自动成形 → ④ 新建 thread、输入一个 **Task**(PRD/bug/重构/spike)→ ⑤ **Lead 自动跨仓 scope 分解**(动哪几个仓 + 怎么分工)→ 之后 automation-first 自动拉起 worker、验证、出 PR。全程产品词、机制不可见。

---

## 6. 技术栈建议

- **外壳**:Tauri v2(Rust 后端 + Web 前端);若先做本地 Web 亦可,后续可平移。
- **会话驱动**:headless chat 引擎(Rust tokio 子进程)——claude 每 timeline 一个长驻 stream-json 进程;codex `exec --json`、opencode `run --format json` 每回合一进程;事件解析后落 SQLite,经 Tauri 事件增量推送前端。
- **方言注意**:per-turn 方言(codex/opencode)无常驻进程——打断 = 结束当前回合进程,inline 图片不支持(落临时文件传路径);三家 resume 语义都收敛在 native id 上。
- **状态库**:SQLite(工作区/会话/worktree 映射 + 会话消息)。
- **git**:直接调 `git worktree` 子命令;分支命名空间化避免冲突。
- **配置下发**:复用 Claude Code `/plugin marketplace add` 与 Codex `codex marketplace add`(均 git-backed),团队基线打成 plugin/skill 包。

---

## 7. 可行性评估(已核验)

总体结论:**绿灯为主**。所有承重组件都已有官方机制或现成开源产品验证;真正的工程难点不在"能不能",而在"把它们组合成跨仓 + 跨工具 + 分层下发的体验"——而这恰恰是现有产品的空白(见第 9 节)。

| 假设 | 评级 | 依据 | 风险/缓解 |
|---|---|---|---|
| headless 结构化流驱动三家 CLI | 🟢 已验证(本仓已落地) | claude `-p` stream-json 双向 + `control_request` 打断(实测);codex `exec --json`;opencode `run --format json`——chat 引擎(lead_chat)已跑通三家 | 方言差异(常驻 vs 每回合、inline 图片、打断语义)收敛在引擎两分支;流 schema 漂移做版本兼容 |
| worktree 做物化 + 并行隔离 | 🟢 已验证 | Conductor、Nimbalyst(原 Crystal)、vibe-kanban、Claude Squad 等一整个品类都用"每会话一个 worktree" | 同分支不能双检出 → 分支命名空间化;每 worktree 依赖各一份 → 懒装/共享 |
| 异构工具(Claude+Codex+OpenCode 并行) | 🟢 已验证 | Conductor 支持 Claude+Codex 并行;Nimbalyst 异构 agent;OpenCode 由本产品经其 REST/SSE 直接驱动,不依赖第三方编排器支持 | 三家可同批支持;各自鉴权独立(各自 `/login`) |
| 创建/resume 回流 CLI | 🟢 已验证 | 三家会话都落本地标准存储,SDK/CLI 共享同一份;`claude --resume`、`codex resume`、opencode `/resume` 均可 | cwd 必须一致(worktree 天然稳定);Codex 自定义 `CODEX_HOME` + resume 有已知 bug(#5247)→ 要回流 CLI 的会话别做 CODEX_HOME 隔离,改用标准路径 |
| 旁路结构化通道(看的同时拿数据) | 🟢 已验证 | Claude jsonl / Codex rollout 为 append-only 实时写;OpenCode 有 `/event` SSE 真 API | jsonl/rollout schema 各异且可能随版本变 → 做归一化层 + 版本兼容 |
| 团队共享 = 配置下发(无服务端) | 🟢 已验证 | Claude Code plugin marketplace(git-backed,`/plugin marketplace add org/repo`,私有仓鉴权沿用 git host);Codex `codex marketplace add github:org/repo` + `requirements.toml` 策略 + dotfiles 下发 | Codex 官方自助发布"coming soon",但 git-backed marketplace 当下可用 |
| skills 跨工具复用 | 🟢 基本可行 | SKILL.md 已是事实标准;OpenCode 直接读 `~/.claude/skills`;有跨工具 skill 合集(如 alirezarezvani/claude-skills 覆盖多工具) | 个别能力字段不通用 → 以 SKILL.md 公共子集为主 |
| 单会话内多仓(同一工具) | 🟡 部分(本设计不依赖) | Claude `additionalDirectories`/`--add-dir`、Codex `--add-dir`/`writable_roots` 支持;OpenCode 原生不支持(#19515) | worktree+每仓一工具模型下每会话本就单根,此项非关键路径;需要时仅 Claude/Codex 用 |
| 配置下发(OpenCode) | 🟢 已验证 | `opencode.json` 的 `plugin`(npm)+ `.opencode/plugins/`(git)+ `opencode-remote-config`(git 同步 skills/agents/plugins) | 形式与 Claude/Codex 的 marketplace 不同但等效,均无服务端 |
| 单会话内**多工具** | 🔴 不做 | 原生不存在;需复合编排,代价大 | 已决策放弃,改异构多会话 + 共享文件系统 |
| 全自动跨工具编排(coordinator 自动调度) | 🟡 进阶 | 现有产品多为"human-in-the-loop 并行派发"(Conductor 即如此) | MVP 先做人主导;自动编排留作后期 |

---

## 8. 风险与缓解(汇总)

- **长回合无输出/跑飞**:watchdog 时钟对 busy 回合计时(墙钟 + 最后活动),超限按 §4.13 护栏处置(打断/升级)。
- **审批拦截点契约漂移**:Ask Bridge 依赖三家各自的结构化拦截点(PreToolUse hook / approval-request / `/event`),随版本可能变 → 版本探测 + 兜底是"在终端接管"原生作答。
- **上下文天花板**:多仓塞一会话会膨胀 → 按会话勾选激活仓,而非永远全挂。
- **worktree 运维成本**:依赖/构建每 worktree 一份 → 懒装/链接共享;提供一键创建/合并/清理。
- **别再用 submodule 当分组单位**:worktree+submodule 难受;改清单引用 + 记录 ref 保留可复现。
- **Codex CODEX_HOME + resume bug(#5247)**:要回流 CLI 的会话用标准路径,不做 home 隔离。
- **schema 漂移**:各工具日志格式随版本变 → 归一化层 + 版本探测 + 优雅降级。
- **无服务端的边界**:跨人实时协作/团队看板天然不在范围;只能靠异步下发/提交回团队仓。别后期又要"实时看队友",会推翻前提。

---

## 9. 开源先例对照(= 你的差异化)

| 产品 | 做了什么 | 缺口(你的机会) |
|---|---|---|
| Conductor | 多 agent(Claude+Codex)并行,每会话一个 worktree,diff-first 评审 | **单仓为主**;无跨仓工作区、无分层 skills 下发 |
| Nimbalyst(原 Crystal) | 跨平台桌面、worktree 会话看板、异构 agent | 同上,单仓中心 |
| vibe-kanban | 开源 worktree 看板 | 同上 |
| Codeman | tmux + WebUI 管 Claude Code/OpenCode,PTY 合帧管线 | 无工作区/worktree 编排、无下发 |
| CloudCLI / Claude Code UI | 多 CLI(CC/Codex/OpenCode)的 Web/移动端会话管理 | 偏远程会话管理,无多仓工作区 + worktree + 分层下发 |
| opcode | Claude Code 单工具 GUI(会话/agent/MCP/checkpoint) | 单工具、单仓 |

**结论**:每块积木都被验证过,但"**逻辑多仓工作区 + worktree 物化 + 异构工具 + 分层 skills 下发 + 产品自有会话 UI(headless 驱动)**"这个组合,目前**无人做全**。可行性 OK,差异化清晰。

---

## 10. MVP 范围与里程碑

**MVP(验证四件事:多根/worktree、配置物化、会话注册表、回流 CLI)**

最小闭环:
1. 定义一个工作区:伞形根 + 引用 2 个仓 + 一条团队 rule + 一个 team skill。
2. 选 1~2 个执行方向 → 按方向开 worktree、注入 skills + `PLAN.md`。
3. 各方向起一个原生会话,**三家(Claude Code / Codex / OpenCode)同为第一批**异构打通,weft 会话界面实时显示。
4. 旁路通道按仓展示 diff。
5. 关闭后能在终端 `claude --resume` / `codex resume` 在同一 worktree cwd 续上。

**里程碑建议**
- M1:chat 引擎跑通单个工具(claude stream-json),能创建/resume。
- M2:worktree 编排(按方向批量建/清理)+ 工作区清单数据模型。
- M3:配置物化(team skills/rules + 黑板注入)+ 有效配置预览。
- M4:旁路通道(jsonl/rollout/SSE 归一化)+ 按仓 diff 聚合。
- M5:配置下发(Claude `/plugin marketplace` + Codex `codex marketplace` + OpenCode `opencode-remote-config`/npm plugin)+ 个人/团队分层。
- M6:多方向并行 + 统一时间线;(可选)轻量 coordinator handoff。

---

## 附:已确认的关键事实速查

- Claude 经 ACP **不跑 hooks**(Zed 官方明确);→ 故选原生驱动。
- Claude 会话:`~/.claude/projects/<编码cwd>/*.jsonl`;`--add-dir`/`additionalDirectories` 支持多根;`--resume <id>` 续(强依赖 cwd)。
- Codex 会话:`~/.codex/sessions/` rollout;`--add-dir`/`writable_roots` 多根;`codex resume`;hooks 实验性且仅 Bash 拦截;`CODEX_HOME`+resume 有 bug(#5247)。
- OpenCode:REST 会话 API(create/get/fork/delete)+ SDK;`opencode run --session/--continue` resume;`--dir` 指定 cwd;`/event` SSE;TUI 即 server 客户端可 attach;下发用 `plugin`(npm)+ `.opencode/plugins/` + `opencode-remote-config`(git 同步);多根原生不支持(#19515,本设计不依赖)。
- 配置下发:Claude `/plugin marketplace add org/repo`、Codex `codex marketplace add github:org/repo`,均 git-backed,无需服务端。
