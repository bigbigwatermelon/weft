# Real Open Computer Use Sidecar Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Atlas's metadata-only Computer Use placeholder with the real pinned `open-computer-use` macOS runtime from `iFurySt/open-codex-computer-use`, bundle it into Atlas, and verify it can be called by agent sessions through the existing stdio MCP injection path.

**Architecture:** Keep the current Atlas-owned Settings, helper discovery, diagnostics, and per-session MCP injection code. Add a reproducible sidecar preparation script that fetches the upstream `open-computer-use@0.1.53` release artifact, verifies its npm integrity and GitHub provenance metadata, extracts the native macOS app runtime, and creates an Atlas-owned wrapper at `src-tauri/sidecars/open-computer-use`. Atlas must execute the bundled runtime directly and must never run upstream installer commands that modify user-global agent configs.

**Tech Stack:** Tauri v2 resources, Rust 2021 helper diagnostics, Node.js preparation script, npm registry package `open-computer-use@0.1.53`, macOS native app bundle, bash wrapper, existing React Settings panel.

---

## Source Spec

- Existing Atlas implementation branch: `codex/computer-use-integration`
- Existing design spec: `docs/superpowers/specs/2026-06-16-computer-use-integration-design.md`
- Upstream project: `https://github.com/iFurySt/open-codex-computer-use`
- Pinned GitHub tag: `v0.1.53`
- Pinned commit: `b753b790cace188152ffb755cd13b2ac9ff6ebf7`
- Pinned npm package: `open-computer-use@0.1.53`
- npm tarball: `https://registry.npmjs.org/open-computer-use/-/open-computer-use-0.1.53.tgz`
- npm shasum: `d740b0c3af25ecc706ca747c1758742a365658ab`
- npm integrity: `sha512-5qwCPl7Gm4Wk2i/wFkq2dVLN2SzNRQSJTd95zXdGF+u5ZsUXkFx1IFVdNbYelWOpc4fgy8Z8/gYrbacj/2chig==`

## Correction From Previous Plan

The first implementation wired Atlas to a future helper path and committed only metadata. That was useful plumbing, but it did not actually integrate the upstream Computer Use project. This plan fixes that by committing a real, verified upstream runtime into `src-tauri/sidecars/` and proving Atlas can execute it.

The upstream GitHub repo uses Git LFS for some payloads. Local checkout failed when `git-lfs` was unavailable, so the implementation should use the official npm release artifact as the binary distribution source while preserving the GitHub tag and commit in source-controlled metadata.

## File Structure

Create:

- `scripts/prepare-open-computer-use-sidecar.mjs`
  Downloads the pinned npm tarball through `npm pack`, verifies `version`, `shasum`, and `integrity`, extracts the macOS app bundle, writes the Atlas wrapper, writes pinned metadata, and supports `--verify-only`.

- `src-tauri/sidecars/open-computer-use`
  Generated executable wrapper. It resolves the bundled app runtime beside itself and forwards all CLI arguments to the native `OpenComputerUse` binary.

- `src-tauri/sidecars/open-computer-use-runtime/Open Computer Use.app/`
  Generated real upstream macOS runtime copied from `open-computer-use@0.1.53`.

- `src-tauri/sidecars/open-computer-use-runtime/LICENSE.open-computer-use`
  Generated upstream license copy for bundled third-party runtime compliance.

Modify:

- `package.json`
  Add scripts for preparing and verifying the sidecar.

- `src-tauri/sidecars/open-computer-use.version.json`
  Replace placeholder metadata with complete npm and GitHub provenance.

- `src-tauri/sidecars/README.md`
  Document the real pinned sidecar, update process, verification commands, and prohibited global installer scripts.

- `src-tauri/tauri.conf.json`
  Bundle `sidecars/open-computer-use`, `sidecars/open-computer-use-runtime`, and `sidecars/open-computer-use.version.json` as Tauri resources.

Do not modify:

- `~/.codex/config.toml`
- `~/.claude.json`
- `~/.config/opencode/opencode.json`
- Any upstream installer script under the npm package
- Existing Computer Use Settings UI behavior unless real-runtime verification exposes a concrete bug

---

### Task 1: Add Reproducible Sidecar Preparation

**Files:**
- Create: `scripts/prepare-open-computer-use-sidecar.mjs`
- Modify: `package.json`
- Modify: `src-tauri/sidecars/open-computer-use.version.json`

- [ ] **Step 1: Add package scripts**

Modify `package.json` so the `scripts` object contains these two entries after `"preview": "vite preview"`:

```json
{
  "computer-use:prepare-sidecar": "node scripts/prepare-open-computer-use-sidecar.mjs",
  "computer-use:verify-sidecar": "node scripts/prepare-open-computer-use-sidecar.mjs --verify-only"
}
```

The resulting `scripts` block should be:

```json
{
  "postinstall": "node scripts/install-git-hooks.mjs",
  "dev": "vite",
  "build": "tsc && vite build",
  "preview": "vite preview",
  "computer-use:prepare-sidecar": "node scripts/prepare-open-computer-use-sidecar.mjs",
  "computer-use:verify-sidecar": "node scripts/prepare-open-computer-use-sidecar.mjs --verify-only",
  "ci:pre-push": "pnpm preflight",
  "preflight": "scripts/preflight.sh",
  "preflight:quick": "scripts/preflight.sh --quick",
  "tauri": "tauri"
}
```

- [ ] **Step 2: Create the preparation script**

Create `scripts/prepare-open-computer-use-sidecar.mjs`:

```javascript
#!/usr/bin/env node

import { createHash } from "node:crypto";
import {
  chmodSync,
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "..");
const sidecarDir = join(repoRoot, "src-tauri", "sidecars");
const runtimeDir = join(sidecarDir, "open-computer-use-runtime");
const runtimeAppName = "Open Computer Use.app";
const runtimeAppPath = join(runtimeDir, runtimeAppName);
const runtimeBinaryPath = join(runtimeAppPath, "Contents", "MacOS", "OpenComputerUse");
const wrapperPath = join(sidecarDir, "open-computer-use");
const metadataPath = join(sidecarDir, "open-computer-use.version.json");
const packageName = "open-computer-use";
const packageVersion = "0.1.53";

const expected = {
  name: packageName,
  npmVersion: packageVersion,
  npmTarball: "https://registry.npmjs.org/open-computer-use/-/open-computer-use-0.1.53.tgz",
  npmShasum: "d740b0c3af25ecc706ca747c1758742a365658ab",
  npmIntegrity:
    "sha512-5qwCPl7Gm4Wk2i/wFkq2dVLN2SzNRQSJTd95zXdGF+u5ZsUXkFx1IFVdNbYelWOpc4fgy8Z8/gYrbacj/2chig==",
  upstream: "https://github.com/iFurySt/open-codex-computer-use",
  gitTag: "v0.1.53",
  pinnedRef: "b753b790cace188152ffb755cd13b2ac9ff6ebf7",
  license: "MIT",
};

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd ?? repoRoot,
    encoding: "utf8",
    stdio: options.stdio ?? "pipe",
  });

  if (result.status !== 0) {
    const stdout = result.stdout ? `\nstdout:\n${result.stdout}` : "";
    const stderr = result.stderr ? `\nstderr:\n${result.stderr}` : "";
    throw new Error(
      `${command} ${args.join(" ")} failed with exit code ${result.status}${stdout}${stderr}`,
    );
  }

  return result;
}

function hashFile(path, algorithm, encoding) {
  const hash = createHash(algorithm);
  hash.update(readFileSync(path));
  return hash.digest(encoding);
}

function assertEqual(label, actual, expectedValue) {
  if (actual !== expectedValue) {
    throw new Error(`${label} mismatch: expected ${expectedValue}, got ${actual}`);
  }
}

function verifyTarball(path, packInfo) {
  assertEqual("npm package name", packInfo.name, expected.name);
  assertEqual("npm package version", packInfo.version, expected.npmVersion);
  assertEqual("npm shasum from npm pack", packInfo.shasum, expected.npmShasum);
  assertEqual("npm integrity from npm pack", packInfo.integrity, expected.npmIntegrity);

  const actualSha1 = hashFile(path, "sha1", "hex");
  const actualSha512 = hashFile(path, "sha512", "base64");
  const expectedSha512 = expected.npmIntegrity.replace(/^sha512-/, "");

  assertEqual("tarball sha1", actualSha1, expected.npmShasum);
  assertEqual("tarball sha512", actualSha512, expectedSha512);
}

function writeWrapper() {
  const wrapper = `#!/usr/bin/env bash
set -euo pipefail

DIR="$(cd "$(dirname "\${BASH_SOURCE[0]}")" && pwd -P)"
RUNTIME="\${DIR}/open-computer-use-runtime/Open Computer Use.app/Contents/MacOS/OpenComputerUse"

if [[ ! -x "\${RUNTIME}" ]]; then
  echo "Atlas Computer Use runtime is missing or not executable: \${RUNTIME}" >&2
  exit 127
fi

exec "\${RUNTIME}" "$@"
`;

  writeFileSync(wrapperPath, wrapper);
  chmodSync(wrapperPath, 0o755);
}

function writeMetadata() {
  const metadata = {
    name: expected.name,
    upstream: expected.upstream,
    license: expected.license,
    gitTag: expected.gitTag,
    pinnedRef: expected.pinnedRef,
    npmPackage: `${packageName}@${packageVersion}`,
    npmTarball: expected.npmTarball,
    npmShasum: expected.npmShasum,
    npmIntegrity: expected.npmIntegrity,
    binary: "open-computer-use",
    runtime: "open-computer-use-runtime/Open Computer Use.app/Contents/MacOS/OpenComputerUse",
    preparedBy: "scripts/prepare-open-computer-use-sidecar.mjs",
    notes:
      "Atlas bundles the native runtime from the pinned npm release artifact and does not run upstream global installer scripts.",
  };

  writeFileSync(metadataPath, `${JSON.stringify(metadata, null, 2)}\n`);
}

function verifyLocalRuntime() {
  if (!existsSync(wrapperPath)) {
    throw new Error(`Missing wrapper: ${wrapperPath}`);
  }

  if (!existsSync(runtimeBinaryPath)) {
    throw new Error(`Missing runtime binary: ${runtimeBinaryPath}`);
  }

  const versionResult = run(wrapperPath, ["--version"]);
  const version = versionResult.stdout.trim();
  assertEqual("runtime version", version, packageVersion);

  const helpResult = run(wrapperPath, ["help", "call"]);
  if (!helpResult.stdout.includes("open-computer-use call list_apps")) {
    throw new Error("Runtime help output does not expose the expected call command");
  }

  console.log(`Verified ${packageName} ${packageVersion} at ${wrapperPath}`);
}

function prepareSidecar() {
  const tmpRoot = mkdtempSync(join(tmpdir(), "atlas-open-computer-use-"));

  try {
    const pack = run("npm", [
      "pack",
      `${packageName}@${packageVersion}`,
      "--json",
      "--pack-destination",
      tmpRoot,
    ]);

    const packInfo = JSON.parse(pack.stdout)[0];
    const tarballPath = join(tmpRoot, packInfo.filename);
    verifyTarball(tarballPath, packInfo);

    const extractRoot = join(tmpRoot, "extract");
    mkdirSync(extractRoot, { recursive: true });
    run("tar", ["-xzf", tarballPath, "-C", extractRoot]);

    const packageRoot = join(extractRoot, "package");
    const sourceAppPath = join(packageRoot, "dist", runtimeAppName);
    const sourceLicensePath = join(packageRoot, "LICENSE");

    if (!existsSync(sourceAppPath)) {
      throw new Error(`Missing runtime app in npm package: ${sourceAppPath}`);
    }

    rmSync(runtimeDir, { recursive: true, force: true });
    mkdirSync(runtimeDir, { recursive: true });
    cpSync(sourceAppPath, runtimeAppPath, { recursive: true });

    if (existsSync(sourceLicensePath)) {
      cpSync(sourceLicensePath, join(runtimeDir, "LICENSE.open-computer-use"));
    }

    chmodSync(runtimeBinaryPath, 0o755);
    writeWrapper();
    writeMetadata();
    verifyLocalRuntime();
    console.log(`Prepared ${packageName} ${packageVersion}`);
  } finally {
    rmSync(tmpRoot, { recursive: true, force: true });
  }
}

if (process.argv.includes("--verify-only")) {
  verifyLocalRuntime();
} else {
  prepareSidecar();
}
```

- [ ] **Step 3: Verify the script is syntactically valid before running network work**

Run:

```bash
node --check scripts/prepare-open-computer-use-sidecar.mjs
```

Expected: Node prints no output and exits with status `0`.

- [ ] **Step 4: Commit the reproducible preparation script**

Run:

```bash
git add package.json scripts/prepare-open-computer-use-sidecar.mjs
git commit -m "chore(computer-use): add sidecar preparation script"
```

Expected output includes:

```text
chore(computer-use): add sidecar preparation script
```

---

### Task 2: Generate And Commit The Real Upstream Runtime

**Files:**
- Create: `src-tauri/sidecars/open-computer-use`
- Create: `src-tauri/sidecars/open-computer-use-runtime/Open Computer Use.app/`
- Create: `src-tauri/sidecars/open-computer-use-runtime/LICENSE.open-computer-use`
- Modify: `src-tauri/sidecars/open-computer-use.version.json`

- [ ] **Step 1: Run the preparation script**

Run:

```bash
pnpm computer-use:prepare-sidecar
```

Expected output includes:

```text
Verified open-computer-use 0.1.53 at
Prepared open-computer-use 0.1.53
```

- [ ] **Step 2: Prove the generated wrapper runs the real native binary**

Run:

```bash
src-tauri/sidecars/open-computer-use --version
src-tauri/sidecars/open-computer-use help
src-tauri/sidecars/open-computer-use help call
file "src-tauri/sidecars/open-computer-use-runtime/Open Computer Use.app/Contents/MacOS/OpenComputerUse"
```

Expected output includes:

```text
0.1.53
Commands:
  mcp                  Start the stdio MCP server.
  doctor               Print permission status and launch onboarding if needed.
  list-apps            Print running or recently used apps.
Usage:
  open-computer-use call <tool> [--args '<json-object>']
Mach-O universal binary
```

- [ ] **Step 3: Inspect the generated metadata**

Run:

```bash
cat src-tauri/sidecars/open-computer-use.version.json
```

Expected exact JSON:

```json
{
  "name": "open-computer-use",
  "upstream": "https://github.com/iFurySt/open-codex-computer-use",
  "license": "MIT",
  "gitTag": "v0.1.53",
  "pinnedRef": "b753b790cace188152ffb755cd13b2ac9ff6ebf7",
  "npmPackage": "open-computer-use@0.1.53",
  "npmTarball": "https://registry.npmjs.org/open-computer-use/-/open-computer-use-0.1.53.tgz",
  "npmShasum": "d740b0c3af25ecc706ca747c1758742a365658ab",
  "npmIntegrity": "sha512-5qwCPl7Gm4Wk2i/wFkq2dVLN2SzNRQSJTd95zXdGF+u5ZsUXkFx1IFVdNbYelWOpc4fgy8Z8/gYrbacj/2chig==",
  "binary": "open-computer-use",
  "runtime": "open-computer-use-runtime/Open Computer Use.app/Contents/MacOS/OpenComputerUse",
  "preparedBy": "scripts/prepare-open-computer-use-sidecar.mjs",
  "notes": "Atlas bundles the native runtime from the pinned npm release artifact and does not run upstream global installer scripts."
}
```

- [ ] **Step 4: Check that the wrapper and native binary are executable**

Run:

```bash
test -x src-tauri/sidecars/open-computer-use
test -x "src-tauri/sidecars/open-computer-use-runtime/Open Computer Use.app/Contents/MacOS/OpenComputerUse"
```

Expected: both commands exit with status `0`.

- [ ] **Step 5: Commit the real sidecar runtime**

Run:

```bash
git add src-tauri/sidecars/open-computer-use src-tauri/sidecars/open-computer-use-runtime src-tauri/sidecars/open-computer-use.version.json
git commit -m "feat(computer-use): bundle real open-computer-use runtime"
```

Expected output includes:

```text
feat(computer-use): bundle real open-computer-use runtime
```

---

### Task 3: Bundle The Runtime As A Tauri Resource

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/sidecars/README.md`

- [ ] **Step 1: Update Tauri resources**

Modify `src-tauri/tauri.conf.json` so the bundle resources block is exactly:

```json
"resources": [
  "sidecars/open-computer-use",
  "sidecars/open-computer-use-runtime",
  "sidecars/open-computer-use.version.json"
]
```

This keeps the wrapper and app bundle as ordinary resources because Atlas already resolves `resource_dir()/sidecars/open-computer-use` and starts it as a stdio process.

- [ ] **Step 2: Replace sidecar documentation**

Replace `src-tauri/sidecars/README.md` with:

````markdown
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
````

- [ ] **Step 3: Verify the Tauri config parses**

Run:

```bash
pnpm tauri info
```

Expected output includes:

```text
Environment
```

The command must not report malformed `tauri.conf.json`.

- [ ] **Step 4: Commit packaging config and docs**

Run:

```bash
git add src-tauri/tauri.conf.json src-tauri/sidecars/README.md
git commit -m "chore(computer-use): package sidecar resources"
```

Expected output includes:

```text
chore(computer-use): package sidecar resources
```

---

### Task 4: Verify Atlas Uses The Bundled Runtime

**Files:**
- No planned production code changes
- Test: `src-tauri/src/computer_use/helper.rs`
- Test: `src-tauri/src/computer_use/diagnostics.rs`
- Test: `src-tauri/src/computer_use/inject.rs`

- [ ] **Step 1: Run sidecar verify script**

Run:

```bash
pnpm computer-use:verify-sidecar
```

Expected output includes:

```text
Verified open-computer-use 0.1.53 at
```

- [ ] **Step 2: Run direct helper commands**

Run:

```bash
src-tauri/sidecars/open-computer-use doctor
src-tauri/sidecars/open-computer-use call list_apps
```

Expected: `doctor` may report missing Accessibility or Screen Recording permissions on a fresh machine. That is acceptable only if the command clearly comes from `open-computer-use` and does not fail with `No such file or directory`, `permission denied`, or `Atlas Computer Use runtime is missing`.

`call list_apps` should either print a tool result or a permissions-related error from the upstream runtime. It must not fail because the helper is absent.

- [ ] **Step 3: Run focused Rust tests with the real helper path**

Run:

```bash
ATLAS_COMPUTER_USE_HELPER="$(pwd)/src-tauri/sidecars/open-computer-use" cargo test computer_use
```

Expected output includes:

```text
test result: ok.
```

- [ ] **Step 4: Confirm MCP injection still points at the Atlas wrapper**

Run:

```bash
ATLAS_COMPUTER_USE_HELPER="$(pwd)/src-tauri/sidecars/open-computer-use" cargo test computer_use::inject
```

Expected output includes:

```text
test result: ok.
```

The tests should prove that the injected command is the Atlas wrapper and the injected args include `mcp`.

- [ ] **Step 5: Commit only if test fixes were required**

If Tasks 4.1 through 4.4 pass without code changes, do not create a commit for this task.

If the tests reveal a real bug, fix the narrowest affected Rust file and commit:

```bash
git add src-tauri/src/computer_use/helper.rs src-tauri/src/computer_use/diagnostics.rs src-tauri/src/computer_use/inject.rs
git commit -m "fix(computer-use): use bundled runtime path"
```

Expected output includes:

```text
fix(computer-use): use bundled runtime path
```

---

### Task 5: Full Verification And Manual Product Smoke

**Files:**
- No planned code changes

- [ ] **Step 1: Run frontend build**

Run:

```bash
pnpm build
```

Expected output includes:

```text
built in
```

The existing Vite chunk size warning is acceptable if it matches the current branch behavior.

- [ ] **Step 2: Run all Rust tests**

Run:

```bash
cd src-tauri && cargo test
```

Expected output includes:

```text
test result: ok.
```

- [ ] **Step 3: Run the repo preflight gate**

Run:

```bash
pnpm preflight
```

Expected output includes:

```text
All preflight checks passed.
```

- [ ] **Step 4: Build a debug Tauri app to catch resource packaging errors**

Run:

```bash
pnpm tauri build --debug --bundles app
```

Expected output includes:

```text
bundle
```

Accept success only when the command exits with status `0` and the log shows the app bundle was produced without missing-resource errors.

- [ ] **Step 5: Manually smoke test the product path**

Run:

```bash
pnpm tauri dev
```

In the Atlas app:

1. Open Settings.
2. Open Computer Use.
3. Confirm the panel reports helper version `0.1.53`.
4. Enable Computer Use.
5. Start a new agent session.
6. Ask the agent to list available desktop apps through Computer Use.

Expected:

```text
The new session receives a session-scoped MCP server command ending in "open-computer-use mcp".
The agent either lists apps or returns an upstream permission-gated Computer Use error.
No global Codex, Claude, or OpenCode config file is modified.
```

- [ ] **Step 6: Re-check global config timestamps**

Run:

```bash
stat -f "%Sm %N" ~/.codex/config.toml ~/.claude.json ~/.config/opencode/opencode.json 2>/dev/null || true
```

Expected: the timestamps must not change as a result of the Atlas sidecar preparation, diagnostics, or session creation flow.

- [ ] **Step 7: Run whitespace check**

Run:

```bash
git diff --check
```

Expected: no output and exit status `0`.

---

## Definition Of Done

- `src-tauri/sidecars/open-computer-use` exists, is executable, and returns `0.1.53` for `--version`.
- `src-tauri/sidecars/open-computer-use-runtime/Open Computer Use.app/Contents/MacOS/OpenComputerUse` exists, is executable, and is a Mach-O universal binary.
- `src-tauri/sidecars/open-computer-use.version.json` records npm package, npm tarball, shasum, integrity, GitHub tag, and pinned commit.
- `src-tauri/tauri.conf.json` bundles the wrapper, runtime directory, and metadata as resources.
- Atlas Settings diagnostics execute the real helper, not a placeholder.
- New agent sessions inject `open-computer-use mcp` through Atlas session config only.
- `pnpm computer-use:verify-sidecar`, `cargo test computer_use`, `pnpm build`, `pnpm preflight`, and `git diff --check` pass.
- `pnpm tauri build --debug --bundles app` passes or a narrower Tauri resource-packaging check is recorded with exact command output.
- No upstream `install-*` command is run.
- User-global agent config files are not modified.

## Self-Review Notes

- Spec coverage: The plan covers real upstream payload acquisition, provenance pinning, sidecar bundling, Atlas wrapper execution, Tauri resource packaging, diagnostics, MCP injection, and global-config safety.
- Placeholder scan: The plan contains exact file paths, exact package metadata, full preparation script code, concrete commands, and expected results.
- Type and command consistency: The wrapper path matches current Rust helper resolution, the runtime CLI commands match upstream `help` output, and package scripts call the new Node script directly.
