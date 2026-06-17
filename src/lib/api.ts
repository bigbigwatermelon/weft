import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type {
  BackupStatusDto,
  BusMsg,
  ComputerUseStatus,
  DefaultToolInfo,
  Direction,
  EnabledSkill,
  ImageAttachment,
  ImRoute,
  LeadMessage,
  LeadStateInfo,
  NeedItem,
  NormEvent,
  ObserveRef,
  ParsedSkill,
  PermissionAsk,
  SessionInfo,
  SkillSource,
  SlashCmd,
  Thread,
  ThreadOverview,
  ToolStatus,
  Workspace,
} from "./types";

// Tauri converts camelCase command args to snake_case Rust params.

type SessionInfoDto = {
  session_id: number;
  run_dir: string;
  cwd: string;
  tool: string;
  resumed: boolean;
  native_id: string | null;
};

type ObserveRefDto = {
  run_dir: string;
  tool: string;
  session_id: number | null;
  native_id: string | null;
  status: string | null;
};

function normalizeSessionInfo(raw: SessionInfoDto): SessionInfo {
  return {
    session_id: raw.session_id,
    run_dir: raw.run_dir || raw.cwd,
    cwd: raw.cwd || raw.run_dir,
    tool: raw.tool,
    resumed: raw.resumed,
    native_id: raw.native_id,
  };
}

function normalizeObserveRef(raw: ObserveRefDto | null): ObserveRef | null {
  if (!raw) return null;
  return {
    run_dir: raw.run_dir,
    tool: raw.tool,
    session_id: raw.session_id,
    native_id: raw.native_id,
    status: raw.status,
  };
}

export const api = {
  listWorkspaces: () => invoke<Workspace[]>("list_workspaces"),
  ensureDefaultWorkspace: () =>
    invoke<number>("ensure_default_workspace"),

  listThreads: (workspaceId: number) =>
    invoke<Thread[]>("list_threads", { workspaceId }),
  workspaceOverview: (workspaceId: number) =>
    invoke<ThreadOverview[]>("workspace_overview", { workspaceId }),
  createThread: (workspaceId: number, title: string, kind: string) =>
    invoke<Thread>("create_thread", { workspaceId, title, kind }),
  renameThread: (threadId: number, title: string) =>
    invoke<Thread>("rename_thread", { threadId, title }),
  deleteThread: (threadId: number) =>
    invoke<void>("delete_thread", { threadId }),

  listDirections: (threadId: number) =>
    invoke<Direction[]>("list_directions", { threadId }),
  setTaskStatus: (directionId: number, status: string) =>
    invoke<void>("set_task_status", { directionId, status }),
  renameDirection: (directionId: number, name: string) =>
    invoke<Direction>("rename_direction", { directionId, name }),

  createRun: (threadId: number, name: string, tool: string) =>
    invoke<Direction>("create_run", { threadId, name, tool }),

  // Lead chat engine: atlas-owned conversation (headless stream-json claude).
  leadSend: (
    threadId: number,
    text: string,
    lang: string,
    images?: ImageAttachment[],
    files?: string[],
  ) => invoke<void>("lead_send", { threadId, text, lang, images, files }),
  leadInterrupt: (threadId: number) =>
    invoke<void>("lead_interrupt", { threadId }),
  leadEnsure: (threadId: number, lang: string) =>
    invoke<void>("lead_ensure", { threadId, lang }),
  leadStop: (threadId: number) => invoke<void>("lead_stop", { threadId }),
  leadState: (threadId: number) =>
    invoke<LeadStateInfo>("lead_state", { threadId }),
  listLeadMessages: (threadId: number) =>
    invoke<LeadMessage[]>("list_lead_messages", { threadId }),
  /** Live slash-command discovery for a worker (sessionId) or the lead
   *  (threadId) — claude's initialize list, opencode's GET /command, codex none. */
  discoverSlash: (threadId: number | null, sessionId: number | null) =>
    invoke<SlashCmd[]>("discover_slash", { threadId, sessionId }),

  // Chat-mode workers (claude): same engine, keyed by session id.
  chatOpenRun: (directionId: number, lang: string) =>
    invoke<SessionInfoDto>("chat_open_run", { directionId, lang }).then(normalizeSessionInfo),
  chatSend: (
    sessionId: number,
    text: string,
    images?: ImageAttachment[],
    files?: string[],
  ) => invoke<void>("chat_send", { sessionId, text, images, files }),
  chatInterrupt: (sessionId: number) =>
    invoke<void>("chat_interrupt", { sessionId }),
  chatStop: (sessionId: number) => invoke<void>("chat_stop", { sessionId }),
  sessionFor: (directionId: number) =>
    invoke<ObserveRefDto | null>("session_for", { directionId }).then(normalizeObserveRef),
  readTranscript: (cwd: string, tool: string) =>
    invoke<NormEvent[]>("read_transcript", { cwd, tool }),

  threadMessages: (threadId: number) =>
    invoke<BusMsg[]>("thread_messages", { threadId }),
  busPostHuman: (threadId: number, to: string | null, text: string) =>
    invoke<void>("bus_post_human", { threadId, to, text }),

  // Ask Bridge: pending tool permission requests + the answer.
  pendingAsks: () => invoke<PermissionAsk[]>("pending_asks"),
  answerPermission: (askId: number, answer: "allow" | "deny" | "always" | "full") =>
    invoke<void>("answer_permission", { askId, answer }),

  // Needs-you: open agent→human questions, aggregated across the workspace.
  needsYou: (workspaceId: number) =>
    invoke<NeedItem[]>("needs_you", { workspaceId }),
  answerAsk: (threadId: number, askId: number, text: string) =>
    invoke<void>("answer_ask", { threadId, askId, text }),

  // Inspect escape hatches: real ways into the hidden plumbing.
  openTerminal: (path: string) => invoke<void>("open_terminal", { path }),
  revealPath: (path: string) => invoke<void>("reveal_path", { path }),
  openUrl: (url: string) => invoke<void>("open_url", { url }),

  // Which coding-agent CLIs are installed locally (for Settings).
  detectTools: () => invoke<ToolStatus[]>("detect_tools"),
  computerUseGetStatus: () =>
    invoke<ComputerUseStatus>("computer_use_get_status"),
  computerUseSetEnabled: (enabled: boolean) =>
    invoke<void>("computer_use_set_enabled", { enabled }),
  computerUseRunDoctor: () =>
    invoke<string>("computer_use_run_doctor"),
  getDefaultTool: () => invoke<DefaultToolInfo>("get_default_tool"),
  setDefaultTool: (tool: string) => invoke<void>("set_default_tool", { tool }),
  // Dangerous mode (global): every agent's tool asks auto-allow.
  setDangerousMode: (on: boolean) => invoke<void>("set_dangerous_mode", { on }),
  // Keep-awake: prevent system idle sleep while any session is running.
  setKeepAwake: (on: boolean) => invoke<void>("set_keep_awake", { on }),
  // Runaway guardrails: idle + wall-clock caps (seconds; 0 disables) for
  // force-stopping a stuck/runaway agent (enforcement pending on the engine).
  setGuardrails: (idleSecs: number, wallSecs: number) =>
    invoke<void>("set_guardrails", { idleSecs, wallSecs }),
  listSkillSources: () => invoke<SkillSource[]>("list_skill_sources"),
  addSkillSource: (gitUrl: string, gitRef?: string) =>
    invoke<SkillSource>("add_skill_source", { gitUrl, gitRef }),
  removeSkillSource: (id: number) => invoke<void>("remove_skill_source", { id }),
  syncSkillSource: (id: number) => invoke<SkillSource>("sync_skill_source", { id }),
  syncAllSkillSources: () => invoke<SkillSource[]>("sync_all_skill_sources"),
  listParsedSkills: (id: number) => invoke<ParsedSkill[]>("list_parsed_skills", { id }),
  setSkillEnabled: (sourceId: number, name: string, scope: string, on: boolean) =>
    invoke<void>("set_skill_enabled", { sourceId, name, scope, on }),
  workspaceSkills: (wsId: number) => invoke<EnabledSkill[]>("workspace_skills", { wsId }),
  flagSessionSkillRefresh: (sessionId: number) =>
    invoke<void>("flag_session_skill_refresh", { sessionId }),
  flagLeadSkillRefresh: (threadId: number) =>
    invoke<void>("flag_lead_skill_refresh", { threadId }),
  imGetSettings: () =>
    invoke<{ app_id: string; has_secret: boolean; bound: boolean; enabled: boolean }>(
      "im_get_settings",
    ),
  imSetSettings: (appId: string, appSecret: string) =>
    invoke<void>("im_set_settings", { appId, appSecret }),
  imSetEnabled: (enabled: boolean) =>
    invoke<void>("im_set_enabled", { enabled }),
  imStatus: () => invoke<string>("im_status"),
  imBindThread: (threadId: number, chatId: string, imThreadRef: string, channel = "feishu") =>
    invoke<ImRoute>("im_bind_thread", { threadId, channel, chatId, imThreadRef }),
  imUnbindThread: (threadId: number) =>
    invoke<void>("im_unbind_thread", { threadId }),
  imRouteForThread: (threadId: number) =>
    invoke<ImRoute | null>("im_route_for_thread", { threadId }),
  imListRoutes: () => invoke<ImRoute[]>("im_list_routes"),
  backupGetStatus: () => invoke<BackupStatusDto>("backup_get_status"),
  backupSavePrefs: (
    enabled: boolean,
    remoteUrl: string,
    autoBackupEnabled: boolean,
    backupOnExit: boolean,
  ) =>
    invoke<void>("backup_save_prefs", {
      enabled,
      remoteUrl,
      autoBackupEnabled,
      backupOnExit,
    }),
  backupTestRemote: (remoteUrl: string) =>
    invoke<void>("backup_test_remote", { remoteUrl }),
  backupRunNow: () => invoke<BackupStatusDto>("backup_run_now"),
  backupExportRecoveryKey: (targetPath: string) =>
    invoke<void>("backup_export_recovery_key", { targetPath }),
  backupRestore: (remoteUrl: string, recoveryKeyPath: string) =>
    invoke<void>("backup_restore", { remoteUrl, recoveryKeyPath }),
  // Native folder picker; returns the chosen absolute path, or null if cancelled.
  pickFolder: async (title?: string) => {
    const sel = await openDialog({ directory: true, multiple: false, title });
    return typeof sel === "string" ? sel : null;
  },
  // Native multi-file picker; [] when cancelled.
  pickFiles: async (title?: string) => {
    const sel = await openDialog({ directory: false, multiple: true, title });
    return Array.isArray(sel) ? sel : typeof sel === "string" ? [sel] : [];
  },
};
