// Mirrors the SeaORM models (serde serializes Rust field names as-is: snake_case).

export type Tool = "claude" | "codex" | "opencode" | "none";
export type ThreadKind = "feature" | "bugfix" | "refactor" | "spike";

export type ComputerUseStatusKind =
  | "disabled"
  | "unsupported_platform"
  | "missing"
  | "not_executable"
  | "found"
  | "doctor_failed"
  | "permission_missing"
  | "ready"
  | "unknown";

export interface ComputerUseStatus {
  enabled: boolean;
  supported: boolean;
  status: ComputerUseStatusKind;
  helper_path: string | null;
  helper_version: string | null;
  doctor_summary: string;
  error: string | null;
}

export interface Workspace {
  id: number;
  name: string;
  slug: string;
  created_at: string;
}

export interface Thread {
  id: number;
  workspace_id: number;
  title: string;
  slug: string;
  kind: string;
  created_at: string;
}

export type Task = Thread;

/** Normalized observe-mode transcript event (from the tool's own sidecar). */
export type NormEvent =
  | { kind: "message"; role: "user" | "assistant"; text: string; ts: string }
  | { kind: "tool"; name: string; summary: string; ts: string };

export interface Direction {
  id: number;
  thread_id: number;
  name: string;
  slug: string;
  tool: string;
  /** agent/human-driven lifecycle: queued | planning | working | done. */
  status: string;
  /** worker mandate: "plan+impl" (plans its direction first) | "impl-only". */
  mandate: string;
  created_at: string;
}

export type Run = Direction;

export interface SessionInfo {
  session_id: number;
  run_dir: string;
  cwd: string;
  tool: string;
  resumed: boolean;
  native_id: string | null;
}

/** Read-only snapshot backing the observe surface (mirrors Rust ObserveRef). */
export interface ObserveRef {
  run_dir: string;
  tool: string;
  session_id: number | null;
  native_id: string | null;
  status: string | null;
}

/** One row in a chat timeline (lead console / chat-mode workers). */
export interface LeadMessage {
  id: number;
  thread_id: number;
  session_id: number | null;
  turn_id: number;
  role: "user" | "assistant" | "system";
  kind:
    | "text"
    | "tool"
    | "command"
    | "approval"
    | "worker_event"
    | "meta"
    | "action_card";
  /** kind-shaped JSON string, e.g. {"text": "..."} for kind=text */
  content: string;
  status: "streaming" | "complete" | "interrupted" | "error" | "queued";
  created_at: string;
}

/** Incremental pushes on the `lead-chat` Tauri event (engine → UI). */
export type LeadChatPush =
  | { type: "message"; thread_id: number; message: LeadMessage }
  | { type: "delta"; thread_id: number; message_id: number; text: string }
  | { type: "finalize"; thread_id: number; message_id: number; status: string }
  | {
      type: "turn";
      thread_id: number;
      session_id: number | null;
      state: "busy" | "idle" | "stopped";
      queued: number;
    }
  | {
      type: "init";
      thread_id: number;
      session_id: number | null;
      native_id: string;
      slash_commands: SlashCmd[];
    }
  | {
      /** The tool call currently executing — transient, cleared by `turn`. */
      type: "activity";
      thread_id: number;
      session_id: number | null;
      name: string;
      summary: string;
    };

/** One slash command for the composer palette: the token plus whatever metadata
 *  the CLI reported (claude adds description + arg hint; opencode adds a
 *  description). `name` is the match + dispatch key. */
export interface SlashCmd {
  name: string;
  description?: string;
  arg_hint?: string;
}

/** One composer attachment heading to the engine (pasted or picked image). */
export interface ImageAttachment {
  media_type: string;
  /** base64 payload, no data-URI prefix. */
  data: string;
}

/** Snapshot of the lead engine, for mount-time hydration. */
export interface LeadStateInfo {
  state: "busy" | "idle" | "stopped";
  queued: number;
  native_id: string | null;
  slash_commands: SlashCmd[];
  cwd: string;
}

/** UI-side runtime status for a live session panel. */
export type SessionStatus = "running" | "idle" | "exited";

export interface BusMsg {
  from: string;
  to: string;
  text: string;
  ts: number;
  kind: string;
}

/** A thread's roll-up for the workspace board (cards = threads). */
/** Why a CLI is missing / unusable / outdated, for the diagnostics panel. */
export interface ToolDiagnostic {
  kind:
    | "MissingTarget"
    | "NotExecutable"
    | "SpawnFailed"
    | "VersionProbeFailed"
    | "BelowMinimum";
  message: string;
}

/** A locally-installed coding-agent CLI, for Settings' default-tool picker. */
export interface ToolStatus {
  tool: string;
  installed: boolean;
  version: string | null;
  path: string | null;
  meets_min: boolean;
  diagnostics: ToolDiagnostic[];
}

export interface SkillSource {
  id: number;
  git_url: string;
  git_ref: string;
  last_synced: string;
  last_status: string; // "never" | "ok" | "error:<msg>"
}
export interface ParsedSkill {
  name: string;
  description: string;
  dir: string;
}
export interface EnabledSkill {
  source_id: number;
  name: string;
  description: string;
  dir: string;
  overridden: boolean;
  global: boolean;
}

/** The resolved default coding tool plus the user's explicit choice (if any). */
export interface DefaultToolInfo {
  tool: string;
  configured: string | null;
}

export interface ThreadOverview {
  thread_id: number;
  title: string;
  kind: string;
  direction_ids: number[];
  /** stored lifecycle status per direction (same order as direction_ids). */
  statuses: string[];
}

/** A tool's permission request, blocked on the human (the Ask Bridge §4.3). */
export interface PermissionAsk {
  id: number;
  thread: number;
  dir: string;
  tool: string;
  summary: string;
  detail: string;
  ts: number;
  /** owning thread title + asking task name, for context on the card. */
  thread_title: string;
  dir_name: string;
}

/** An open agent→human question, aggregated workspace-wide for "Needs you". */
export interface NeedItem {
  ask_id: number;
  thread_id: number;
  thread_title: string;
  direction_id: number;
  direction_name: string;
  text: string;
  ts: number;
}

/** IM 话题绑定行：task ↔ 飞书话题 1:1 映射（M2-5）。 */
export interface ImRoute {
  thread_id: number;
  channel: string;
  chat_id: string;
  im_thread_ref: string;
  created_at: string;
}

/** Backup config + last-run telemetry surfaced to the Settings panel. Mirrors
 *  `commands_backup::BackupStatusDto` (Rust uses serde rename_all = camelCase). */
export interface BackupStatusDto {
  enabled: boolean;
  remoteUrl: string;
  autoBackupEnabled: boolean;
  backupOnExit: boolean;
  intervalSeconds: number;
  lastBackupAt: string | null;
  lastBackupCommitSha: string | null;
  lastBackupBytes: number | null;
  lastError: string | null;
}
