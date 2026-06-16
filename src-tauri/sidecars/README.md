# Computer Use Sidecar

Atlas bundles a pinned `open-computer-use` runtime from `iFurySt/open-codex-computer-use`.

The source-controlled runtime is prepared by:

```bash
pnpm computer-use:prepare-sidecar
```

Expected runtime paths:

```text
src-tauri/sidecars/open-computer-use
src-tauri/sidecars/open-computer-use-runtime/Open Computer Use.app/Contents/MacOS/OpenComputerUse
src-tauri/sidecars/open-computer-use.version.json
```

The wrapper at `src-tauri/sidecars/open-computer-use` forwards all arguments to the bundled native app binary. Atlas injects it into new agent sessions as:

```text
<resource-dir>/sidecars/open-computer-use mcp
```

Pinned version:

```text
open-computer-use@0.1.53
GitHub tag: v0.1.53
GitHub commit: b753b790cace188152ffb755cd13b2ac9ff6ebf7
```

Do not run these upstream commands from Atlas:

```text
open-computer-use install-codex
open-computer-use install-claude
open-computer-use install-opencode
```

Those commands edit user-global agent configs. Atlas owns session-scoped MCP injection instead.

Verification:

```bash
pnpm computer-use:verify-sidecar
src-tauri/sidecars/open-computer-use --version
src-tauri/sidecars/open-computer-use help call
src-tauri/sidecars/open-computer-use call list_apps
```

`pnpm computer-use:verify-sidecar` checks the pinned runtime version, command surface,
and the sha256 digests recorded in `open-computer-use.version.json` for both the
wrapper and native runtime binary.
