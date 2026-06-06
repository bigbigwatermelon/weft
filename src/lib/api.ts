import { invoke } from "@tauri-apps/api/core";
import type {
  BusMsg,
  Direction,
  DirectionRepo,
  NeedItem,
  RepoGraph,
  RepoRef,
  Role,
  SessionInfo,
  Thread,
  Workspace,
  Worktree,
} from "./types";

// Tauri converts camelCase command args to snake_case Rust params. Nested
// structs (scope items) keep serde field names, hence `repo_id`/`role`.

export const api = {
  listWorkspaces: () => invoke<Workspace[]>("list_workspaces"),
  createWorkspace: (name: string) =>
    invoke<Workspace>("create_workspace", { name }),

  listRepos: (workspaceId: number) =>
    invoke<RepoRef[]>("list_repos", { workspaceId }),
  addRepoRef: (workspaceId: number, name: string, localGitPath: string) =>
    invoke<RepoRef>("add_repo_ref", { workspaceId, name, localGitPath }),

  // Repo map (curator): profiles + cross-repo dependency graph.
  repoGraph: (workspaceId: number) =>
    invoke<RepoGraph>("repo_graph", { workspaceId }),
  reprofileRepo: (repoId: number) =>
    invoke<void>("reprofile_repo", { repoId }),
  updateRepoProfile: (repoId: number, summary: string, role: string) =>
    invoke<void>("update_repo_profile", { repoId, summary, role }),

  listThreads: (workspaceId: number) =>
    invoke<Thread[]>("list_threads", { workspaceId }),
  createThread: (workspaceId: number, title: string, kind: string) =>
    invoke<Thread>("create_thread", { workspaceId, title, kind }),
  deleteThread: (threadId: number) =>
    invoke<void>("delete_thread", { threadId }),

  listDirections: (threadId: number) =>
    invoke<Direction[]>("list_directions", { threadId }),
  listDirectionRepos: (directionId: number) =>
    invoke<DirectionRepo[]>("list_direction_repos", { directionId }),
  createDirection: (
    threadId: number,
    name: string,
    tool: string,
    scope: { repo_id: number; role: Role }[],
  ) => invoke<Direction>("create_direction", { threadId, name, tool, scope }),

  listWorktrees: (directionId: number) =>
    invoke<Worktree[]>("list_worktrees", { directionId }),

  openSession: (directionId: number, repoId: number) =>
    invoke<SessionInfo>("open_session", { directionId, repoId }),
  resumeSession: (sessionId: number) =>
    invoke<SessionInfo>("resume_session", { sessionId }),
  writePty: (sessionId: number, data: string) =>
    invoke<void>("write_pty", { sessionId, data }),
  resizePty: (sessionId: number, rows: number, cols: number) =>
    invoke<void>("resize_pty", { sessionId, rows, cols }),
  killSession: (sessionId: number) =>
    invoke<void>("kill_session", { sessionId }),

  threadMessages: (threadId: number) =>
    invoke<BusMsg[]>("thread_messages", { threadId }),
  busPostHuman: (threadId: number, to: string | null, text: string) =>
    invoke<void>("bus_post_human", { threadId, to, text }),

  // Needs-you: open agent→human questions, aggregated across the workspace.
  needsYou: (workspaceId: number) =>
    invoke<NeedItem[]>("needs_you", { workspaceId }),
  answerAsk: (threadId: number, askId: number, text: string) =>
    invoke<void>("answer_ask", { threadId, askId, text }),

  // Inspect escape hatches (§4.7): real ways into the hidden plumbing.
  openTerminal: (path: string) => invoke<void>("open_terminal", { path }),
  revealPath: (path: string) => invoke<void>("reveal_path", { path }),
};
