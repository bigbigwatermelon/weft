import { cn } from "../lib/cn";

const SRC: Record<string, string> = {
  claude: "/tools/claude.svg",
  codex: "/tools/codex.svg",
  opencode: "/tools/opencode.svg",
};

const FULL_NAME: Record<string, string> = {
  claude: "Claude Code",
  codex: "Codex",
  opencode: "OpenCode",
};

export function toolFullName(tool: string) {
  return FULL_NAME[tool] ?? tool;
}

/** The official mark for a coding tool (claude / codex / opencode). */
export function ToolIcon({
  tool,
  size = 14,
  className,
}: {
  tool: string;
  size?: number;
  className?: string;
}) {
  const src = SRC[tool];
  if (!src) return null;
  return (
    <img
      src={src}
      alt={toolFullName(tool)}
      width={size}
      height={size}
      draggable={false}
      className={cn("shrink-0 rounded-[3px]", className)}
    />
  );
}
