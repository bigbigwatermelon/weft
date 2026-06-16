# Computer Use Integration Verification Record

Date: 2026-06-16 09:19:43 CST

Worktree:
isolated superpowers worktree `codex-computer-use-integration`

Commit range under verification:

- Base merge point: `a69a981cd51830775712c3214055608cb5f8319a`
- Latest committed base before this record: `c5c1004c0717cd7fcbd84129eb1a4bc77a746b9f`
- Verified change set: `c5c1004` plus the digest hardening and this verification record, to be committed as `chore(computer-use): verify bundled runtime digest`.

## Sidecar Digest Metadata

`pnpm computer-use:prepare-sidecar` regenerated
`src-tauri/sidecars/open-computer-use.version.json` with committed binary digests:

```json
{
  "wrapperSha256": "ee8a68fb780c0b3a89e75220f94dca42e3884a7fbb7c553d9bd7b31af80bab6b",
  "runtimeSha256": "07be48ca254b1113bd61585a48f4274e5c6b590fc9a6c957e5533f2fe96695b2"
}
```

`pnpm computer-use:verify-sidecar` now reads this metadata and fails if
`wrapperSha256` or `runtimeSha256` is missing or does not match the current
source-controlled wrapper/runtime files.

## Automated Verification

| Command | Exit | Key output |
| --- | ---: | --- |
| `pnpm computer-use:prepare-sidecar` | 0 | `Verified open-computer-use 0.1.53`; `Prepared open-computer-use 0.1.53` |
| `node --check scripts/prepare-open-computer-use-sidecar.mjs` | 0 | no syntax errors |
| `pnpm computer-use:verify-sidecar` | 0 | `Verified open-computer-use 0.1.53` |
| `pnpm build` | 0 | `tsc && vite build`; `2573 modules transformed`; Vite chunk-size warning only |
| `cargo test --manifest-path src-tauri/Cargo.toml` | 0 | lib tests `270 passed; 0 failed`; integration tests passed; warning only: `repo_map_json` unused |
| `pnpm preflight` | 0 | `git diff --check fork/main...HEAD`, Atlas identity check, `pnpm build`, and Rust tests all passed |
| `pnpm tauri build --debug --bundles app` | 1 | Built `target/debug/atlas-app`; produced `Atlas.app` and `Atlas.app.tar.gz`; failed only because `TAURI_SIGNING_PRIVATE_KEY` is not set |
| packaged wrapper `--version` | 0 | `0.1.53` |
| packaged wrapper/runtime sha256 | 0 | same digests as metadata: wrapper `ee8a68...bab6b`, runtime `07be48...695b2` |

Tauri build blocker output:

```text
Finished 1 bundle at:
  src-tauri/target/debug/bundle/macos/Atlas.app
  src-tauri/target/debug/bundle/macos/Atlas.app.tar.gz (updater)

A public key has been found, but no private key. Make sure to set `TAURI_SIGNING_PRIVATE_KEY` environment variable.
```

Packaged resource checks:

```text
src-tauri/target/debug/bundle/macos/Atlas.app/Contents/Resources/sidecars/open-computer-use --version
0.1.53

ee8a68fb780c0b3a89e75220f94dca42e3884a7fbb7c553d9bd7b31af80bab6b  .../Resources/sidecars/open-computer-use
07be48ca254b1113bd61585a48f4274e5c6b590fc9a6c957e5533f2fe96695b2  .../Resources/sidecars/open-computer-use-runtime/Open Computer Use.app/Contents/MacOS/OpenComputerUse
```

## Manual Product Smoke

Final smoke used the debug `Atlas.app` bundle produced by the latest Tauri
build, not the user's real Atlas home.

Environment:

```text
ATLAS_HOME=/private/tmp/atlas-smoke.AsNffU
CODEX_HOME=<unset>
```

The temporary DB key was generated for this run and was not logged.

Observed UI path:

1. Started `src-tauri/target/debug/bundle/macos/Atlas.app/Contents/MacOS/atlas-app`.
2. Connected Tauri driver to the app bridge on port `9223`.
3. Opened Settings.
4. Opened Settings > Computer Use.
5. Enabled "For new sessions".
6. Settings showed:

```text
Status: 权限缺失
Helper: .../Atlas.app/Contents/Resources/sidecars/open-computer-use
Version: "0.1.53"
Permissions: accessibility=missing, screenRecording=missing
```

No macOS Accessibility or Screen Recording permissions were modified.

Agent session smoke:

1. Created task `Computer Use digest smoke`.
2. Sent prompt asking the agent to list macOS apps through Computer Use.
3. Atlas spawned Codex through exec transport:

```text
codex exec --ignore-user-config ... \
  -c mcp_servers.open_computer_use.command=".../Resources/sidecars/open-computer-use" \
  -c mcp_servers.open_computer_use.args=["mcp"] \
  --json --cd /private/tmp/atlas-smoke.AsNffU/leads/1 ...
```

4. UI showed an `mcp_tool_call`.
5. Final UI response said `list_apps` was called and summarized running apps, including Codex, Google Chrome, 微信, Steam, System Settings, Terminal, Tencent Lemon, Calendar, Notes, Finder, and Atlas.

The Atlas app and the bundle-scoped Open Computer Use app-agent process were
stopped after the smoke.

## Global Config Evidence

Before final bundle smoke:

```text
1781570153 Jun 16 08:35:53 2026 /Users/chenxingyao/.codex/config.toml
1778765058 May 14 21:24:18 2026 /Users/chenxingyao/.claude.json
1778765058 May 14 21:24:18 2026 /Users/chenxingyao/.config/opencode/opencode.json
```

Before safe grep:

```text
176:[projects."/private/tmp/atlas-smoke.ATGsFD/leads/1"]
179:[projects."/private/tmp/atlas-smoke.hsISyH/leads/1"]
```

After final bundle smoke:

```text
1781570153 Jun 16 08:35:53 2026 /Users/chenxingyao/.codex/config.toml
1778765058 May 14 21:24:18 2026 /Users/chenxingyao/.claude.json
1778765058 May 14 21:24:18 2026 /Users/chenxingyao/.config/opencode/opencode.json
```

After safe grep:

```text
176:[projects."/private/tmp/atlas-smoke.ATGsFD/leads/1"]
179:[projects."/private/tmp/atlas-smoke.hsISyH/leads/1"]
```

Conclusion:

- `~/.codex/config.toml` mtime did not change during the latest smoke.
- Safe grep content did not change during the latest smoke.
- No `/private/tmp/atlas-smoke.AsNffU` entry was added.
- No persistent `open_computer_use`, `open-computer-use`, `computer_use`,
  `sidecars`, or `mcp_servers.open*` entry exists in the safe grep output.

The two existing `/private/tmp/atlas-smoke.*` entries were already present before
this smoke and were not edited per the global-config safety rule.

## Residual Risks

- `pnpm tauri build --debug --bundles app` exits 1 on this machine because the
  updater signing public key is configured but `TAURI_SIGNING_PRIVATE_KEY` is not
  set. The debug app bundle and sidecar resources were still produced and
  verified.
- The user's `~/.codex/config.toml` still contains two old smoke project entries
  from earlier failed verification runs. This task did not edit or revert the
  user-global config.
- The machine reports macOS Accessibility and Screen Recording permissions as
  missing for Computer Use doctor. The smoke intentionally did not modify those
  permissions.
- A dev-mode attempt using `target/debug/sidecars/open-computer-use` showed a
  helper `--version` timeout in Settings, while the source-controlled sidecar and
  packaged bundle sidecar both returned `0.1.53`. Final product smoke therefore
  used the latest packaged debug `Atlas.app` bundle, which matches the resource
  layout users receive.
