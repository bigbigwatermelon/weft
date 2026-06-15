# Repository Guidelines

## Project Structure & Module Organization

Atlas is a Tauri v2 desktop app with a React frontend and Rust backend.

- `src/`: React + TypeScript UI. Key areas: `board/` for workspace/issue boards, `session/` for chat/observe/diff surfaces, `components/` for shared UI, `i18n/` for English/Chinese strings.
- `src-tauri/src/`: Rust backend. Key modules: `lead_chat/` for headless agent sessions, `store/` for SQLite/SeaORM entities and migrations, `bus/` for local MCP/thread bus, `git.rs` and `materialize.rs` for worktree handling.
- `src-tauri/tests/`: Rust integration tests.
- `assets/`, `public/`: screenshots, icons, and generated diagrams.

## Build, Test, and Development Commands

- `pnpm install`: install frontend dependencies.
- `pnpm dev`: run Vite for frontend-only iteration.
- `pnpm build`: run TypeScript checking and create the production frontend bundle.
- `pnpm preflight:quick`: run the fast local pre-PR gate.
- `pnpm preflight`: run the full local pre-PR gate; the pre-push hook runs this automatically.
- `pnpm tauri dev`: run the full desktop app in development mode.
- `pnpm tauri build`: build a release app bundle.
- `cd src-tauri && cargo test`: run Rust unit and integration tests.
- `git diff --check`: check patches for whitespace errors before committing.

## Coding Style & Naming Conventions

Use TypeScript for frontend code and Rust 2021 for backend code. Keep modules focused and follow the existing directory boundaries. Component files use `PascalCase.tsx`; helpers and state modules use lower camel or kebab style already present in the folder. User-facing strings must go through `src/i18n/en.ts` and `src/i18n/zh.ts`.

Rust production paths deny `unwrap`, `expect`, and `panic`; return `Result` and surface errors clearly. Avoid adding embedded terminal/TUI dependencies; Atlas renders its own chat UI and uses terminal takeover only as an escape hatch.

## Testing Guidelines

Backend logic is covered with Rust unit tests next to modules and integration tests under `src-tauri/tests/`. Add tests for store migrations, worktree behavior, chat protocol parsing, planner scope, bus behavior, and verification logic when those areas change. Frontend changes should at minimum pass `npm run build`.

## Commit & Pull Request Guidelines

History uses short conventional prefixes such as `feat(plan): ...`, `fix(store): ...`, `polish(needs): ...`, and `chore: ...`. Keep commits scoped and descriptive.

PRs should include a concise summary, verification commands and results, linked issue/task when applicable, and screenshots or short recordings for visible UI changes.

## Local-First PR Workflow

Mirror the local-first flow used in the RedditFind project:

- Treat `pnpm preflight` as the default PR gate before pushing. It runs whitespace checks, Atlas identity checks, the frontend build, and Rust tests.
- The pre-push hook runs `pnpm ci:pre-push`, which delegates to `pnpm preflight`. Do not bypass it with `--no-verify`; if it fails, diagnose the failure and fix the real issue.
- `pnpm preflight:quick` is acceptable while iterating, but a final push should have a full `pnpm preflight` result unless the user explicitly chooses a narrower validation.
- GitHub `CI` is a manual cross-platform fallback via `workflow_dispatch`; it is not part of the default PR loop. Run it only when a platform-specific risk, release decision, or user request needs remote Linux/macOS/Windows confirmation.
- When creating or updating a PR, record the PR URL, number, base branch, head branch, head commit, draft state, and local validation commands/results.
- The PR body should include `Summary`, `Root Cause / Why`, and `Validation`.

## Merge Workflow

Before merging a PR:

- Confirm the PR is open, non-draft, mergeable, and targeting the intended base branch.
- Confirm the latest pushed head has local `pnpm preflight` evidence or an explicitly accepted narrower validation.
- Confirm there are no unresolved actionable review threads, issue comments, review bodies, or failed required checks.
- For Codex review, accept either a latest-head Good/pass/no-suggestions comment or an existing PR-level `THUMBS_UP` from `chatgpt-codex-connector[bot]` when there are no newer actionable Codex comments or unresolved threads after the latest push. Do not block forever waiting for a duplicate PR-level reaction, because GitHub reactions are singleton per user per PR.
- Default to squash merge with `--match-head-commit` when using `gh pr merge`; delete the remote head branch only when it is not needed by another open or stacked PR.

After merge:

- Re-read the PR and confirm it is `MERGED`, recording the merge commit.
- Delete or stop the PR monitor.
- Fetch `origin main` / the repository default base and fast-forward local `main` only when the worktree is clean and the branch has not diverged.
- Clean local PR branches or worktrees only after listing their path, branch, commit, and dirty/untracked status; skip anything dirty.

## Fix And Review Workflow

When a PR review or check reports a problem:

- Read review threads in a thread-aware way, including resolved/outdated state, inline path/line, review bodies, issue comments, and reactions.
- Classify each item as true bug, missing validation, speculative, out-of-scope, duplicate gate, or cosmetic.
- Fix true bugs and missing validation with code plus focused tests. Push back clearly on speculative, out-of-scope, duplicate, or cosmetic feedback instead of expanding the PR.
- After a fix, run the narrow relevant check while iterating, then run `pnpm preflight` before the final push unless the user accepts a smaller gate.
- If manually triggered GitHub `CI` fails, inspect the GitHub Actions logs before changing code; do not treat a local guess as root cause.
- After pushing a fix, update or create a lightweight PR monitor that watches for new actionable review comments, unresolved threads, head changes, mergeability changes, and failed checks. The monitor should not wait for duplicate Codex reactions once a valid PR-level approval exists and no newer actionable feedback appears.

## Review Guidelines

Codex Cloud automatic code review is configured through the GitHub App / Codex settings for this repository. Do not add a separate API-key-backed Codex GitHub Action unless the repository owner explicitly asks for a self-hosted review bot.

When Codex reviews a pull request, review the PR diff against its base branch and focus on correctness, security, data safety, migration risk, regression risk, and missing tests. Lead with actionable findings ordered by severity and include exact file and line references where possible. Avoid style-only comments unless they mask a real bug.

For broad identity or architecture changes, check all coupled surfaces rather than a single page: app metadata, bundle identifiers, data directories, database names, environment variables, protocol names, user-facing strings, tests, docs, release assets, and recovery paths should move together unless the PR explicitly documents a transition bridge.

After a PR is opened or updated, expect `chatgpt-codex-connector` to review the latest head commit. If it leaves comments, address real defects with code and tests; push back clearly on out-of-scope or speculative feedback.

## Architecture & Configuration Notes

Do not write cross-repo wiring into canonical repositories. Use temporary launch flags, worktree-local ignored files, or Atlas-managed state. Current delivery reaches reviewable worktree diffs with pre-PR checks; PR creation, CI/CD observation, and deployment orchestration are roadmap work.
