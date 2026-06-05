# M1 垂直切片 — 设计文档

> 项目:weft —— 多仓 · 多工具 · 会话编排器(本地优先、无服务端)。
> 完整架构见 `ARCHITECTURE.md`(源:`多仓多工具会话编排器-架构设计与可行性.md`);开工约束见 `CLAUDE.md`(源:`CLAUDE-CODE-启动包.md`)。
> 本文档只覆盖 **M1 垂直切片**。架构与技术栈已锁定,本文档不重新讨论它们。

## 状态

- 日期:2026-06-05
- 范围:M1(单工具端到端)
- 起点决策:**方案 C** —— 先做无 UI 的 Rust 风险验证 spike,再包最小 Tauri UI。
- 已锁定技术栈(不更换):Tauri v2 + React/TS/Vite + xterm.js + portable-pty + (M2 起)SQLite + 系统 git。

## 环境(已核验)

- Node 24.15 / npm 11.12;Rust 1.95 / cargo;git 2.50。
- 三家 CLI 均已安装:`claude 2.1.158`、`codex`、`opencode`。M1 只用 Claude。
- Tauri CLI 未安装,脚手架阶段补(`cargo install tauri-cli` 或 `npm create tauri-app`)。
- 结论:M1 可用真实 `claude` 二进制跑集成冒烟,无需 mock。

## 目标(M1 验证什么)

整个产品的物化层押在一个假设上:**`claude --resume` 能在 worktree 的 cwd 里,用我们抓到的 native session id,接回历史并继续**。M1 先把这条最高风险链路验证掉,再立 UI。

## ✅ Step 1 实测结论(2026-06-05,已跑通)

用真实 `claude 2.1.158` + `portable-pty 0.8.1` 跑通的 spike(`crates/spike-pty`),三条断言全 PASS:

- ① demo 仓 worktree 里 `hello.txt` 被交互式会话创建。
- ② session id 捕获成功且交叉校验通过(jsonl 文件名 stem == 文件内 `sessionId` 字段)。
- ③ `--resume <id>` 在同一 cwd 复用**同一** jsonl(文件数 1→1,行数 12→37,被续写而非新建),且 resume 后 TUI 正确答出此前创建的文件名 → 历史确实加载。

**实测修正/新增发现(覆盖原假设):**

1. **encoded-cwd 必须先 canonicalize**:Claude 用解析符号链接后的真实路径编码。macOS `/tmp` → `/private/tmp`,故 `/tmp/.../wt` 的会话目录是 `-private-tmp-...`。编码规则 = canonical 路径里 `/` 和 `.` 均替换为 `-`。
2. **首次 `--dangerously-skip-permissions` 有 Bypass 确认屏**:`1. No, exit` / `2. Yes, I accept`,Enter 确认。程序化 spawn 必须跨过它(spike 发 `2`+Enter)。**产品级影响**:spawn claude 会撞 onboarding/trust/bypass 首屏,产品要么驱动这些按键,要么在 **worktree-local、gitignore 的** 配置里预置 accepted 标志(绝不写 canonical 仓)。这条进 M4/M5 的会话启动逻辑。

## 权限模式抉择:**产品默认不用 `--dangerously-skip-permissions`**

用 `crates/spike-pty/src/bin/probe.rs` 实测(普通权限模式,不加 flag),确认产品该走"保留权限、自己呈现审批"的路线:

- **① 文件夹信任屏**(每个新 worktree 路径一次):`❯ 1. Yes, I trust this folder / 2. No, exit`。先于一切;与工具权限无关。
- **② 工具权限请求屏**(每个动作):`Do you want to create hi.txt? ❯ 1. Yes / 2. Yes, allow all edits during this session / 3. No`。**这正是架构 §4.3 审批流要消费的 `ApprovalRequested` 事件**;`Always` ≈ TUI 的 "2. allow all this session"。

`--dangerously-skip-permissions` 只是 spike 为无人值守拿确定性结果才用;产品保留权限请求并用自身审批 UI 呈现,才符合 §4.3。

**会话启动纪律(M4 会话启动逻辑必须遵守):门屏先答,再喂 prompt。** 探针证实:prompt 早于信任门发送会被信任门吞掉。健壮实现应**靠旁路通道/输出检测屏状态来驱动过门**,而非固定 sleep(spike/probe 里的固定 sleep 已显脆弱)。

## 范围

**In**

- 在一个 demo git 仓的 worktree(分支 `ws/demo/t1/main`)里,用 Rust `portable-pty` spawn 交互式 `claude`。
- xterm.js 双向渲染(打字进 stdin、输出回显)。
- 抓取 native session id。
- Ctrl-C 透传。
- 关闭进程后 `claude --resume <id>` 在**同一 cwd** 接回历史。

**Out(M1 不做,留给 M2+)**

- SQLite 持久化与 Workspace/Thread/Direction/Session 数据模型(M2)。
- 多 driver(Codex / OpenCode)与旁路事件归一化(M3)。
- 完整键位归属表、审批快捷条、注入仲裁、composer(M4)。
- scope 懒物化、配置物化/下发(M5)。
- thread bus、coordinator(M6)。

M1 的 worktree/session 信息只放内存,不持久化。

## 两步走

### Step 1 — 风险验证 spike(纯 Rust,无 UI)

一个 Rust 集成测试 + 一个可手动跑的小 bin,执行链路:

1. 建一次性 demo 仓(test fixture:`git init` + 一个文件 + 一次 commit)。
2. `git worktree add` 出 `ws/demo/t1/main` 分支的 worktree。
3. portable-pty 中 spawn `claude`,cwd = 该 worktree。
4. 往 stdin 写一个 prompt(如"创建 hello.txt 写入 hi"),等 worktree 里出现该文件。
5. **抓 session id**:监视 `~/.claude/projects/<encoded-cwd>/`,取最新 `*.jsonl`,文件名 stem 即 session id;并用 jsonl 首行 JSON 的 `sessionId` 字段交叉校验,二者一致才算抓到。
6. kill 进程。
7. `claude --resume <id>`,cwd 不变,断言同一 session jsonl 被续写(id 复用)、历史在。

**验收**

- 自动断言:① demo 仓 worktree 里目标文件被创建/改动;② 抓到非空 session id 且文件名 stem 与 jsonl 内 `sessionId` 一致;③ resume 后复用同一 jsonl(id 不变、文件被续写)。
- 视觉/TUI 部分可手动眼检(对齐启动包 §6"集成冒烟可手动")。

### Step 2 — 最小 Tauri 应用

在已证明的地基上套 UI:单窗口、单 xterm.js 面板、底部 prompt 输入框 + Resume 按钮。

**验收(启动包 M1 三条)**

- 能在面板里打字让 claude 改文件。
- 能 Ctrl-C。
- 关闭进程后点 Resume(走 `--resume`)在同一 cwd 接回历史并继续;worktree 内能看到改动。

## 承重细节:session-id 捕获与 resume

- **encoded-cwd 规则**:Claude 将 cwd 的 `/` 编码为 `-`(如 `/Users/x/wt` → `-Users-x-wt`)。**spike 第一件事就是实测确认这条规则,不假定**;若实测不符,以实测为准并回写本设计。
- **id 来源**:目录内最新 jsonl 的文件名 stem,用首行 JSON 的 `sessionId` 字段交叉校验;一致才算捕获成功。
- **cwd 稳定性**:worktree 路径一旦建好绝不变更(`--resume` 强依赖 cwd 编码一致,启动包 §7 已标红风险)。

## PTY 双向 + 合帧管线

- **双向**:xterm.js `onData` → Tauri command `write_pty` → pty stdin;pty 输出 → Tauri event `pty_output` → xterm。
- **合帧**:Claude Code 基于 Ink 高频重绘,直转会闪。Rust 侧把 pty 输出按 ~16ms 批量聚合后再 emit,前端 rAF 喂 xterm。M1 即需此基础管线(启动包列为 M1 必需)。
- **键位**:M1 只做 Ctrl-C 透传 + 普通字符透传;完整键位归属(截留 ⌘ 前缀)留 M4。

## 项目脚手架(M1 只立用得到的部分)

```
weft/
  src-tauri/          Rust 后端
    src/pty/          portable-pty 会话管理
    src/git/          worktree 建/删
    src/drivers/      ToolDriver trait + claude.rs(M1 只实现 Claude)
    tests/            spike 集成测试
  src/                React + TS + Vite
    panels/           xterm 面板
  ARCHITECTURE.md     架构文档
  CLAUDE.md           启动包约束
  docs/superpowers/specs/   设计文档
```

`ToolDriver` trait 在 M1 即按启动包 §4 的形状定义(`spawn` / `capture_native_session_id` / `sidecar`),但只落地 `ClaudeDriver`,为 M3 留扩展点。

```rust
struct SpawnSpec {
  cwd: PathBuf,                 // = 某 worktree
  read_dirs: Vec<PathBuf>,      // M1 恒为空;只读挂载留 M5
  resume_id: Option<String>,    // None=新建; Some=resume
  mcp_servers: Vec<McpRef>,     // M1 恒为空;thread bus 留 M6
}

trait ToolDriver {
  fn id(&self) -> Tool;
  fn spawn(&self, s: &SpawnSpec) -> Result<PtyHandle>;
  fn capture_native_session_id(&self, h: &PtyHandle) -> Option<String>;
  fn sidecar(&self, s: &SpawnSpec) -> Box<dyn EventStream>;  // M1 可返回空流
}
```

## 测试

- **单元**:encoded-cwd 编码函数;session-id 解析(喂样例 jsonl);合帧批处理逻辑。
- **集成**:Step 1 的 spawn→capture→kill→resume 真二进制冒烟。
- **手动验收**:启动包 M1 三条逐条过。

## 关键回归点(全程守住)

- Claude `--resume` 的 cwd 编码一致性 → worktree 路径必须稳定。
- 跨仓接线只用临时启动参数,绝不写进 canonical 仓(M1 无接线,但脚手架与习惯从一开始就守)。

## 完成定义(M1 Done)

- Step 1 集成测试在 CI/本地用真实 `claude` 跑通三条自动断言。
- Step 2 最小 Tauri 应用三条手动验收逐条通过。
- 单元测试覆盖 encoded-cwd、session-id 解析、合帧逻辑。
- 改动说明随交付。
