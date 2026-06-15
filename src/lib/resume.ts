// Build the shell command to resume a session in the user's own terminal, and
// the app deep link where one exists. atlas drives native CLIs, so a session can
// always be picked back up outside atlas (architecture §5.6).

function shq(s: string): string {
  return `'${s.replace(/'/g, "'\\''")}'`;
}

/** `cd <cwd> && <tool> resume <id>` for the given tool. */
export function resumeCommand(tool: string, cwd: string, nativeId: string): string {
  const at = `cd ${shq(cwd)} && `;
  switch (tool) {
    case "claude":
      return `${at}claude --resume ${nativeId}`;
    case "codex":
      return `${at}codex resume ${nativeId}`;
    case "opencode":
      return `${at}opencode . --session ${nativeId}`;
    default:
      return at + tool;
  }
}

/** An app deep link to the session, where the tool offers one (Codex only). */
export function appLink(tool: string, nativeId: string): string | null {
  if (tool === "codex") return `codex://threads/${nativeId}`;
  return null;
}
