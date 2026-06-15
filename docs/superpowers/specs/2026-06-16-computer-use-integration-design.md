# Computer Use Integration Design

Date: 2026-06-16
Status: Approved design draft

## Summary

Atlas will integrate `open-codex-computer-use` as a bundled macOS helper binary. When the user enables Computer Use in Settings, all newly started agent sessions receive an `open_computer_use` stdio MCP server that points at the bundled `open-computer-use mcp` command.

The first version is intentionally direct and trusted:

- macOS only.
- Bundle a fixed `open-computer-use` binary with the Atlas app.
- Global Settings toggle enables Computer Use for new sessions.
- No per-app first-use confirmation.
- No Atlas-level high-risk action interception.
- No Atlas proxy for Computer Use tool calls.
- Each agent session starts its own stdio sidecar through its native MCP client.
- Settings shows enablement, helper status, helper version, and permission diagnostics.

This gives users a first-class product capability while keeping implementation risk low. Atlas owns packaging, configuration, diagnostics, and MCP injection; `open-computer-use` owns macOS Accessibility, Screen Recording, screenshots, input simulation, and MCP tool dispatch.

## Context

Atlas already has the right runtime surfaces for this integration:

- A Rust/Tauri backend that starts and drives agent sessions.
- Existing per-tool MCP injection for Claude, Codex, and OpenCode.
- Settings screens and persisted app settings.
- Session timeline and transcript rendering for native agent events.
- Existing Ask Bridge and permission surfaces, though they are not part of this first Computer Use path.

The selected upstream reference is `iFurySt/open-codex-computer-use`. It provides a Swift `OpenComputerUseKit` library and an `OpenComputerUse` executable with `mcp`, `doctor`, `call`, and version/diagnostic commands. The integration should not call upstream installer commands that modify `~/.codex/config.toml`, `~/.claude.json`, or global OpenCode config.

## Goals

1. Make Computer Use feel built into Atlas for macOS users.
2. Let all newly started agent sessions use Computer Use when the global setting is enabled.
3. Avoid requiring users to install `open-computer-use` globally.
4. Keep the first version small by using stdio MCP direct connection instead of an Atlas proxy.
5. Keep all Computer Use integration points centralized so a future proxy architecture can replace direct injection without changing Settings or product vocabulary.
6. Provide enough diagnostics for users to understand missing helper binaries and macOS permission issues.

## Non-Goals

- No Windows or Linux support in the first version.
- No per-app allowlist or first-use approval.
- No Atlas-level action audit log for every click, key press, type, scroll, or drag.
- No Atlas proxy MCP server in the first version.
- No direct vendoring of the upstream Swift source into the Rust process.
- No online helper download on first use.
- No modification of user global Codex, Claude, Gemini, or OpenCode config files.
- No hot insertion into already-running sessions.

## User-Facing Behavior

Settings gains a Computer Use section:

- `Enable Computer Use for new sessions`
- Helper status: found, missing, not executable, or unknown
- Helper version: parsed from helper output when available
- macOS permissions: Accessibility and Screen Recording status when `doctor` can report them
- Actions: Recheck, Run doctor, and Open System Settings when available

When the setting is enabled, Atlas tells the user that new sessions can operate local GUI apps directly and that Atlas will not ask before each app or action. This is informational, not a blocking confirmation.

The setting only affects new sessions. If the user disables Computer Use while sessions are running, Atlas does not forcibly stop sidecar processes that were already started by agent MCP clients.

## Architecture

The integration is a direct stdio MCP path:

```text
Atlas Settings
  -> computer_use.enabled = true

New Agent Session
  -> Atlas builds normal session injection
  -> Atlas adds open_computer_use MCP server config
  -> agent MCP client starts bundled open-computer-use mcp
  -> agent calls Computer Use tools directly
  -> sidecar controls macOS through Accessibility / Screen Recording
```

Atlas does not sit in the tool-call data path.

```text
src-tauri/src/computer_use/
  mod.rs
  settings.rs
  helper.rs
  diagnostics.rs
  inject.rs
```

Suggested responsibilities:

- `settings`: load/save `computer_use_enabled`.
- `helper`: resolve bundled helper path for dev and packaged app modes.
- `diagnostics`: run `doctor` and version commands with timeouts.
- `inject`: build MCP injection snippets for Claude, Codex, and OpenCode.

The existing agent startup code should call a single Computer Use injection helper rather than duplicating helper-path and config details in each adapter.

## Session Injection

Computer Use is injected only into new sessions, and only when all of these are true:

- Settings has `computer_use_enabled = true`.
- The current platform is macOS.
- A bundled helper path can be resolved.
- The helper exists and is executable.

If these checks fail, Atlas starts the agent session normally without Computer Use and records a diagnostic message for Settings/log output.

### Claude

Atlas writes a worktree-local, ignored MCP config file:

```json
{
  "mcpServers": {
    "open_computer_use": {
      "command": "/path/to/open-computer-use",
      "args": ["mcp"]
    }
  }
}
```

Then Atlas adds:

```text
--mcp-config <path-to-.atlas-computer-use.mcp.json>
```

This follows the existing ephemeral config approach used for Atlas bus and planner MCP injection.

### Codex

Atlas injects a stdio MCP server through Codex per-session config overrides. The exact config shape must match the Codex version supported by this repository, but the logical server is:

```toml
[mcp_servers.open_computer_use]
command = "/path/to/open-computer-use"
args = ["mcp"]
```

The implementation should keep this in `computer_use::inject` so Codex config shape changes are isolated.

### OpenCode

Atlas deep-merges a worktree-local `opencode.json` MCP entry:

```json
{
  "mcp": {
    "open_computer_use": {
      "type": "local",
      "command": ["/path/to/open-computer-use", "mcp"]
    }
  }
}
```

This must preserve existing repository config, matching the existing OpenCode merge behavior.

## Helper Packaging

Atlas ships a fixed helper binary:

```text
Atlas.app/
  Contents/
    Resources/
      sidecars/
        open-computer-use
```

The exact Tauri bundle location should be implemented through Tauri path/resource APIs rather than string concatenation where possible.

The build/release process records:

- Upstream repository
- Upstream release or commit
- Helper binary version
- License notice

Atlas upgrades the helper only through app updates. There is no automatic helper download.

## Diagnostics

Diagnostics are best-effort and non-blocking.

Commands:

```bash
open-computer-use --version
open-computer-use doctor
```

Behavior:

- Use short timeouts.
- Capture stdout and stderr.
- Prefer structured parsing only if the upstream output is stable.
- Otherwise show a concise raw summary.
- Never block Settings rendering on diagnostics.

Status states:

- `disabled`
- `unsupported_platform`
- `missing`
- `not_executable`
- `found`
- `doctor_failed`
- `permission_missing`
- `ready`
- `unknown`

Permission diagnostics are advisory. Missing Accessibility or Screen Recording permissions do not prevent injection; the tool call may fail inside the sidecar until the user grants permissions.

## Trust Model

This design intentionally implements a full-trust mode.

Atlas does not:

- Ask before first use of each app.
- Maintain a Computer Use app allowlist.
- Block Terminal, browsers, email clients, chat apps, or payment screens at the Atlas layer.
- Intercept tool calls for high-risk actions.
- Inspect or redact every screenshot returned by the sidecar.

Boundaries still exist outside Atlas:

- macOS privacy permissions.
- The target app's own security model.
- Upstream `open-computer-use` behavior and limitations.
- The agent provider's MCP/tool-call behavior.

The Settings copy must make this trust model clear without creating an additional approval flow.

## Error Handling

- Helper missing: do not inject; show missing status.
- Helper not executable: do not inject; show not executable status.
- Unsupported platform: hide or disable enablement and show macOS-only status.
- Version command fails: keep helper status if executable, show version unknown.
- Doctor fails: show doctor error, do not prevent session start.
- Permission missing: show advisory state, still inject.
- Agent MCP startup fails: the agent session owns the immediate error; Atlas only guarantees the injected config was generated.
- Settings disabled: no injection for future sessions.

## Data Model

Use the existing app settings storage unless a stronger local pattern already exists.

Suggested keys:

- `computer_use_enabled`: boolean string value, default false.
- `computer_use_last_doctor`: optional cached text/status if there is already a settings-cache pattern.

The first version can avoid a new database table.

## UI Scope

Settings-only UI is enough for the first version.

No session Computer Use panel is added because Atlas is not proxying tool calls and therefore cannot reliably show a complete action history.

The existing transcript/timeline can still show whatever the underlying agent emits for MCP calls.

All copy goes through:

- `src/i18n/en.ts`
- `src/i18n/zh.ts`

## Testing

Automated tests:

- Settings load/save for `computer_use_enabled`.
- Helper path resolution in dev and packaged-like modes.
- Missing helper and not-executable helper diagnostics.
- Doctor/version timeout handling.
- Claude injection includes `open_computer_use` when enabled and omits it when disabled.
- Codex injection includes `open_computer_use` when enabled and omits it when disabled.
- OpenCode merge preserves existing config and adds `open_computer_use`.
- Injection never writes global Codex, Claude, Gemini, or OpenCode config.
- i18n keys exist in English and Chinese.

Repository verification:

```bash
pnpm build
git diff --check
```

Manual macOS verification:

1. Open Settings and enable Computer Use.
2. Run Recheck/doctor and confirm helper and permission status render.
3. Start a new agent session.
4. Ask the agent to call `list_apps` or `get_app_state`.
5. Open TextEdit and ask the agent to type a short test string.
6. Disable Computer Use.
7. Start another new session and confirm Computer Use is not injected.

## Rollout

Phase 1:

- Add settings, helper resolver, diagnostics, and session injection.
- Bundle a fixed helper binary in local development and packaged app builds.
- Verify macOS happy path manually with TextEdit.

Phase 2:

- Improve packaging automation, version reporting, and diagnostics parsing.
- Add clearer release notes and license notices.
- Add more agent-provider-specific injection tests.

Phase 3:

- Reconsider an Atlas proxy MCP if users need action history, policy controls, screenshot redaction, or per-action observability.

## Risks

The main risk is not technical feasibility; it is product control. Direct MCP sidecar access means Atlas cannot fully audit or constrain Computer Use actions. This is acceptable for the selected first version because the user explicitly chose global trust and direct connection.

The second risk is upstream drift. `open-computer-use` command-line flags, MCP schema, or doctor output may change. Atlas should pin a version and isolate all assumptions in `computer_use`.

The third risk is macOS packaging. Accessibility and Screen Recording permission prompts are sensitive to app identity, helper identity, code signing, and bundle layout. Manual macOS validation is required before calling the integration complete.

## Open Decisions

None for the first implementation plan. The design assumes:

- macOS-only.
- Bundled fixed helper binary.
- Global enablement for all new sessions.
- Direct stdio MCP connection.
- Settings-only UI.
- Full-trust behavior at the Atlas layer.
