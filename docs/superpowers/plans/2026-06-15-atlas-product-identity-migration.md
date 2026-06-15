# Atlas Product Identity Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the Atlas product identity migration so the app has no source, runtime, protocol, packaging, asset, documentation, or local data dependency on the previous product identity.

**Architecture:** Treat Atlas as a fresh product identity with no compatibility bridge. Execute the migration as layered audits over the current large diff: first make ignored planning docs scan-clean, then verify runtime/data identity, backend symbols and protocols, frontend UI identity, assets/package metadata, desktop shell behavior, and final repository-wide scans.

**Tech Stack:** Tauri v2, Rust 2021, SeaORM, SQLite/SQLCipher, React 19, TypeScript, Vite, i18next, pnpm, Cargo, shell verification commands, observable browser checks.

---

## Scope Check

This spec is one coherent migration, not multiple independent product features. The layers are coupled by a single invariant: Atlas is a new identity and must not read, generate, mention, or accept the previous identity in source-controlled files, runtime defaults, generated protocol names, or packaging metadata.

## File Structure

### Plan And Spec Documents

- Modify: `docs/superpowers/specs/2026-06-12-agent-app-decoding-design.md`
  - Responsibility: historical design doc must describe the generic Agent App work using Atlas runtime paths and product identity.
- Modify: `docs/superpowers/plans/2026-06-12-agent-app-decoding.md`
  - Responsibility: historical implementation examples must use Atlas crate names, env vars, runtime paths, bus names, and sample text.
- Modify: `docs/superpowers/plans/2026-06-15-atlas-product-identity-migration.md`
  - Responsibility: this plan must be committed despite `docs/` being ignored.

### Product And Runtime Identity

- Modify/Audit: `package.json`
  - Responsibility: npm package identity.
- Modify/Audit: `index.html`
  - Responsibility: browser title, favicon path, and theme storage key.
- Modify/Audit: `src-tauri/Cargo.toml`
  - Responsibility: Rust package and library identity.
- Modify/Audit: `src-tauri/Cargo.lock`
  - Responsibility: generated package lock identity.
- Modify/Audit: `src-tauri/tauri.conf.json`
  - Responsibility: Tauri productName, bundle identifier, icons, and updater endpoint.
- Modify/Audit: `src-tauri/src/paths.rs`
  - Responsibility: Atlas home directory, database filename, app-managed workspace path helpers, and path tests.
- Modify/Audit: `src-tauri/src/config.rs`
  - Responsibility: environment variable names and persisted setting labels.
- Modify/Audit: `src-tauri/src/store/key.rs`
  - Responsibility: encryption key env var and keychain service identity.
- Modify/Audit: `src-tauri/src/power.rs`
  - Responsibility: OS power-management app id.
- Modify/Audit: `src-tauri/tests/db_encryption.rs`
  - Responsibility: encryption env var tests.
- Modify/Audit: `src-tauri/tests/ensure_default_workspace.rs`
  - Responsibility: fresh Atlas database/default workspace behavior.

### Backend Source Identity

- Modify/Audit: `src-tauri/src/adapters/mod.rs`
- Modify/Audit: `src-tauri/src/brief.rs`
- Modify/Audit: `src-tauri/src/check.rs`
- Modify/Audit: `src-tauri/src/claude.rs`
- Modify/Audit: `src-tauri/src/codex.rs`
- Modify/Audit: `src-tauri/src/codex_app_server.rs`
- Modify/Audit: `src-tauri/src/commands.rs`
- Modify/Audit: `src-tauri/src/detect.rs`
- Modify/Audit: `src-tauri/src/gc.rs`
- Modify/Audit: `src-tauri/src/git.rs`
- Modify/Audit: `src-tauri/src/lib.rs`
- Modify/Audit: `src-tauri/src/main.rs`
- Modify/Audit: `src-tauri/src/materialize.rs`
- Modify/Audit: `src-tauri/src/profile.rs`
- Modify/Audit: `src-tauri/src/sidecar.rs`
  - Responsibility: backend function names, error messages, comments, tests, sidecar/proxy names, and provider setup must not use the previous identity.

### Backup And Sync Identity

- Modify/Audit: `src-tauri/src/backup/git_remote.rs`
- Modify/Audit: `src-tauri/src/backup/mod.rs`
- Modify/Audit: `src-tauri/src/backup/recovery_key.rs`
- Modify/Audit: `src-tauri/src/backup/scheduler.rs`
- Modify/Audit: `src-tauri/src/backup/snapshot.rs`
- Modify/Audit: `src-tauri/tests/backup_end_to_end.rs`
- Modify/Audit: `src-tauri/tests/backup_git_remote.rs`
- Modify/Audit: `src-tauri/tests/backup_recovery.rs`
- Modify/Audit: `src-tauri/tests/backup_scheduler.rs`
- Modify/Audit: `src-tauri/tests/backup_snapshot.rs`
  - Responsibility: backup folder names, messages, recovery text, scheduler env vars, and backup tests must use Atlas names only.

### Agent, MCP, Hook, Sentinel, And Skill Protocol Identity

- Modify/Audit: `src-tauri/src/ask.rs`
- Modify/Audit: `src-tauri/src/bus/global.rs`
- Modify/Audit: `src-tauri/src/bus/inject.rs`
- Modify/Audit: `src-tauri/src/bus/server.rs`
- Modify/Audit: `src-tauri/src/planner.rs`
- Modify/Audit: `src-tauri/src/opencode.rs`
- Modify/Audit: `src-tauri/src/lead_chat/commands.rs`
- Modify/Audit: `src-tauri/src/lead_chat/engine.rs`
- Modify/Audit: `src-tauri/src/lead_chat/mod.rs`
- Modify/Audit: `src-tauri/src/lead_chat/proto.rs`
- Modify/Audit: `src-tauri/src/lead_chat/repo_state.rs`
- Modify/Audit: `src-tauri/src/lead_chat/sentinels.rs`
- Modify/Audit: `src-tauri/src/skills/inject.rs`
- Modify/Audit: `src-tauri/src/skills/mod.rs`
- Modify/Audit: `src-tauri/src/skills/parse.rs`
- Modify/Audit: `src-tauri/src/skills/sync.rs`
- Modify/Audit: `src-tauri/tests/bus_http.rs`
- Modify/Audit: `src-tauri/tests/lead_prompt.rs`
- Modify/Audit: `src-tauri/tests/lead_repo_state.rs`
- Modify/Audit: `src-tauri/tests/lead_sentinels.rs`
  - Responsibility: MCP server names, generated config files, sentinel namespace, skill layer names, injected ask hook filenames, opencode plugin filenames, and prompt text must be Atlas-only. Parsers must not accept previous names as aliases.

### Store, IM, And Integration Identity

- Modify/Audit: `src-tauri/src/im/feishu/ws.rs`
- Modify/Audit: `src-tauri/src/im/inbound.rs`
- Modify/Audit: `src-tauri/src/im/mod.rs`
- Modify/Audit: `src-tauri/src/im/outbound.rs`
- Modify/Audit: `src-tauri/src/store/entities/direction.rs`
- Modify/Audit: `src-tauri/src/store/entities/skill_source.rs`
- Modify/Audit: `src-tauri/src/store/legacy.rs`
- Modify/Audit: `src-tauri/src/store/mod.rs`
- Modify/Audit: `src-tauri/src/store/repo.rs`
- Modify/Audit: `src-tauri/tests/im_bridge.rs`
- Modify/Audit: `src-tauri/tests/m2_git.rs`
- Modify/Audit: `src-tauri/tests/m2_worktree.rs`
  - Responsibility: IM bridge text, store defaults, legacy/fresh database behavior, and integration tests must not expose or rely on previous identity strings.

### Frontend Product Identity

- Modify/Audit: `src/App.tsx`
- Modify/Audit: `src/board/RepoGraph.tsx`
- Modify/Audit: `src/board/ThreadBoard.tsx`
- Modify/Audit: `src/board/WorkspaceKanban.tsx`
- Modify/Audit: `src/components/CommandPalette.tsx`
- Modify/Audit: `src/components/EffectiveConfigDialog.tsx`
- Modify/Audit: `src/components/Inspect.tsx`
- Modify/Audit: `src/components/Markdown.tsx`
- Modify/Audit: `src/components/ui/Dialog.tsx`
- Modify/Audit: `src/components/ui/Select.tsx`
- Modify/Audit: `src/components/ui/StatusChip.tsx`
- Modify/Audit: `src/i18n/en.ts`
- Modify/Audit: `src/i18n/index.ts`
- Modify/Audit: `src/i18n/zh.ts`
- Modify/Audit: `src/index.css`
- Modify/Audit: `src/lib/api.ts`
- Modify/Audit: `src/lib/resume.ts`
- Modify/Audit: `src/nav/AppTopBar.tsx`
- Modify/Audit: `src/nav/WorkspaceNav.tsx`
- Modify/Audit: `src/session/ChatTimeline.tsx`
- Modify/Audit: `src/session/DiffPanel.tsx`
- Modify/Audit: `src/session/LeadTab.tsx`
- Modify/Audit: `src/session/ObserveView.tsx`
- Modify/Audit: `src/session/SessionView.tsx`
- Modify/Audit: `src/session/transcriptBits.ts`
- Modify/Audit: `src/settings/Backup.tsx`
- Modify/Audit: `src/state/shortcuts.ts`
- Modify/Audit: `src/state/store.tsx`
- Modify/Audit: `src/state/theme.ts`
  - Responsibility: visible copy, i18n keys, CSS class names, DOM ids, localStorage keys, custom events, command palette text, settings labels, and transcript rendering must use Atlas identity only.

### Documentation, Diagrams, And Assets

- Modify/Audit: `AGENTS.md`
- Modify/Audit: `ARCHITECTURE.md`
- Modify/Audit: `DESIGN.md`
- Modify/Audit: `PRODUCT.md`
- Modify/Audit: `README.md`
- Modify/Audit: `README.zh-CN.md`
- Modify/Audit: `assets/diagrams/arch-en.svg`
- Modify/Audit: `assets/diagrams/arch-zh.svg`
- Modify/Audit: `assets/diagrams/board-en.svg`
- Modify/Audit: `assets/diagrams/board-zh.svg`
- Modify/Audit: `assets/diagrams/im-en.svg`
- Modify/Audit: `assets/diagrams/im-zh.svg`
- Delete: README overview image whose filename uses the previous lowercase product prefix.
- Create/Audit: `assets/readme/atlas-overview.png`
- Create/Audit: `assets/brand/atlas-icon-embedded.png`
- Create/Audit: `assets/brand/atlas-icon-master.png`
- Create/Audit: `assets/brand/atlas-icon-source.svg`
- Delete: public SVG files whose filenames use the previous lowercase product prefix.
- Create/Audit: `public/atlas-icon.png`
- Create/Audit: `public/atlas-mark.png`
  - Responsibility: documentation, diagram source, README media, brand source assets, and public browser assets must be Atlas-only.

### Desktop Icons

- Modify/Audit: `src-tauri/icons/128x128.png`
- Modify/Audit: `src-tauri/icons/128x128@2x.png`
- Modify/Audit: `src-tauri/icons/32x32.png`
- Modify/Audit: `src-tauri/icons/64x64.png`
- Modify/Audit: `src-tauri/icons/Square107x107Logo.png`
- Modify/Audit: `src-tauri/icons/Square142x142Logo.png`
- Modify/Audit: `src-tauri/icons/Square150x150Logo.png`
- Modify/Audit: `src-tauri/icons/Square284x284Logo.png`
- Modify/Audit: `src-tauri/icons/Square30x30Logo.png`
- Modify/Audit: `src-tauri/icons/Square310x310Logo.png`
- Modify/Audit: `src-tauri/icons/Square44x44Logo.png`
- Modify/Audit: `src-tauri/icons/Square71x71Logo.png`
- Modify/Audit: `src-tauri/icons/Square89x89Logo.png`
- Modify/Audit: `src-tauri/icons/StoreLogo.png`
- Modify/Audit: `src-tauri/icons/icon.icns`
- Modify/Audit: `src-tauri/icons/icon.ico`
- Modify/Audit: `src-tauri/icons/icon.png`
- Modify/Audit: `src-tauri/icons/android/mipmap-hdpi/ic_launcher.png`
- Modify/Audit: `src-tauri/icons/android/mipmap-hdpi/ic_launcher_foreground.png`
- Modify/Audit: `src-tauri/icons/android/mipmap-hdpi/ic_launcher_round.png`
- Modify/Audit: `src-tauri/icons/android/mipmap-mdpi/ic_launcher.png`
- Modify/Audit: `src-tauri/icons/android/mipmap-mdpi/ic_launcher_foreground.png`
- Modify/Audit: `src-tauri/icons/android/mipmap-mdpi/ic_launcher_round.png`
- Modify/Audit: `src-tauri/icons/android/mipmap-xhdpi/ic_launcher.png`
- Modify/Audit: `src-tauri/icons/android/mipmap-xhdpi/ic_launcher_foreground.png`
- Modify/Audit: `src-tauri/icons/android/mipmap-xhdpi/ic_launcher_round.png`
- Modify/Audit: `src-tauri/icons/android/mipmap-xxhdpi/ic_launcher.png`
- Modify/Audit: `src-tauri/icons/android/mipmap-xxhdpi/ic_launcher_foreground.png`
- Modify/Audit: `src-tauri/icons/android/mipmap-xxhdpi/ic_launcher_round.png`
- Modify/Audit: `src-tauri/icons/android/mipmap-xxxhdpi/ic_launcher.png`
- Modify/Audit: `src-tauri/icons/android/mipmap-xxxhdpi/ic_launcher_foreground.png`
- Modify/Audit: `src-tauri/icons/android/mipmap-xxxhdpi/ic_launcher_round.png`
- Modify/Audit: `src-tauri/icons/ios/AppIcon-20x20@1x.png`
- Modify/Audit: `src-tauri/icons/ios/AppIcon-20x20@2x-1.png`
- Modify/Audit: `src-tauri/icons/ios/AppIcon-20x20@2x.png`
- Modify/Audit: `src-tauri/icons/ios/AppIcon-20x20@3x.png`
- Modify/Audit: `src-tauri/icons/ios/AppIcon-29x29@1x.png`
- Modify/Audit: `src-tauri/icons/ios/AppIcon-29x29@2x-1.png`
- Modify/Audit: `src-tauri/icons/ios/AppIcon-29x29@2x.png`
- Modify/Audit: `src-tauri/icons/ios/AppIcon-29x29@3x.png`
- Modify/Audit: `src-tauri/icons/ios/AppIcon-40x40@1x.png`
- Modify/Audit: `src-tauri/icons/ios/AppIcon-40x40@2x-1.png`
- Modify/Audit: `src-tauri/icons/ios/AppIcon-40x40@2x.png`
- Modify/Audit: `src-tauri/icons/ios/AppIcon-40x40@3x.png`
- Modify/Audit: `src-tauri/icons/ios/AppIcon-512@2x.png`
- Modify/Audit: `src-tauri/icons/ios/AppIcon-60x60@2x.png`
- Modify/Audit: `src-tauri/icons/ios/AppIcon-60x60@3x.png`
- Modify/Audit: `src-tauri/icons/ios/AppIcon-76x76@1x.png`
- Modify/Audit: `src-tauri/icons/ios/AppIcon-76x76@2x.png`
- Modify/Audit: `src-tauri/icons/ios/AppIcon-83.5x83.5@2x.png`
  - Responsibility: generated desktop/mobile app icon files must carry the Atlas mark.

## Shared Verification Helpers

These snippets avoid embedding the previous product name in the plan file while still scanning for it during execution.

```bash
LOWER="$(printf '%s%s%s%s' w e f t)"
PASCAL="$(printf '%s%s' W "${LOWER#?}")"
UPPER="$(printf '%s' "$LOWER" | tr '[:lower:]' '[:upper:]')"
OLD_IDENTITY_PATTERN="\\b${PASCAL}\\b|\\b${LOWER}\\b|${UPPER}|com\\.${LOWER}|${LOWER}\\.db|\\.${LOWER}|${LOWER}-|${LOWER}_|${LOWER}:"
```

Full source scan:

```bash
LOWER="$(printf '%s%s%s%s' w e f t)"
PASCAL="$(printf '%s%s' W "${LOWER#?}")"
UPPER="$(printf '%s' "$LOWER" | tr '[:lower:]' '[:upper:]')"
OLD_IDENTITY_PATTERN="\\b${PASCAL}\\b|\\b${LOWER}\\b|${UPPER}|com\\.${LOWER}|${LOWER}\\.db|\\.${LOWER}|${LOWER}-|${LOWER}_|${LOWER}:"
rg --no-ignore -n "$OLD_IDENTITY_PATTERN" . \
  -g '!node_modules' \
  -g '!src-tauri/target' \
  -g '!build' \
  -g '!*.png' \
  -g '!*.ico' \
  -g '!*.icns' \
  -g '!.git'
```

Expected final result: no output and exit code `1`.

Full filename scan:

```bash
LOWER="$(printf '%s%s%s%s' w e f t)"
PASCAL="$(printf '%s%s' W "${LOWER#?}")"
UPPER="$(printf '%s' "$LOWER" | tr '[:lower:]' '[:upper:]')"
find . \
  \( -path './node_modules' -o -path './src-tauri/target' -o -path './.git' -o -path './build' \) -prune -o \
  \( -iname "*${LOWER}*" -o -iname "*${PASCAL}*" -o -iname "*${UPPER}*" \) -print | sort
```

Expected final result: no output and exit code `0`.

Current changed-file manifest:

```bash
git status --short \
  | sed 's/^...//' \
  | sed 's/ -> /\n/' \
  | sort -u
```

Expected during execution: output is the working migration surface. Every source file in this output must be covered by one task below or by a recursive directory audit that includes it.

---

### Task 1: Make Plan And Historical Superpowers Docs Trackable And Atlas-Clean

**Files:**
- Modify: `docs/superpowers/specs/2026-06-12-agent-app-decoding-design.md`
- Modify: `docs/superpowers/plans/2026-06-12-agent-app-decoding.md`
- Modify: `docs/superpowers/plans/2026-06-15-atlas-product-identity-migration.md`

- [ ] **Step 1: Verify the plan file exists and is ignored by default**

Run:

```bash
test -f docs/superpowers/plans/2026-06-15-atlas-product-identity-migration.md
git check-ignore -v docs/superpowers/plans/2026-06-15-atlas-product-identity-migration.md
```

Expected: the first command succeeds; the second command prints a `.gitignore` rule for `docs/`.

- [ ] **Step 2: Verify current ignored-doc residuals**

Run:

```bash
LOWER="$(printf '%s%s%s%s' w e f t)"
PASCAL="$(printf '%s%s' W "${LOWER#?}")"
UPPER="$(printf '%s' "$LOWER" | tr '[:lower:]' '[:upper:]')"
OLD_IDENTITY_PATTERN="\\b${PASCAL}\\b|\\b${LOWER}\\b|${UPPER}|com\\.${LOWER}|${LOWER}\\.db|\\.${LOWER}|${LOWER}-|${LOWER}_|${LOWER}:"
rg --no-ignore -n "$OLD_IDENTITY_PATTERN" docs/superpowers \
  -g '!*.png' \
  -g '!*.ico' \
  -g '!*.icns'
```

Expected before patching: matches only in the 2026-06-12 historical spec and plan.

- [ ] **Step 3: Convert the historical spec mechanically**

Run this exact script:

```bash
node <<'NODE'
const fs = require('fs');
const path = 'docs/superpowers/specs/2026-06-12-agent-app-decoding-design.md';
let s = fs.readFileSync(path, 'utf8');
const lower = ['w','e','f','t'].join('');
const pascal = 'W' + lower.slice(1);
const upper = lower.toUpperCase();
s = s.split(pascal).join('Atlas');
s = s.split(`~/.${lower}`).join('~/.atlas');
s = s.split(`${lower}.db`).join('atlas.db');
s = s.split(upper).join('ATLAS');
s = s.split(`${lower}_`).join('atlas_');
s = s.split(`${lower}-`).join('atlas-');
s = s.split(`${lower}:`).join('atlas:');
s = s.split(lower).join('atlas');
fs.writeFileSync(path, s);
NODE
```

Expected: the file still describes the generic local Agent App design, but all product identity strings are Atlas.

- [ ] **Step 4: Convert the historical implementation plan mechanically**

Run this exact script:

```bash
node <<'NODE'
const fs = require('fs');
const path = 'docs/superpowers/plans/2026-06-12-agent-app-decoding.md';
let s = fs.readFileSync(path, 'utf8');
const lower = ['w','e','f','t'].join('');
const pascal = 'W' + lower.slice(1);
const upper = lower.toUpperCase();
s = s.split(pascal).join('Atlas');
s = s.split(`~/.${lower}`).join('~/.atlas');
s = s.split(`${lower}.db`).join('atlas.db');
s = s.split(`${lower}_app_lib`).join('atlas_app_lib');
s = s.split(`${lower}_home`).join('atlas_home');
s = s.split(`${lower}_bus`).join('atlas_bus');
s = s.split(upper).join('ATLAS');
s = s.split(`${lower}_`).join('atlas_');
s = s.split(`${lower}-`).join('atlas-');
s = s.split(`${lower}:`).join('atlas:');
s = s.split(`/tmp/${lower}`).join('/tmp/atlas');
s = s.split(lower).join('atlas');
fs.writeFileSync(path, s);
NODE
```

Expected: code snippets in the historical plan consistently use `atlas_home`, `ATLAS_HOME`, `atlas_app_lib`, and `atlas_bus`.

- [ ] **Step 5: Verify ignored-doc scan is clean**

Run:

```bash
LOWER="$(printf '%s%s%s%s' w e f t)"
PASCAL="$(printf '%s%s' W "${LOWER#?}")"
UPPER="$(printf '%s' "$LOWER" | tr '[:lower:]' '[:upper:]')"
OLD_IDENTITY_PATTERN="\\b${PASCAL}\\b|\\b${LOWER}\\b|${UPPER}|com\\.${LOWER}|${LOWER}\\.db|\\.${LOWER}|${LOWER}-|${LOWER}_|${LOWER}:"
rg --no-ignore -n "$OLD_IDENTITY_PATTERN" docs/superpowers \
  -g '!*.png' \
  -g '!*.ico' \
  -g '!*.icns'
```

Expected: no output and exit code `1`.

- [ ] **Step 6: Commit the docs conversion and this plan**

Run:

```bash
git add -f docs/superpowers/specs/2026-06-12-agent-app-decoding-design.md \
  docs/superpowers/plans/2026-06-12-agent-app-decoding.md \
  docs/superpowers/plans/2026-06-15-atlas-product-identity-migration.md
git diff --cached --name-only
git commit -m "docs: align planning docs with Atlas identity"
```

Expected: staged files are exactly the three docs listed above; commit succeeds.

---

### Task 2: Verify Product, Package, Runtime, And Data Identity

**Files:**
- Modify/Audit: `package.json`
- Modify/Audit: `index.html`
- Modify/Audit: `src-tauri/Cargo.toml`
- Modify/Audit: `src-tauri/Cargo.lock`
- Modify/Audit: `src-tauri/tauri.conf.json`
- Modify/Audit: `src-tauri/src/paths.rs`
- Modify/Audit: `src-tauri/src/config.rs`
- Modify/Audit: `src-tauri/src/store/key.rs`
- Modify/Audit: `src-tauri/src/power.rs`
- Modify/Audit: `src-tauri/tests/db_encryption.rs`
- Modify/Audit: `src-tauri/tests/ensure_default_workspace.rs`

- [ ] **Step 1: Check required Atlas runtime markers**

Run:

```bash
rg --no-ignore -n "Atlas|atlas|ATLAS|com\\.jingchen\\.atlas|atlas\\.db|~/.atlas" \
  package.json index.html src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json \
  src-tauri/src/paths.rs src-tauri/src/config.rs src-tauri/src/store/key.rs src-tauri/src/power.rs \
  src-tauri/tests/db_encryption.rs src-tauri/tests/ensure_default_workspace.rs
```

Expected: output includes `package.json`, `index.html`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/src/paths.rs`, `src-tauri/src/config.rs`, `src-tauri/src/store/key.rs`, and tests using `ATLAS_*`, `atlas.db`, and `com.jingchen.atlas`.

- [ ] **Step 2: Check old runtime markers are absent**

Run:

```bash
LOWER="$(printf '%s%s%s%s' w e f t)"
PASCAL="$(printf '%s%s' W "${LOWER#?}")"
UPPER="$(printf '%s' "$LOWER" | tr '[:lower:]' '[:upper:]')"
OLD_IDENTITY_PATTERN="\\b${PASCAL}\\b|\\b${LOWER}\\b|${UPPER}|com\\.${LOWER}|${LOWER}\\.db|\\.${LOWER}|${LOWER}-|${LOWER}_|${LOWER}:"
rg --no-ignore -n "$OLD_IDENTITY_PATTERN" \
  package.json index.html src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json \
  src-tauri/src/paths.rs src-tauri/src/config.rs src-tauri/src/store/key.rs src-tauri/src/power.rs \
  src-tauri/tests/db_encryption.rs src-tauri/tests/ensure_default_workspace.rs
```

Expected: no output and exit code `1`.

- [ ] **Step 3: Patch runtime mismatches without compatibility**

If Step 2 prints matches, replace them exactly by category:

```text
Display product name -> Atlas
Lowercase product prefix -> atlas
Uppercase environment prefix -> ATLAS
Home directory -> ~/.atlas
Database filename -> atlas.db
Reverse domain -> com.jingchen.atlas
Rust package name -> atlas-app
Rust library crate -> atlas_app_lib
Theme storage key -> atlas-theme
```

Do not add fallback reads for the previous home directory, previous database, previous env var prefix, or previous crate name.

- [ ] **Step 4: Run runtime-focused tests as separate commands**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml paths::tests --lib
cargo test --manifest-path src-tauri/Cargo.toml store::key::tests --lib
cargo test --manifest-path src-tauri/Cargo.toml --test db_encryption
cargo test --manifest-path src-tauri/Cargo.toml --test ensure_default_workspace
```

Expected: each command exits `0`.

- [ ] **Step 5: Commit runtime identity fixes**

Run:

```bash
git add package.json index.html src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json \
  src-tauri/src/paths.rs src-tauri/src/config.rs src-tauri/src/store/key.rs src-tauri/src/power.rs \
  src-tauri/tests/db_encryption.rs src-tauri/tests/ensure_default_workspace.rs
git diff --cached --name-only
git commit -m "chore: migrate runtime identity to Atlas"
```

Expected: commit succeeds if files changed. If all listed files were already correct and no files are staged, record `runtime identity already clean` in execution notes and continue.

---

### Task 3: Verify Backend Source, Backup, Store, IM, And Provider Identity

**Files:**
- Modify/Audit: `src-tauri/src/adapters/mod.rs`
- Modify/Audit: `src-tauri/src/backup/git_remote.rs`
- Modify/Audit: `src-tauri/src/backup/mod.rs`
- Modify/Audit: `src-tauri/src/backup/recovery_key.rs`
- Modify/Audit: `src-tauri/src/backup/scheduler.rs`
- Modify/Audit: `src-tauri/src/backup/snapshot.rs`
- Modify/Audit: `src-tauri/src/brief.rs`
- Modify/Audit: `src-tauri/src/check.rs`
- Modify/Audit: `src-tauri/src/claude.rs`
- Modify/Audit: `src-tauri/src/codex.rs`
- Modify/Audit: `src-tauri/src/codex_app_server.rs`
- Modify/Audit: `src-tauri/src/commands.rs`
- Modify/Audit: `src-tauri/src/detect.rs`
- Modify/Audit: `src-tauri/src/gc.rs`
- Modify/Audit: `src-tauri/src/git.rs`
- Modify/Audit: `src-tauri/src/im/feishu/ws.rs`
- Modify/Audit: `src-tauri/src/im/inbound.rs`
- Modify/Audit: `src-tauri/src/im/mod.rs`
- Modify/Audit: `src-tauri/src/im/outbound.rs`
- Modify/Audit: `src-tauri/src/lib.rs`
- Modify/Audit: `src-tauri/src/main.rs`
- Modify/Audit: `src-tauri/src/materialize.rs`
- Modify/Audit: `src-tauri/src/profile.rs`
- Modify/Audit: `src-tauri/src/sidecar.rs`
- Modify/Audit: `src-tauri/src/store/entities/direction.rs`
- Modify/Audit: `src-tauri/src/store/entities/skill_source.rs`
- Modify/Audit: `src-tauri/src/store/legacy.rs`
- Modify/Audit: `src-tauri/src/store/mod.rs`
- Modify/Audit: `src-tauri/src/store/repo.rs`
- Modify/Audit: `src-tauri/tests/backup_end_to_end.rs`
- Modify/Audit: `src-tauri/tests/backup_git_remote.rs`
- Modify/Audit: `src-tauri/tests/backup_recovery.rs`
- Modify/Audit: `src-tauri/tests/backup_scheduler.rs`
- Modify/Audit: `src-tauri/tests/backup_snapshot.rs`
- Modify/Audit: `src-tauri/tests/im_bridge.rs`
- Modify/Audit: `src-tauri/tests/m2_git.rs`
- Modify/Audit: `src-tauri/tests/m2_worktree.rs`

- [ ] **Step 1: Generate backend touched-file manifest**

Run:

```bash
git status --short \
  | sed 's/^...//' \
  | sed 's/ -> /\n/' \
  | rg '^src-tauri/(src|tests)/' \
  | sort -u > /tmp/atlas_backend_touched.txt
cat /tmp/atlas_backend_touched.txt
```

Expected: output includes backend source and test files touched by the Atlas migration.

- [ ] **Step 2: Check old identity is absent from every touched backend file**

Run:

```bash
LOWER="$(printf '%s%s%s%s' w e f t)"
PASCAL="$(printf '%s%s' W "${LOWER#?}")"
UPPER="$(printf '%s' "$LOWER" | tr '[:lower:]' '[:upper:]')"
OLD_IDENTITY_PATTERN="\\b${PASCAL}\\b|\\b${LOWER}\\b|${UPPER}|com\\.${LOWER}|${LOWER}\\.db|\\.${LOWER}|${LOWER}-|${LOWER}_|${LOWER}:"
xargs rg --no-ignore -n "$OLD_IDENTITY_PATTERN" < /tmp/atlas_backend_touched.txt
```

Expected: no output and exit code `123` if `xargs` receives no matches from `rg`, or exit code `1` if run manually per file. Any printed match is a failure.

- [ ] **Step 3: Check Atlas-positive backend markers**

Run:

```bash
rg --no-ignore -n "Atlas|atlas|ATLAS|atlas_app_lib|atlas.db|~/.atlas|com\\.jingchen\\.atlas" \
  src-tauri/src src-tauri/tests \
  -g '!target'
```

Expected: output includes runtime, backup, provider, store, and tests. It is acceptable for files not directly related to product identity to have no Atlas marker, as long as Step 2 is clean.

- [ ] **Step 4: Patch backend mismatches**

If Step 2 prints matches, use this replacement table:

```text
Display product name -> Atlas
Lowercase symbol/file prefix -> atlas
Uppercase env prefix -> ATLAS
Rust library import -> atlas_app_lib
Temp test path prefix -> atlas
Backup label/prefix -> Atlas / atlas
IM bridge product label -> Atlas
Provider sidecar product label -> Atlas
Store legacy product label -> Atlas
```

Do not rename unrelated domain terms such as `thread`, `direction`, `repo`, or `worktree` unless the string is specifically part of the product identity.

- [ ] **Step 5: Run focused backend test groups**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test backup_end_to_end
cargo test --manifest-path src-tauri/Cargo.toml --test backup_git_remote
cargo test --manifest-path src-tauri/Cargo.toml --test backup_recovery
cargo test --manifest-path src-tauri/Cargo.toml --test backup_scheduler
cargo test --manifest-path src-tauri/Cargo.toml --test backup_snapshot
cargo test --manifest-path src-tauri/Cargo.toml --test im_bridge
cargo test --manifest-path src-tauri/Cargo.toml --test m2_git
cargo test --manifest-path src-tauri/Cargo.toml --test m2_worktree
cargo test --manifest-path src-tauri/Cargo.toml store::repo --lib
```

Expected: each command exits `0`.

- [ ] **Step 6: Commit backend identity fixes**

Run:

```bash
git add src-tauri/src/adapters/mod.rs src-tauri/src/backup src-tauri/src/brief.rs src-tauri/src/check.rs \
  src-tauri/src/claude.rs src-tauri/src/codex.rs src-tauri/src/codex_app_server.rs src-tauri/src/commands.rs \
  src-tauri/src/detect.rs src-tauri/src/gc.rs src-tauri/src/git.rs src-tauri/src/im src-tauri/src/lib.rs \
  src-tauri/src/main.rs src-tauri/src/materialize.rs src-tauri/src/profile.rs src-tauri/src/sidecar.rs \
  src-tauri/src/store src-tauri/tests/backup_end_to_end.rs src-tauri/tests/backup_git_remote.rs \
  src-tauri/tests/backup_recovery.rs src-tauri/tests/backup_scheduler.rs src-tauri/tests/backup_snapshot.rs \
  src-tauri/tests/im_bridge.rs src-tauri/tests/m2_git.rs src-tauri/tests/m2_worktree.rs
git diff --cached --name-only
git commit -m "chore: migrate backend identity to Atlas"
```

Expected: commit succeeds if files changed. If no files are staged, record `backend identity already clean` and continue.

---

### Task 4: Verify Agent Protocol, MCP, Hook, Sentinel, And Skill Identity

**Files:**
- Modify/Audit: `src-tauri/src/ask.rs`
- Modify/Audit: `src-tauri/src/bus/global.rs`
- Modify/Audit: `src-tauri/src/bus/inject.rs`
- Modify/Audit: `src-tauri/src/bus/server.rs`
- Modify/Audit: `src-tauri/src/planner.rs`
- Modify/Audit: `src-tauri/src/opencode.rs`
- Modify/Audit: `src-tauri/src/lead_chat/commands.rs`
- Modify/Audit: `src-tauri/src/lead_chat/engine.rs`
- Modify/Audit: `src-tauri/src/lead_chat/mod.rs`
- Modify/Audit: `src-tauri/src/lead_chat/proto.rs`
- Modify/Audit: `src-tauri/src/lead_chat/repo_state.rs`
- Modify/Audit: `src-tauri/src/lead_chat/sentinels.rs`
- Modify/Audit: `src-tauri/src/skills/inject.rs`
- Modify/Audit: `src-tauri/src/skills/mod.rs`
- Modify/Audit: `src-tauri/src/skills/parse.rs`
- Modify/Audit: `src-tauri/src/skills/sync.rs`
- Modify/Audit: `src-tauri/tests/bus_http.rs`
- Modify/Audit: `src-tauri/tests/lead_prompt.rs`
- Modify/Audit: `src-tauri/tests/lead_repo_state.rs`
- Modify/Audit: `src-tauri/tests/lead_sentinels.rs`

- [ ] **Step 1: Check required Atlas protocol markers**

Run:

```bash
rg --no-ignore -n "atlas_bus|atlas_planner|atlas_global|atlas-ask|atlas:|atlas-app|layer_atlas|Atlas" \
  src-tauri/src/ask.rs src-tauri/src/bus src-tauri/src/planner.rs src-tauri/src/opencode.rs \
  src-tauri/src/lead_chat src-tauri/src/skills src-tauri/tests/bus_http.rs \
  src-tauri/tests/lead_prompt.rs src-tauri/tests/lead_repo_state.rs src-tauri/tests/lead_sentinels.rs
```

Expected: output includes MCP names, generated config filenames, sentinel namespace, skill layer names, prompt text, and tests.

- [ ] **Step 2: Check old protocol markers are absent**

Run:

```bash
LOWER="$(printf '%s%s%s%s' w e f t)"
PASCAL="$(printf '%s%s' W "${LOWER#?}")"
UPPER="$(printf '%s' "$LOWER" | tr '[:lower:]' '[:upper:]')"
OLD_IDENTITY_PATTERN="\\b${PASCAL}\\b|\\b${LOWER}\\b|${UPPER}|com\\.${LOWER}|${LOWER}\\.db|\\.${LOWER}|${LOWER}-|${LOWER}_|${LOWER}:"
rg --no-ignore -n "$OLD_IDENTITY_PATTERN" \
  src-tauri/src/ask.rs src-tauri/src/bus src-tauri/src/planner.rs src-tauri/src/opencode.rs \
  src-tauri/src/lead_chat src-tauri/src/skills src-tauri/tests/bus_http.rs \
  src-tauri/tests/lead_prompt.rs src-tauri/tests/lead_repo_state.rs src-tauri/tests/lead_sentinels.rs
```

Expected: no output and exit code `1`.

- [ ] **Step 3: Patch protocol mismatches without aliases**

If Step 2 prints matches, use this replacement table:

```text
Bus server prefix -> atlas_bus
Planner server prefix -> atlas_planner
Global server prefix -> atlas_global
Sentinel namespace -> atlas
Ask hook filename prefix -> atlas-ask
OpenCode plugin filename prefix -> atlas-ask
Skill layer prefix -> layer_atlas
Injected app label -> Atlas
```

Keep parser behavior strict. Do not accept previous sentinel names, MCP server names, or hook filenames as compatibility aliases.

- [ ] **Step 4: Run focused protocol tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test lead_sentinels
cargo test --manifest-path src-tauri/Cargo.toml --test bus_http
cargo test --manifest-path src-tauri/Cargo.toml --test lead_prompt
cargo test --manifest-path src-tauri/Cargo.toml --test lead_repo_state
cargo test --manifest-path src-tauri/Cargo.toml lead_chat::sentinels --lib
```

Expected: each command exits `0`.

- [ ] **Step 5: Commit protocol identity fixes**

Run:

```bash
git add src-tauri/src/ask.rs src-tauri/src/bus src-tauri/src/planner.rs src-tauri/src/opencode.rs \
  src-tauri/src/lead_chat src-tauri/src/skills src-tauri/tests/bus_http.rs \
  src-tauri/tests/lead_prompt.rs src-tauri/tests/lead_repo_state.rs src-tauri/tests/lead_sentinels.rs
git diff --cached --name-only
git commit -m "chore: migrate agent protocol identity to Atlas"
```

Expected: commit succeeds if files changed. If no files are staged, record `agent protocol identity already clean` and continue.

---

### Task 5: Verify Frontend UI, State, Events, And User-Facing Copy

**Files:**
- Modify/Audit: `src/App.tsx`
- Modify/Audit: `src/board/RepoGraph.tsx`
- Modify/Audit: `src/board/ThreadBoard.tsx`
- Modify/Audit: `src/board/WorkspaceKanban.tsx`
- Modify/Audit: `src/components/CommandPalette.tsx`
- Modify/Audit: `src/components/EffectiveConfigDialog.tsx`
- Modify/Audit: `src/components/Inspect.tsx`
- Modify/Audit: `src/components/Markdown.tsx`
- Modify/Audit: `src/components/ui/Dialog.tsx`
- Modify/Audit: `src/components/ui/Select.tsx`
- Modify/Audit: `src/components/ui/StatusChip.tsx`
- Modify/Audit: `src/i18n/en.ts`
- Modify/Audit: `src/i18n/index.ts`
- Modify/Audit: `src/i18n/zh.ts`
- Modify/Audit: `src/index.css`
- Modify/Audit: `src/lib/api.ts`
- Modify/Audit: `src/lib/resume.ts`
- Modify/Audit: `src/nav/AppTopBar.tsx`
- Modify/Audit: `src/nav/WorkspaceNav.tsx`
- Modify/Audit: `src/session/ChatTimeline.tsx`
- Modify/Audit: `src/session/DiffPanel.tsx`
- Modify/Audit: `src/session/LeadTab.tsx`
- Modify/Audit: `src/session/ObserveView.tsx`
- Modify/Audit: `src/session/SessionView.tsx`
- Modify/Audit: `src/session/transcriptBits.ts`
- Modify/Audit: `src/settings/Backup.tsx`
- Modify/Audit: `src/state/shortcuts.ts`
- Modify/Audit: `src/state/store.tsx`
- Modify/Audit: `src/state/theme.ts`

- [ ] **Step 1: Generate frontend touched-file manifest**

Run:

```bash
git status --short \
  | sed 's/^...//' \
  | sed 's/ -> /\n/' \
  | rg '^src/' \
  | sort -u > /tmp/atlas_frontend_touched.txt
cat /tmp/atlas_frontend_touched.txt
```

Expected: output includes frontend files touched by the Atlas migration.

- [ ] **Step 2: Check required Atlas UI markers**

Run:

```bash
rg --no-ignore -n "Atlas|atlas|ATLAS|atlas-theme|atlas:open-palette|layer_atlas" \
  src/App.tsx src/board src/components src/i18n src/index.css src/lib src/nav src/session src/settings src/state
```

Expected: output includes i18n strings, CSS/event/localStorage prefixes, command palette text, settings text, and transcript/session rendering where product identity is relevant.

- [ ] **Step 3: Check old UI markers are absent from all touched frontend files**

Run:

```bash
LOWER="$(printf '%s%s%s%s' w e f t)"
PASCAL="$(printf '%s%s' W "${LOWER#?}")"
UPPER="$(printf '%s' "$LOWER" | tr '[:lower:]' '[:upper:]')"
OLD_IDENTITY_PATTERN="\\b${PASCAL}\\b|\\b${LOWER}\\b|${UPPER}|com\\.${LOWER}|${LOWER}\\.db|\\.${LOWER}|${LOWER}-|${LOWER}_|${LOWER}:"
xargs rg --no-ignore -n "$OLD_IDENTITY_PATTERN" < /tmp/atlas_frontend_touched.txt
```

Expected: no output. Any printed match is a failure.

- [ ] **Step 4: Patch frontend mismatches**

If Step 3 prints matches, use this replacement table:

```text
Display product name -> Atlas
Lowercase CSS prefix -> atlas
Lowercase localStorage key prefix -> atlas
Lowercase custom event prefix -> atlas
Skill layer UI key prefix -> layer_atlas
Public asset reference -> /atlas-icon.png or /atlas-mark.png
```

Keep app functionality unchanged; do not rename task/run/domain concepts unrelated to product identity.

- [ ] **Step 5: Run frontend build**

Run:

```bash
pnpm build
```

Expected: exit code `0`. A Vite large chunk warning is acceptable if the build succeeds.

- [ ] **Step 6: Commit frontend identity fixes**

Run:

```bash
git add src/App.tsx src/board src/components src/i18n src/index.css src/lib src/nav src/session src/settings src/state index.html
git diff --cached --name-only
git commit -m "chore: migrate frontend identity to Atlas"
```

Expected: commit succeeds if files changed. If no files are staged, record `frontend identity already clean` and continue.

---

### Task 6: Verify Documentation, Diagrams, Public Assets, And Desktop Icons

**Files:**
- Modify/Audit: `AGENTS.md`
- Modify/Audit: `ARCHITECTURE.md`
- Modify/Audit: `DESIGN.md`
- Modify/Audit: `PRODUCT.md`
- Modify/Audit: `README.md`
- Modify/Audit: `README.zh-CN.md`
- Modify/Audit: `assets/diagrams/arch-en.svg`
- Modify/Audit: `assets/diagrams/arch-zh.svg`
- Modify/Audit: `assets/diagrams/board-en.svg`
- Modify/Audit: `assets/diagrams/board-zh.svg`
- Modify/Audit: `assets/diagrams/im-en.svg`
- Modify/Audit: `assets/diagrams/im-zh.svg`
- Delete: README overview image whose filename uses the previous lowercase product prefix.
- Create/Audit: `assets/readme/atlas-overview.png`
- Create/Audit: `assets/brand/atlas-icon-embedded.png`
- Create/Audit: `assets/brand/atlas-icon-master.png`
- Create/Audit: `assets/brand/atlas-icon-source.svg`
- Delete: public SVG files whose filenames use the previous lowercase product prefix.
- Create/Audit: `public/atlas-icon.png`
- Create/Audit: `public/atlas-mark.png`
- Modify/Audit: every file under `src-tauri/icons/`

- [ ] **Step 1: Check required Atlas doc and asset markers**

Run:

```bash
rg --no-ignore -n "Atlas|atlas|atlas-icon|atlas-mark|atlas-overview|com\\.jingchen\\.atlas" \
  AGENTS.md ARCHITECTURE.md DESIGN.md PRODUCT.md README.md README.zh-CN.md assets/diagrams index.html src-tauri/tauri.conf.json
```

Expected: output includes product docs, diagram source text, README image references, entrypoint asset references, and Tauri metadata.

- [ ] **Step 2: Check old doc markers are absent**

Run:

```bash
LOWER="$(printf '%s%s%s%s' w e f t)"
PASCAL="$(printf '%s%s' W "${LOWER#?}")"
UPPER="$(printf '%s' "$LOWER" | tr '[:lower:]' '[:upper:]')"
OLD_IDENTITY_PATTERN="\\b${PASCAL}\\b|\\b${LOWER}\\b|${UPPER}|com\\.${LOWER}|${LOWER}\\.db|\\.${LOWER}|${LOWER}-|${LOWER}_|${LOWER}:"
rg --no-ignore -n "$OLD_IDENTITY_PATTERN" \
  AGENTS.md ARCHITECTURE.md DESIGN.md PRODUCT.md README.md README.zh-CN.md assets/diagrams index.html src-tauri/tauri.conf.json
```

Expected: no output and exit code `1`.

- [ ] **Step 3: Check required assets exist and old asset filenames are removed**

Run:

```bash
test -f assets/readme/atlas-overview.png
test -f assets/brand/atlas-icon-embedded.png
test -f assets/brand/atlas-icon-master.png
test -f assets/brand/atlas-icon-source.svg
test -f public/atlas-icon.png
test -f public/atlas-mark.png
PREVIOUS_LOWER="$(printf '%s%s%s%s' w e f t)"
test ! -e "public/${PREVIOUS_LOWER}-icon.svg"
test ! -e "public/${PREVIOUS_LOWER}-logo.svg"
test ! -e "public/${PREVIOUS_LOWER}-mark.svg"
test ! -e "assets/readme/${PREVIOUS_LOWER}-overview.png"
```

Expected: every command exits `0`.

- [ ] **Step 4: Check old asset filenames are absent**

Run:

```bash
LOWER="$(printf '%s%s%s%s' w e f t)"
PASCAL="$(printf '%s%s' W "${LOWER#?}")"
UPPER="$(printf '%s' "$LOWER" | tr '[:lower:]' '[:upper:]')"
find assets public src-tauri/icons \
  \( -iname "*${LOWER}*" -o -iname "*${PASCAL}*" -o -iname "*${UPPER}*" \) -print | sort
```

Expected: no output.

- [ ] **Step 5: Check desktop icon files are non-empty**

Run:

```bash
find src-tauri/icons -type f \( -name '*.png' -o -name '*.ico' -o -name '*.icns' \) -print0 \
  | xargs -0 file
find src-tauri/icons -type f \( -name '*.png' -o -name '*.ico' -o -name '*.icns' \) -size 0 -print
```

Expected: `file` identifies image/icon formats; the zero-size scan prints no output.

- [ ] **Step 6: Patch docs/assets mismatches**

If Steps 2-5 fail, use this replacement table:

```text
Display product name -> Atlas
Lowercase asset prefix -> atlas
README screenshot path -> assets/readme/atlas-overview.png
Browser icon path -> /atlas-icon.png
Browser mark path -> /atlas-mark.png
Bundle identifier -> com.jingchen.atlas
Updater release path -> atlas
```

Do not keep deleted old public SVGs or the old README image.

- [ ] **Step 7: Commit docs and assets fixes**

Run:

```bash
git add AGENTS.md ARCHITECTURE.md DESIGN.md PRODUCT.md README.md README.zh-CN.md \
  assets public index.html src-tauri/tauri.conf.json src-tauri/icons
git diff --cached --name-only
git commit -m "chore: migrate docs and assets to Atlas"
```

Expected: commit succeeds if files changed. If no files are staged, record `docs and assets already clean` and continue.

---

### Task 7: Run Full Automated Verification

**Files:**
- Verify: entire repository except `.git`, `node_modules`, `src-tauri/target`, and `build`

- [ ] **Step 1: Run full source old-identity scan**

Run the full source scan from Shared Verification Helpers.

Expected: no output and exit code `1`.

- [ ] **Step 2: Run full filename old-identity scan**

Run the full filename scan from Shared Verification Helpers.

Expected: no output and exit code `0`.

- [ ] **Step 3: Run TypeScript build**

Run:

```bash
pnpm build
```

Expected: exit code `0`. A Vite large chunk warning is acceptable if the build succeeds.

- [ ] **Step 4: Run full Rust test suite**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: exit code `0`.

- [ ] **Step 5: Run Rust compile check without executing tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --no-run
```

Expected: exit code `0`.

- [ ] **Step 6: Run whitespace check**

Run:

```bash
git diff --check
```

Expected: no output and exit code `0`.

- [ ] **Step 7: Commit verification-driven fixes**

Run:

```bash
git status --short
```

Expected: only Atlas migration files remain changed. If there are unstaged migration fixes from Tasks 2-7, stage and commit them:

```bash
git add AGENTS.md ARCHITECTURE.md DESIGN.md PRODUCT.md README.md README.zh-CN.md \
  assets docs index.html package.json public src src-tauri
git diff --cached --name-only
git commit -m "chore: complete Atlas product identity migration"
```

If there are no unstaged changes, record `no final migration fixes to commit` and continue.

---

### Task 8: Verify Desktop Shell, Build Bundle, And Entrypoint Assets

**Files:**
- Verify: `index.html`
- Verify: `package.json`
- Verify: `src-tauri/tauri.conf.json`
- Verify: `public/atlas-icon.png`
- Verify: `public/atlas-mark.png`
- Verify: generated Tauri bundle metadata under `src-tauri/target/`

- [ ] **Step 1: Run Tauri build**

Run:

```bash
pnpm tauri build
```

Expected: exit code `0`. If macOS signing/notarization is unavailable, the failure must explicitly be a signing/notarization environment failure after compilation and bundling has progressed. Product identity failures, missing icons, missing bundle identifier, or frontend build failures are blocking failures.

- [ ] **Step 2: Inspect generated bundle metadata**

Run:

```bash
find src-tauri/target -path '*Atlas.app' -maxdepth 8 -print | head -20
find src-tauri/target -name Info.plist -path '*Atlas.app*' -print | head -5
```

Expected: output includes an `Atlas.app` path and an `Info.plist` inside it on macOS builds.

- [ ] **Step 3: Start Vite dev server for static entrypoint checks**

Run:

```bash
pnpm dev --host 127.0.0.1
```

Expected: server prints a local URL using port `1420` unless that port is occupied.

- [ ] **Step 4: Check static Atlas assets**

In a second terminal, run:

```bash
curl -I -s http://127.0.0.1:1420/atlas-icon.png
curl -I -s http://127.0.0.1:1420/atlas-mark.png
```

Expected: both responses include `HTTP/1.1 200 OK` and `Content-Type: image/png`.

- [ ] **Step 5: Check browser document identity**

Open `http://127.0.0.1:1420/` in an observable browser and evaluate:

```javascript
({
  title: document.title,
  icon: document.querySelector('link[rel~="icon"]')?.getAttribute('href') ?? null,
  visibleHasAtlas: document.body.innerText.includes('Atlas'),
})
```

Expected:

```json
{
  "title": "Atlas",
  "icon": "/atlas-icon.png"
}
```

`visibleHasAtlas` may be `false` on loading screens that do not show brand text. Title and icon are required.

- [ ] **Step 6: Stop Vite dev server**

Stop the dev server with `Ctrl-C`.

Expected: the server exits and no long-running development process remains.

- [ ] **Step 7: Record browser limitation**

If a normal browser console shows Tauri `invoke` or `listen` errors, record this exact note in the execution summary:

```text
Browser-only Vite checks confirm title and static assets. Tauri invoke/listen errors in a normal browser are expected because the Tauri shell is not present.
```

Do not treat those browser-only Tauri API errors as Atlas migration failures.

---

### Task 9: Final Coverage Review, Git State, And User Handoff

**Files:**
- Verify: `docs/superpowers/specs/2026-06-15-atlas-product-identity-migration-design.md`
- Verify: `docs/superpowers/plans/2026-06-15-atlas-product-identity-migration.md`
- Verify: full git worktree

- [ ] **Step 1: Confirm spec-to-task coverage**

Run:

```bash
sed -n '1,240p' docs/superpowers/specs/2026-06-15-atlas-product-identity-migration-design.md
sed -n '1,760p' docs/superpowers/plans/2026-06-15-atlas-product-identity-migration.md
```

Expected coverage mapping:

```text
Product display -> Tasks 5, 6, 8
Code identity -> Tasks 2, 3, 4, 7
Runtime identity -> Tasks 2, 3, 7, 8
Agent/protocol identity -> Task 4 and Task 7
Packaging/release identity -> Tasks 2, 6, 8
Docs/assets identity -> Tasks 1, 6, 7
No old data compatibility -> Tasks 2, 3, 7
```

- [ ] **Step 2: Confirm every touched migration file is covered**

Run:

```bash
git status --short \
  | sed 's/^...//' \
  | sed 's/ -> /\n/' \
  | sort -u > /tmp/atlas_changed_files_final.txt
while IFS= read -r file; do
  case "$file" in
    docs/superpowers/*|package.json|index.html|src-tauri/*|src/*|assets/*|public/*|AGENTS.md|ARCHITECTURE.md|DESIGN.md|PRODUCT.md|README.md|README.zh-CN.md)
      ;;
    *)
      printf 'Unclassified changed file: %s\n' "$file"
      ;;
  esac
done < /tmp/atlas_changed_files_final.txt
```

Expected: no `Unclassified changed file` output. If output appears, classify that file in this plan or explicitly exclude it as unrelated user work before finalizing.

- [ ] **Step 3: Confirm final git state**

Run:

```bash
git status --short
```

Expected: no unstaged Atlas migration changes remain after commits. If unrelated user changes exist, list them separately and do not stage them.

- [ ] **Step 4: Prepare final user summary**

Include these sections:

```text
完成内容
验证结果
数据断开策略
残余限制
提交记录
```

The data断开策略 section must state in Chinese:

```text
Atlas 不读取、不迁移旧 home 目录、旧数据库、旧环境变量或旧协议名。
```

The residual limitations section must include any command that could not run, any Tauri build limitation, and any browser-only Tauri API limitation.
