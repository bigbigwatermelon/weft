import { useMemo, useState } from "react";
import { motion, useReducedMotion } from "motion/react";
import {
  AppWindow,
  Boxes,
  CircleDashed,
  FileText,
  Network,
  Package,
  Pencil,
  RefreshCw,
  Server,
} from "lucide-react";
import { useStore } from "../state/store";
import type { RepoProfile } from "../lib/types";
import { Input } from "../components/ui/Input";
import { cn } from "../lib/cn";

const ROLE_ICON: Record<string, typeof Server> = {
  service: Server,
  app: AppWindow,
  library: Package,
  infra: Boxes,
  docs: FileText,
  unknown: CircleDashed,
};

/**
 * The Repo map (ARCHITECTURE §4.9): what each repo is, and how they depend on
 * one another — the curator's map that powers cross-repo scope decomposition.
 * Inferred on add; correcting a summary teaches the map (source flips to user
 * and survives re-profiling).
 */
export function RepoMapView() {
  const { repoProfiles, repoEdges } = useStore();
  const reduce = useReducedMotion();

  const nameOf = useMemo(() => {
    const m = new Map<number, string>();
    for (const p of repoProfiles) m.set(p.repo_id, p.repo_name);
    return (id: number) => m.get(id) ?? `repo ${id}`;
  }, [repoProfiles]);

  return (
    <section className="flex min-w-0 flex-1 flex-col bg-bg">
      <header className="flex items-center gap-2.5 border-b border-border px-5 py-3">
        <span className="grid h-6 w-6 place-items-center rounded-[var(--radius-sm)] bg-brand-ghost">
          <Network size={14} className="text-brand" />
        </span>
        <h1 className="text-[16px] font-semibold tracking-tight text-ink">
          Repo map
        </h1>
        {repoProfiles.length > 0 && (
          <span className="rounded-full bg-raised px-2 py-0.5 text-[11px] tabular-nums text-ink-muted">
            {repoProfiles.length}
          </span>
        )}
        <span className="ml-auto text-[12px] text-ink-faint">
          what each repo is, and how they depend on each other
        </span>
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto">
        {repoProfiles.length === 0 ? (
          <EmptyMap />
        ) : (
          <div className="mx-auto flex w-full max-w-[760px] flex-col gap-2.5 px-5 py-5">
            {repoProfiles.map((p, i) => {
              const dependsOn = repoEdges.filter((e) => e.from === p.repo_id);
              const usedBy = repoEdges.filter((e) => e.to === p.repo_id);
              return (
                <motion.div
                  key={p.repo_id}
                  initial={reduce ? false : { opacity: 0, y: 6 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{
                    duration: 0.18,
                    delay: reduce ? 0 : Math.min(i * 0.03, 0.18),
                    ease: [0.22, 1, 0.36, 1],
                  }}
                >
                  <RepoRow
                    profile={p}
                    dependsOn={dependsOn.map((e) => ({ name: nameOf(e.to), via: e.via }))}
                    usedBy={usedBy.map((e) => nameOf(e.from))}
                  />
                </motion.div>
              );
            })}
          </div>
        )}
      </div>
    </section>
  );
}

function RepoRow({
  profile,
  dependsOn,
  usedBy,
}: {
  profile: RepoProfile;
  dependsOn: { name: string; via: string }[];
  usedBy: string[];
}) {
  const { reprofileRepo } = useStore();
  const Icon = ROLE_ICON[profile.role] ?? CircleDashed;
  const isCore = usedBy.length >= 2;

  return (
    <div className="rounded-[var(--radius-lg)] border border-border bg-surface px-3.5 py-3">
      <div className="flex items-center gap-2">
        <span className="grid h-6 w-6 shrink-0 place-items-center rounded-[var(--radius-sm)] bg-raised">
          <Icon size={13} className="text-ink-muted" />
        </span>
        <span className="truncate text-[13px] font-medium text-ink">
          {profile.repo_name}
        </span>
        <span className="rounded-full bg-bg px-1.5 py-0.5 text-[10px] capitalize text-ink-faint">
          {profile.role}
        </span>
        {isCore && (
          <span
            title={`${usedBy.length} repos depend on this — changes ripple`}
            className="rounded-full bg-accent-ghost px-1.5 py-0.5 text-[10px] font-medium text-accent"
          >
            core · {usedBy.length} dependents
          </span>
        )}
        <div className="ml-auto flex items-center gap-1.5">
          {profile.stale && (
            <span
              title="The repo moved since this was profiled"
              className="flex items-center gap-1 text-[10px] text-waiting"
            >
              <span className="h-1.5 w-1.5 rounded-full bg-waiting" />
              stale
            </span>
          )}
          {profile.stack.map((s) => (
            <span
              key={s}
              className="rounded bg-bg px-1.5 py-0.5 font-mono text-[10px] text-ink-faint"
            >
              {s}
            </span>
          ))}
          <button
            onClick={() => void reprofileRepo(profile.repo_id)}
            aria-label="Re-profile this repo"
            title="Re-profile from disk"
            className="grid h-6 w-6 place-items-center rounded text-ink-faint transition-colors hover:bg-brand-ghost hover:text-ink"
          >
            <RefreshCw size={12} />
          </button>
        </div>
      </div>

      <EditableSummary profile={profile} />

      {(dependsOn.length > 0 || usedBy.length > 0) && (
        <div className="mt-2 flex flex-col gap-1 border-t border-border pt-2">
          {dependsOn.length > 0 && (
            <DepLine label="depends on" arrow="→">
              {dependsOn.map((d) => (
                <Chip key={d.name} title={`via ${d.via}`}>
                  {d.name}
                </Chip>
              ))}
            </DepLine>
          )}
          {usedBy.length > 0 && (
            <DepLine label="used by" arrow="←">
              {usedBy.map((n) => (
                <Chip key={n}>{n}</Chip>
              ))}
            </DepLine>
          )}
        </div>
      )}
    </div>
  );
}

function EditableSummary({ profile }: { profile: RepoProfile }) {
  const { editRepoProfile } = useStore();
  const [editing, setEditing] = useState(false);
  const [text, setText] = useState(profile.summary);

  async function save() {
    setEditing(false);
    const next = text.trim();
    if (next === profile.summary) return;
    await editRepoProfile(profile.repo_id, next, profile.role);
  }

  if (editing) {
    return (
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void save();
        }}
        className="mt-1.5"
      >
        <Input
          autoFocus
          value={text}
          placeholder="One line: what is this repo for?"
          onChange={(e) => setText(e.currentTarget.value)}
          onBlur={() => void save()}
        />
      </form>
    );
  }

  return (
    <button
      onClick={() => {
        setText(profile.summary);
        setEditing(true);
      }}
      className="group/sum mt-1 flex w-full items-center gap-1.5 text-left"
      title="Click to edit — your wording outranks inference"
    >
      <span
        className={cn(
          "text-[12.5px] leading-relaxed",
          profile.summary ? "text-ink-muted" : "text-ink-faint italic",
        )}
      >
        {profile.summary || "Add a one-line description"}
      </span>
      {profile.source === "user" && (
        <span className="rounded bg-brand-ghost px-1 py-px text-[9px] font-medium text-brand">
          yours
        </span>
      )}
      <Pencil
        size={11}
        className="shrink-0 text-ink-faint opacity-0 transition-opacity group-hover/sum:opacity-100"
      />
    </button>
  );
}

function DepLine({
  label,
  arrow,
  children,
}: {
  label: string;
  arrow: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex flex-wrap items-center gap-1.5">
      <span className="w-[68px] shrink-0 text-[10px] uppercase tracking-wide text-ink-faint">
        {arrow} {label}
      </span>
      {children}
    </div>
  );
}

function Chip({
  children,
  title,
}: {
  children: React.ReactNode;
  title?: string;
}) {
  return (
    <span
      title={title}
      className="rounded-full border border-border bg-bg px-2 py-0.5 text-[11px] text-ink-muted"
    >
      {children}
    </span>
  );
}

function EmptyMap() {
  return (
    <div className="flex h-full flex-col items-center justify-center px-6 text-center">
      <div className="grid h-12 w-12 place-items-center rounded-[var(--radius-lg)] border border-border bg-surface">
        <Network size={22} className="text-ink-faint" />
      </div>
      <h2 className="mt-4 text-[15px] font-semibold text-ink">No repos yet</h2>
      <p className="mt-1.5 max-w-sm text-[13px] leading-relaxed text-ink-faint">
        Add a repo and weft profiles it on the spot — role, stack, what it
        publishes — then links it into the dependency graph. That map is what
        lets a task split itself across the right repos.
      </p>
    </div>
  );
}
