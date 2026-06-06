import { useMemo, useState } from "react";
import { Layers, Lightbulb, Plus, Sparkles, X } from "lucide-react";
import { useStore } from "../state/store";
import type { Proposal, RepoRef, ResolvedProposal } from "../lib/types";
import { Button } from "../components/ui/Button";
import { Select } from "../components/ui/Select";
import { cn } from "../lib/cn";

type RoleState = "none" | "read" | "write";

interface DraftDir {
  name: string;
  tool: string;
  roles: Record<number, RoleState>;
}

const TOOL_OPTIONS = [
  { value: "claude", label: "Claude" },
  { value: "codex", label: "Codex" },
  { value: "opencode", label: "OpenCode" },
];

/**
 * The scope-confirm step (ARCHITECTURE §5.1): review the lead's proposed split
 * of a Task into directions, correct write/read/none per repo, then create the
 * directions — only write repos get a worktree. The human edits; nothing is
 * materialized until "Create".
 */
export function ScopeConfirmView({
  proposal,
  repos,
  taskTitle,
}: {
  proposal: ResolvedProposal;
  repos: RepoRef[];
  taskTitle: string;
}) {
  const { saveProposal, confirmProposal } = useStore();
  const [busy, setBusy] = useState(false);

  const [dirs, setDirs] = useState<DraftDir[]>(() =>
    proposal.directions.map((d) => ({
      name: d.name,
      tool: d.tool,
      roles: Object.fromEntries(
        repos.map((r) => {
          const entry = d.scope.find((s) => s.repo_id === r.id);
          return [r.id, (entry?.role as RoleState) ?? "none"];
        }),
      ),
    })),
  );

  const built = useMemo<Proposal>(
    () => ({
      rationale: proposal.rationale,
      directions: dirs.map((d) => ({
        name: d.name.trim() || "Untitled",
        tool: d.tool,
        writes: repos.filter((r) => d.roles[r.id] === "write").map((r) => r.name),
        reads: repos.filter((r) => d.roles[r.id] === "read").map((r) => r.name),
      })),
    }),
    [dirs, repos, proposal.rationale],
  );

  const writeCount = built.directions.filter((d) => d.writes.length > 0).length;
  const canCreate = dirs.length > 0 && writeCount > 0 && !busy;

  function patch(i: number, next: Partial<DraftDir>) {
    setDirs((cur) => cur.map((d, j) => (j === i ? { ...d, ...next } : d)));
  }
  function setRole(i: number, repoId: number, role: RoleState) {
    setDirs((cur) =>
      cur.map((d, j) => (j === i ? { ...d, roles: { ...d.roles, [repoId]: role } } : d)),
    );
  }
  function addDir() {
    setDirs((cur) => [
      ...cur,
      {
        name: `Direction ${cur.length + 1}`,
        tool: "claude",
        roles: Object.fromEntries(repos.map((r) => [r.id, "none" as RoleState])),
      },
    ]);
  }
  function removeDir(i: number) {
    setDirs((cur) => cur.filter((_, j) => j !== i));
  }

  async function saveDraft() {
    setBusy(true);
    try {
      await saveProposal(built);
    } finally {
      setBusy(false);
    }
  }
  async function create() {
    setBusy(true);
    try {
      await saveProposal(built); // confirm reads the stored proposal
      await confirmProposal();
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="mx-auto flex w-full max-w-[760px] flex-col gap-3 px-5 py-5">
      <div className="flex items-center gap-2 text-[12px] text-ink-faint">
        <Sparkles size={13} className="text-accent" />
        <span>
          Proposed plan for <span className="text-ink-muted">{taskTitle}</span>
        </span>
      </div>

      {proposal.rationale && (
        <div className="flex gap-2 rounded-[var(--radius-md)] border border-border bg-surface px-3 py-2.5">
          <Lightbulb size={14} className="mt-0.5 shrink-0 text-accent" />
          <p className="text-[12.5px] leading-relaxed text-ink-muted">
            {proposal.rationale}
          </p>
        </div>
      )}

      {dirs.map((d, i) => (
        <DirectionEditor
          key={i}
          dir={d}
          repos={repos}
          onName={(name) => patch(i, { name })}
          onTool={(tool) => patch(i, { tool })}
          onRole={(repoId, role) => setRole(i, repoId, role)}
          onRemove={dirs.length > 1 ? () => removeDir(i) : undefined}
        />
      ))}

      <button
        onClick={addDir}
        className="flex items-center justify-center gap-2 rounded-[var(--radius-lg)] border border-dashed border-border py-2.5 text-[12px] text-ink-faint transition-colors hover:border-border-strong hover:bg-surface hover:text-ink-muted"
      >
        <Plus size={14} />
        Add direction
      </button>

      <div className="sticky bottom-0 mt-1 flex items-center gap-2 border-t border-border bg-bg/90 py-3 backdrop-blur">
        <span className="text-[12px] text-ink-faint">
          {writeCount > 0
            ? `${dirs.length} ${dirs.length === 1 ? "direction" : "directions"} · ${writeCount} will open a worktree`
            : "Give at least one direction a write repo to create it"}
        </span>
        <div className="ml-auto flex items-center gap-2">
          <Button variant="ghost" onClick={() => void saveDraft()} disabled={busy}>
            Save draft
          </Button>
          <Button variant="primary" onClick={() => void create()} disabled={!canCreate}>
            <Layers size={14} />
            Create {writeCount > 0 ? writeCount : ""}{" "}
            {writeCount === 1 ? "direction" : "directions"}
          </Button>
        </div>
      </div>
    </div>
  );
}

function DirectionEditor({
  dir,
  repos,
  onName,
  onTool,
  onRole,
  onRemove,
}: {
  dir: DraftDir;
  repos: RepoRef[];
  onName: (v: string) => void;
  onTool: (v: string) => void;
  onRole: (repoId: number, role: RoleState) => void;
  onRemove?: () => void;
}) {
  return (
    <div className="rounded-[var(--radius-lg)] border border-border bg-surface">
      <div className="flex items-center gap-2 border-b border-border px-3 py-2.5">
        <Layers size={13} className="shrink-0 text-ink-faint" />
        <input
          value={dir.name}
          onChange={(e) => onName(e.currentTarget.value)}
          placeholder="Direction name"
          className="min-w-0 flex-1 bg-transparent text-[13px] font-medium text-ink outline-none placeholder:text-ink-faint"
        />
        <div className="w-32 shrink-0">
          <Select value={dir.tool} onValueChange={onTool} ariaLabel="Tool" options={TOOL_OPTIONS} />
        </div>
        {onRemove && (
          <button
            onClick={onRemove}
            aria-label="Remove direction"
            className="grid h-6 w-6 shrink-0 place-items-center rounded text-ink-faint transition-colors hover:bg-[oklch(0.64_0.2_25/0.15)] hover:text-danger"
          >
            <X size={13} />
          </button>
        )}
      </div>
      <ul className="flex flex-col">
        {repos.map((r) => (
          <li
            key={r.id}
            className="flex items-center gap-2 px-3 py-1.5 [&:not(:last-child)]:border-b [&:not(:last-child)]:border-border/60"
          >
            <span className="truncate text-[12px] text-ink-muted">{r.name}</span>
            <div className="ml-auto">
              <ScopeToggle value={dir.roles[r.id] ?? "none"} onChange={(v) => onRole(r.id, v)} />
            </div>
          </li>
        ))}
        {repos.length === 0 && (
          <li className="px-3 py-3 text-center text-[11px] text-ink-faint">
            Add repos to this workspace first.
          </li>
        )}
      </ul>
    </div>
  );
}

const SEGMENTS: { value: RoleState; label: string }[] = [
  { value: "none", label: "None" },
  { value: "read", label: "Read" },
  { value: "write", label: "Write" },
];

function ScopeToggle({
  value,
  onChange,
}: {
  value: RoleState;
  onChange: (v: RoleState) => void;
}) {
  return (
    <div className="inline-flex overflow-hidden rounded-[var(--radius-sm)] border border-border">
      {SEGMENTS.map((s) => {
        const active = s.value === value;
        return (
          <button
            key={s.value}
            onClick={() => onChange(s.value)}
            className={cn(
              "px-2 py-0.5 text-[11px] transition-colors",
              active
                ? s.value === "write"
                  ? "bg-running/20 font-medium text-running"
                  : s.value === "read"
                    ? "bg-brand-ghost font-medium text-brand"
                    : "bg-raised font-medium text-ink-muted"
                : "text-ink-faint hover:bg-raised hover:text-ink-muted",
            )}
          >
            {s.label}
          </button>
        );
      })}
    </div>
  );
}
