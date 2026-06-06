import { useState } from "react";
import { motion } from "motion/react";
import { useTranslation } from "react-i18next";
import {
  ArrowRight,
  Bell,
  Check,
  CircleCheck,
  Layers,
  Plus,
  Sparkles,
  TerminalSquare,
  X,
} from "lucide-react";
import { useStore } from "../state/store";
import type { Direction, RepoChecks, SessionStatus } from "../lib/types";
import { Button } from "../components/ui/Button";
import { StatusDot } from "../components/ui/StatusChip";
import { Inspect } from "../components/Inspect";
import { ToolIcon } from "../components/ToolIcon";
import { CreateDirectionDialog } from "../nav/dialogs";
import { CoordinationPanel } from "./CoordinationPanel";
import { ScopeConfirmView } from "./ScopeConfirmView";
import { cn } from "../lib/cn";

const TOOL_LABEL: Record<string, string> = {
  claude: "Claude",
  codex: "Codex",
  opencode: "OpenCode",
};

/** Lifecycle status of a task, derived from live state (§4.6). */
type TaskState = "queued" | "running" | "needs" | "review";

const COLUMNS: { key: TaskState; label: string; dot: string }[] = [
  { key: "queued", label: "thread.colQueued", dot: "bg-idle" },
  { key: "running", label: "thread.colRunning", dot: "bg-running" },
  { key: "needs", label: "thread.colNeeds", dot: "bg-waiting" },
  { key: "review", label: "thread.colReview", dot: "bg-brand" },
];

export function ThreadBoard() {
  const {
    threads,
    activeThreadId,
    directionsByThread,
    repos,
    proposal,
    sessions,
    needs,
    asks,
    checksByDirection,
  } = useStore();
  const { t } = useTranslation();
  const thread = threads.find((th) => th.id === activeThreadId);
  const [newDir, setNewDir] = useState(false);
  if (!thread) return null;
  const dirs = directionsByThread[thread.id] ?? [];
  const proposing = proposal?.status === "proposed";

  const statusOf = (dirId: number): TaskState => {
    const sess = Object.values(sessions).find((s) => s.directionId === dirId);
    const need =
      needs.some((n) => n.direction_id === dirId) ||
      asks.some((a) => a.dir === String(dirId));
    const checks = checksByDirection[dirId];
    const failing = (checks ?? []).some((rc) => rc.checks.some((c) => c.status === "fail"));
    if (need || failing) return "needs";
    if (sess && (sess.status === "running" || sess.status === "starting")) return "running";
    if (sess?.status === "exited" || (checks && checks.length > 0)) return "review";
    return "queued";
  };

  return (
    <section className="flex min-w-0 flex-1 flex-col overflow-hidden bg-bg">
      <header className="flex items-center gap-3 border-b border-border px-5 py-3">
        <div className="flex min-w-0 flex-col">
          <div className="flex items-center gap-2">
            <h1 className="truncate text-[16px] font-semibold tracking-tight text-ink">
              {thread.title}
            </h1>
            <span className="rounded bg-surface px-1.5 py-0.5 font-mono text-[10px] uppercase text-ink-faint">
              {t(`kind.${thread.kind}`, thread.kind)}
            </span>
          </div>
          <span className="mt-0.5 text-[12px] text-ink-faint">
            {proposing
              ? t("thread.reviewScope")
              : t("thread.directionsSub", { count: dirs.length })}
          </span>
        </div>
        {!proposing && (
          <Button variant="primary" className="ml-auto" onClick={() => setNewDir(true)}>
            <Plus size={14} />
            {t("thread.newDirection")}
          </Button>
        )}
      </header>

      <div className="flex min-h-0 flex-1">
        <div className="min-h-0 flex-1 overflow-auto">
          {proposing && proposal ? (
            <ScopeConfirmView proposal={proposal} repos={repos} taskTitle={thread.title} />
          ) : dirs.length === 0 ? (
            <div className="px-5 py-4">
              <EmptyBoard onAdd={() => setNewDir(true)} />
            </div>
          ) : (
            <div className="flex h-full min-w-fit gap-3 px-5 py-4">
              {COLUMNS.map((col) => {
                const cards = dirs.filter((d) => statusOf(d.id) === col.key);
                return (
                  <div key={col.key} className="flex w-[300px] shrink-0 flex-col gap-2">
                    <div className="flex items-center gap-2 px-1 text-[11px] font-semibold uppercase tracking-wider text-ink-faint">
                      <span className={cn("h-1.5 w-1.5 rounded-full", col.dot)} />
                      {t(col.label)}
                      <span className="tabular-nums text-ink-faint/70">{cards.length}</span>
                      {col.key === "queued" && (
                        <button
                          onClick={() => setNewDir(true)}
                          aria-label={t("thread.addDirection")}
                          className="ml-auto grid h-5 w-5 place-items-center rounded text-ink-faint transition-colors hover:bg-brand-ghost hover:text-ink"
                        >
                          <Plus size={13} />
                        </button>
                      )}
                    </div>
                    <div className="flex min-h-0 flex-1 flex-col gap-2 rounded-[var(--radius-lg)] bg-surface/40 p-2">
                      {cards.map((d) => (
                        <DirectionCard key={d.id} direction={d} />
                      ))}
                      {cards.length === 0 && (
                        <div className="flex flex-1 items-center justify-center py-6 text-[11px] text-ink-faint/60">
                          {t("thread.colEmpty")}
                        </div>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>
        {!proposing && <CoordinationPanel directions={dirs} />}
      </div>

      <CreateDirectionDialog open={newDir} onOpenChange={setNewDir} threadId={thread.id} />
    </section>
  );
}

function DirectionCard({ direction }: { direction: Direction }) {
  const {
    worktreesByDirection,
    repos,
    sessions,
    openSession,
    nudgeDirection,
    checksByDirection,
    checkingDirections,
    verifyDirection,
  } = useStore();
  const { t } = useTranslation();
  const hasLive = Object.values(sessions).some(
    (s) => s.directionId === direction.id && s.status === "running",
  );
  const writes = worktreesByDirection[direction.id] ?? [];
  const checks = checksByDirection[direction.id];
  const checking = checkingDirections[direction.id];

  return (
    <motion.div
      layout
      className="flex flex-col rounded-[var(--radius-lg)] border border-border bg-surface"
    >
      <div className="flex items-center gap-2 border-b border-border px-3 py-2.5">
        <span className="flex items-center gap-1.5 text-[13px] font-medium text-ink">
          <Layers size={13} className="text-ink-faint" />
          {direction.name}
        </span>
        {hasLive && (
          <button
            onClick={() => void nudgeDirection(direction.id)}
            aria-label={t("thread.nudge")}
            title={t("thread.nudge")}
            className="grid h-5 w-5 place-items-center rounded text-ink-faint transition-colors hover:bg-brand-ghost hover:text-brand"
          >
            <Bell size={12} />
          </button>
        )}
        {writes.length > 0 && (
          <button
            onClick={() => void verifyDirection(direction.id)}
            disabled={checking}
            aria-label={t("thread.runChecks")}
            title={t("thread.runChecks")}
            className="grid h-5 w-5 place-items-center rounded text-ink-faint transition-colors hover:bg-brand-ghost hover:text-brand disabled:opacity-50"
          >
            <CircleCheck size={12} className={checking ? "animate-pulse" : ""} />
          </button>
        )}
        <span className="ml-auto flex items-center gap-1.5 rounded-full bg-raised px-2 py-0.5 text-[11px] text-ink-muted">
          <ToolIcon tool={direction.tool} size={12} />
          {TOOL_LABEL[direction.tool] ?? direction.tool}
        </span>
      </div>

      {/* write repos — openable session slots. Each is an isolated working copy;
          the real path/branch lives in Inspect (§4.7), not on the card face. */}
      <ul className="flex flex-col gap-0.5 px-1.5 py-1.5">
        {writes.map((w) => {
          const repo = repos.find((r) => r.id === w.repo_id);
          const sess = Object.values(sessions).find(
            (s) => s.directionId === direction.id && s.repoId === w.repo_id,
          );
          return (
            <li
              key={w.id}
              className="group flex items-center gap-0.5 rounded-[var(--radius-md)] transition-colors hover:bg-brand-ghost"
            >
              <button
                onClick={() => void openSession(direction.id, w.repo_id)}
                className="flex min-w-0 flex-1 items-center gap-2 px-2 py-1.5 text-left"
              >
                <span className="grid h-5 w-5 place-items-center rounded bg-raised">
                  <TerminalSquare size={12} className="text-brand" />
                </span>
                <span className="truncate text-[12px] text-ink">
                  {repo?.name ?? `repo ${w.repo_id}`}
                </span>
                <span className="rounded bg-bg px-1 py-px font-mono text-[9px] uppercase text-running">
                  {t("thread.write")}
                </span>
                <span className="ml-auto flex items-center">
                  {sess ? (
                    <StatusDot status={sess.status as SessionStatus} />
                  ) : (
                    <ArrowRight
                      size={13}
                      className="text-ink-faint opacity-0 transition-opacity group-hover:opacity-100"
                    />
                  )}
                </span>
              </button>
              <Inspect
                path={w.path}
                branch={w.branch}
                nativeId={sess?.nativeId}
                size={13}
                className="mr-1 h-6 w-6 shrink-0 opacity-0 group-hover:opacity-100"
              />
            </li>
          );
        })}
      </ul>

      {checks && checks.length > 0 && (
        <div className="flex flex-col gap-1.5 border-t border-border px-3 py-2">
          {checks.map((rc) => (
            <ChecksRow key={rc.repo} rc={rc} />
          ))}
        </div>
      )}
    </motion.div>
  );
}

function ChecksRow({ rc }: { rc: RepoChecks }) {
  const { t } = useTranslation();
  if (rc.checks.length === 0) {
    return (
      <div className="flex items-center gap-2 text-[11px]">
        <span className="truncate text-ink-muted">{rc.repo}</span>
        <span className="text-ink-faint">{t("thread.noChecks")}</span>
      </div>
    );
  }
  return (
    <div className="flex flex-wrap items-center gap-1.5 text-[11px]">
      <span className="mr-0.5 truncate text-ink-muted">{rc.repo}</span>
      {rc.checks.map((c) => {
        const pass = c.status === "pass";
        return (
          <span
            key={c.name}
            title={pass ? `${c.name}: passed` : c.output_tail || `${c.name}: failed (exit ${c.code})`}
            className={cn(
              "flex items-center gap-1 rounded-full px-1.5 py-0.5 font-medium",
              pass ? "bg-running/15 text-running" : "bg-[oklch(0.64_0.2_25/0.15)] text-danger",
            )}
          >
            {pass ? <Check size={10} /> : <X size={10} />}
            {c.name}
          </span>
        );
      })}
    </div>
  );
}

function EmptyBoard({ onAdd }: { onAdd: () => void }) {
  const { startDraftPlan, planWithLead } = useStore();
  const { t } = useTranslation();
  return (
    <div className="flex h-full flex-col items-center justify-center text-center">
      <div className="grid h-11 w-11 place-items-center rounded-[var(--radius-lg)] border border-border bg-surface">
        <Layers size={20} className="text-ink-faint" />
      </div>
      <h2 className="mt-3 text-[14px] font-semibold text-ink">{t("thread.planTitle")}</h2>
      <p className="mt-1 max-w-xs text-[12px] leading-relaxed text-ink-faint">
        {t("thread.planBody")}
      </p>
      <div className="mt-4 flex items-center gap-2">
        <Button variant="primary" onClick={() => void planWithLead()}>
          <Sparkles size={14} />
          {t("thread.planWithLead")}
        </Button>
        <Button variant="ghost" onClick={() => void startDraftPlan()}>
          {t("thread.draftManually")}
        </Button>
        <Button variant="ghost" onClick={onAdd}>
          <Plus size={14} />
          {t("thread.newDirection")}
        </Button>
      </div>
    </div>
  );
}
