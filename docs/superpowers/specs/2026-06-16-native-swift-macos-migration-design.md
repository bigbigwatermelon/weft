# Native Swift macOS Migration Design

## Goal

Migrate Atlas from a Tauri v2 + React frontend to a native macOS application written in Swift, while preserving the existing Rust core for the first production migration phase.

The first version must be a real working product path, not a static shell. It must launch and drive existing Claude, Codex, and OpenCode agent sessions through the existing Atlas runtime, while all visible app UI is implemented with native macOS 26+ SwiftUI/AppKit components and the system Liquid Glass design language.

## Confirmed Decisions

- Architecture path: native Swift macOS UI plus existing Rust core.
- First migration scope: full native shell plus core workflow MVP.
- Minimum OS target: macOS 26+ with the latest Xcode and SwiftUI/AppKit APIs.
- Runtime requirement: the MVP must really start and drive agent sessions.
- Visual process: no visual companion or browser mockups for this brainstorming pass.
- Implementation gate: this document is design only. No Swift project scaffolding or product code migration starts until this spec is reviewed and an implementation plan is approved.

## Non-Goals

- Do not rewrite the Rust runtime into Swift in the first phase.
- Do not implement a fake-data prototype as the primary deliverable.
- Do not preserve the React/Tauri UI as the new user-facing shell.
- Do not recreate Tailwind, Radix, or lucide components in Swift.
- Do not support macOS 14 or macOS 15 in the first native target.
- Do not move every secondary surface in the first phase, including full diff review, repo graph editing, backup restore flows, IM administration, and full skills management.

## Current System Summary

The current app is a Tauri v2 desktop app:

- React and TypeScript render the workspace board, issue board, lead chat, worker session chat, observe/diff views, command palette, settings, Needs-you queue, and permission prompts.
- Rust owns SQLite and SeaORM storage, migrations, workspaces, issues, runs, worktrees, git operations, agent process orchestration, lead and worker chat engines, Ask Bridge, local MCP/thread bus, Feishu/Lark bridge, backup, sidecar observation, and verification commands.
- The frontend talks to Rust through many Tauri `invoke` commands and receives streaming updates through Tauri events, especially the `lead-chat` stream.

The migration risk is not mainly visual. The hard part is replacing the Tauri command/event boundary and rebuilding the frontend state model in Swift without regressing agent orchestration, permission handling, session streaming, and local data safety.

## Recommended Approach

Use a native Swift app that launches and talks to a bundled Rust core server.

Rejected alternatives:

- Rust as a Swift-callable static library or dylib: attractive on paper, but async streams, Tokio lifecycle, callbacks, child processes, and crash isolation are harder in the first phase.
- Full Swift rewrite of the backend: too much regression risk because it would rewrite store, git, MCP, backup, IM, agent adapters, and process control at the same time as the UI migration.
- Tauri WebView with a Liquid Glass-like theme: does not satisfy the requirement that all interface and frontend UI components use native macOS components.

## Target Architecture

The migration introduces two explicit layers.

### AtlasNative

`AtlasNative` is the new macOS 26+ app target written in Swift. It owns:

- Window and scene lifecycle.
- Navigation and native app commands.
- SwiftUI/AppKit views.
- Native dialogs, sheets, pickers, toolbar items, menus, settings, notifications, and accessibility behavior.
- A Swift `AppStore` and typed view models.
- A `CoreClient` that talks to the Rust core API.

Swift views do not directly read SQLite, spawn agent CLIs, run git commands, or parse agent transcripts.

### atlas-core

`atlas-core` is extracted from the existing Rust backend. It continues to own:

- SQLite database and migrations.
- Workspace, repo, issue, run, session, ask, and need state.
- Agent process orchestration for Claude, Codex, and OpenCode.
- Lead and worker chat streaming.
- Permission Ask Bridge and human-question handling.
- Local MCP/thread bus.
- Git worktree and verification operations.
- Backup, restore, power guard, skill sync, sidecar observation, and IM bridge where already implemented.

The initial Swift app talks to `atlas-core` through a local server process instead of Tauri commands.

## Core API Boundary

The first implementation should extract a typed `core-api` contract from the current TypeScript/Rust command surface.

### Commands

The MVP command set includes:

- `health` and `version`
- workspace list/create/rename/select
- repo list and basic register/create/clone where needed by the MVP
- issue list/create/rename/delete
- run list/create/open/revive/status
- lead send/ensure/interrupt/stop/state/list messages
- worker open/send/interrupt/stop/state
- pending asks/list/answer
- needs list/answer/go-to target
- write trigger list/approve/deny
- default tool/detect tools/set tool
- dangerous mode, guardrails, notifications, keep awake, projects directory

Commands should use JSON DTOs that preserve the current product model names:

- `Workspace`
- `RepoRef`
- `Issue`, backed by the current Rust `thread` entity and `thread_id` fields where needed for compatibility
- `Run`, backed by the current Rust `direction` entity and `direction_id` fields where needed for compatibility
- `Worktree`
- `SessionInfo`
- `LeadMessage`
- `LeadChatPush`
- `NeedItem`
- `PermissionAsk`
- `WriteTrigger`
- `ToolStatus`

The API must carry a schema version so Swift can fail early if the bundled core binary and app build are mismatched.

### Event Stream

The MVP event stream includes:

- chat message creation
- streaming text delta
- message finalize
- turn state changes
- activity updates
- session init and native session id
- session status updates
- needs/asks/write-trigger refresh notifications
- workspace/issue/run state changes
- core shutdown or fatal errors

The existing `lead-chat` Tauri event should become a core-owned event sink abstraction. Tauri can remain as a temporary adapter, but the new Rust core must emit through an interface that the core server and any legacy Tauri wrapper can both implement.

### Transport

Use a bundled local core server for phase one.

Recommended transport:

- Swift launches `atlas-core-server` from the app bundle.
- Rust binds either a Unix domain socket or a random localhost port.
- Rust prints a startup handshake containing endpoint, process id, API version, and a per-launch token.
- Swift connects with the token and starts the event stream.
- Swift sends graceful shutdown on app quit and verifies that the core process exits.

The token is not a security boundary against a compromised local user, but it prevents accidental calls from unrelated local processes.

## Native UI MVP

The native app should rebuild information architecture around macOS patterns rather than directly translating the React component tree.

### App Shell

Use `NavigationSplitView` as the main shell:

- Sidebar: workspace switcher, Needs-you entry, board/issues entry, repositories entry, settings entry, issue list.
- Content: selected workspace board, issue/run list, or settings surface.
- Detail: lead or worker session, selected run details, or Needs/Ask resolution.

Use native toolbar items for new issue, search/command, Needs, settings, and session actions. Do not recreate the current custom top bar as a visual port.

### Core Workflow Surfaces

First phase surfaces:

- Workspace list and switching.
- Issue list and creation.
- Run list and creation.
- Lead chat timeline and composer.
- Worker chat timeline and composer.
- Session status, activity, stop, interrupt, and resume/takeover affordances.
- Needs-you list.
- Permission ask sheet/popover with Allow, Always, Full, and Deny.
- Basic settings needed for agent execution.

Second phase surfaces:

- Full diff panel and review tooling.
- Rich Kanban board parity.
- Repo graph editing.
- Full backup and restore UI.
- Full skills source management.
- IM bridge administration.
- Full command palette parity.

### Liquid Glass and Native Components

The UI must use system components first:

- `NavigationSplitView` for app structure.
- `List`, `Table`, segmented controls, menus, search fields, sheets, popovers, confirmation dialogs, forms, and toolbar controls where they fit.
- SF Symbols instead of lucide icons.
- Native pickers for folders, files, and attachments.
- Native menu bar commands and keyboard shortcuts.

Custom Liquid Glass treatment is only for custom interface pieces that are not already covered by system components, such as compact status capsules, floating session controls, or grouped toolbar adjuncts.

Use Apple guidance as the baseline:

- Standard SwiftUI, UIKit, and AppKit controls and navigation elements adopt Liquid Glass appearance automatically on supported systems.
- Custom views can use `glassEffect(_:in:)` and related APIs when a custom surface must participate in the material system.
- The app must respect accessibility settings such as Reduce Transparency and increased contrast.

Apple references:

- Liquid Glass overview: https://developer.apple.com/documentation/technologyoverviews/liquid-glass
- Adopting Liquid Glass: https://developer.apple.com/documentation/TechnologyOverviews/adopting-liquid-glass
- Applying Liquid Glass to custom views: https://developer.apple.com/documentation/SwiftUI/Applying-Liquid-Glass-to-custom-views
- Human Interface Guidelines materials: https://developer.apple.com/design/human-interface-guidelines/materials

## Swift State Model

Create a Swift `AppStore` that mirrors the current React store responsibilities without copying its implementation shape.

Responsibilities:

- Hold workspace, issue, run, session, need, ask, tool, and settings state.
- Own selected workspace, selected issue, selected session, and visible route.
- Subscribe to core events and apply typed state updates.
- Provide intent methods such as `createIssue`, `openRun`, `sendLeadMessage`, `answerPermission`, and `interruptSession`.

Views should bind to focused view models so state aggregation does not leak across every Swift view.

## Error Handling

The native app must fail explicitly and recover where possible:

- Core process fails to start: show a blocking startup error with log path and retry.
- API schema mismatch: stop and explain that app and bundled core versions do not match.
- Event stream disconnects: mark the app offline, attempt one controlled reconnect, and require manual retry after repeated failure.
- Agent process error: surface in the session timeline and preserve the transcript.
- Permission ask failure: keep the ask visible until the core confirms an answer.
- Core shutdown while sessions run: warn before quit and attempt graceful stop.

No silent fallback to the legacy React/Tauri UI is allowed inside the native app.

## Testing And Verification

### Automated Tests

Required for the MVP:

- Rust core unit and integration tests for extracted command handlers.
- Core API contract tests that validate command and event DTOs.
- Swift unit tests for `CoreClient`, event decoding, and state reducer behavior.
- Swift UI smoke tests for navigation, chat composer, asks, and settings if the project test harness supports them.
- Process lifecycle tests for launching and stopping the bundled core server.

### Manual Product Path

The final MVP must be verified on macOS 26+ with the latest build:

1. Launch AtlasNative.
2. Start or connect to the bundled Rust core.
3. Confirm health/version.
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

## Migration Phases

### Phase 0: API And Core Extraction

- Inventory current Tauri commands and events.
- Extract core business functions away from Tauri wrappers.
- Add core event sink abstraction.
- Build `atlas-core-server` with health/version, selected commands, and event streaming.
- Keep the current Tauri app working while extraction happens.

### Phase 1: Native Shell And Core Workflow

- Add the Swift macOS project.
- Bundle and launch `atlas-core-server`.
- Implement `CoreClient`.
- Implement workspace shell, issue list, run list, lead chat, worker chat, Needs/Ask, and execution settings.
- Verify real agent session flow.

### Phase 2: Parity Expansion

- Bring over diff/review surfaces.
- Bring over repo map and richer board interactions.
- Bring over skills, backup, IM, command palette, and update flows.
- Harden keyboard shortcuts, menu bar behavior, accessibility, and multi-window policy.

### Phase 3: Tauri Retirement

- Freeze React/Tauri development after native parity is sufficient.
- Remove Tauri packaging and React frontend from the default app build.
- Keep Rust core as the shared runtime unless a later, separate spec approves rewriting backend subsystems in Swift.

## Completion Definition

The first migration milestone is complete only when:

- `AtlasNative` launches as a native macOS 26+ app.
- All visible MVP UI surfaces are SwiftUI/AppKit native components.
- Liquid Glass is provided by system components or appropriate SwiftUI custom view APIs.
- The native app starts and communicates with the bundled Rust core.
- The app can create/select workspace, create issue/run, start real agent session, send chat, receive streaming events, handle Needs/Ask, interrupt/stop, quit, and rehydrate.
- Rust core tests pass.
- Swift tests pass.
- Core API contract tests pass.
- Manual end-to-end product path is recorded.
- No first-phase requirement depends on the old React/Tauri UI.
