import { useEffect, useState } from "react";
import { motion } from "motion/react";
import { useTranslation } from "react-i18next";
import * as DM from "@radix-ui/react-dropdown-menu";
import {
  Check,
  ChevronDown,
  ChevronRight,
  Layers,
  MessagesSquare,
  ScanEye,
  TerminalSquare,
  X,
} from "lucide-react";
import { useStore } from "../state/store";
import type { Direction, RepoChecks, SessionStatus } from "../lib/types";
import { Button } from "../components/ui/Button";
import { StatusDot } from "../components/ui/StatusChip";
import { Inspect } from "../components/Inspect";
import { ToolIcon, toolFullName } from "../components/ToolIcon";
import { ScopeReview } from "./ScopeReview";
import { LeadTab } from "../session/LeadTab";
import { cn } from "../lib/cn";

/** Task lifecycle column. "needs" is a weft overlay (an open ask / failing
 *  check); the rest are the agent/human-set stored status (§4.6). */
type TaskState = "queued" | "working" | "needs" | "review" | "done";

const COLUMNS: { key: TaskState; label: string; dot: string }[] = [
  { key: "queued", label: "thread.colQueued", dot: "bg-idle" },
  { key: "working", label: "thread.colRunning", dot: "bg-running" },
  { key: "needs", label: "thread.colNeeds", dot: "bg-waiting" },
  { key: "review", label: "thread.colReview", dot: "bg-brand" },
  { key: "done", label: "thread.colDone", dot: "bg-accent" },
];

export function ThreadBoard() {
  const {
    threads,
    activeThreadId,
    directionsByThread,
    proposal,
    reviewingProposal,
    setReviewingProposal,
    threadTab,
    setThreadTab,
    needs,
    asks,
    checksByDirection,
  } = useStore();
  const { t } = useTranslation();
  const thread = threads.find((th) => th.id === activeThreadId);
  useEffect(() => {
    setThreadTab("lead");
    setReviewingProposal(false);
  }, [activeThreadId, setReviewingProposal, setThreadTab]);

  if (!thread) return null;
  const dirs = directionsByThread[thread.id] ?? [];

  // Column = the stored, agent/human-set status; an open ask/need or a failing
  // check overlays the task into Needs-you (the exception lane weft owns).
  const statusOf = (d: Direction): TaskState => {
    const need =
      needs.some((n) => n.direction_id === d.id) ||
      asks.some((a) => a.dir === String(d.id));
    const failing = (checksByDirection[d.id] ?? []).some((rc) =>
      rc.checks.some((c) => c.status === "fail"),
    );
    if (need || failing) return "needs";
    const s = d.status;
    if (s === "working" || s === "review" || s === "done") return s;
    return "queued";
  };

  return (
    <section className="flex min-w-0 flex-1 flex-col overflow-hidden bg-bg">
      <div className="flex min-h-0 flex-1 flex-col">
        {threadTab === "lead" ? (
          <LeadTab onReview={() => setThreadTab("board")} />
        ) : reviewingProposal && proposal && proposal.status === "proposed" ? (
          <ScopeReview
            onBack={() => {
              setReviewingProposal(false);
              setThreadTab("lead");
            }}
          />
        ) : dirs.length === 0 ? (
          <EmptyDiscuss onTalk={() => setThreadTab("lead")} />
        ) : (
          <div className="min-h-0 flex-1 overflow-auto">
            <div className="flex h-full min-w-fit gap-3 px-5 py-4">
              {COLUMNS.map((col) => {
                const cards = dirs.filter((d) => statusOf(d) === col.key);
                return (
                  <div key={col.key} className="flex w-[300px] shrink-0 flex-col gap-2">
                    <div className="flex items-center gap-2 px-1 text-[11px] font-semibold uppercase tracking-wider text-ink-faint">
                      <span className={cn("h-1.5 w-1.5 rounded-full", col.dot)} />
                      {t(col.label)}
                      <span className="tabular-nums text-ink-faint/70">{cards.length}</span>
                    </div>
                    <div
                      className="flex min-h-0 flex-1 flex-col gap-2 rounded-[var(--radius-lg)] bg-surface/40 p-2"
                    >
                      {cards.map((d) => (
                        <div key={d.id}>
                          <DirectionCard direction={d} />
                        </div>
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
          </div>
        )}
      </div>

    </section>
  );
}

function EmptyDiscuss({ onTalk }: { onTalk: () => void }) {
  const { t } = useTranslation();
  return (
    <div className="flex h-full flex-col items-center justify-center px-6 text-center">
      <div className="grid h-11 w-11 place-items-center rounded-[var(--radius-lg)] border border-border bg-surface">
        <Layers size={20} className="text-ink-faint" />
      </div>
      <h2 className="mt-3 text-[14px] font-semibold text-ink">{t("thread.discussTitle")}</h2>
      <p className="mt-1.5 max-w-sm text-[12px] leading-relaxed text-ink-faint">
        {t("thread.discussBody")}
      </p>
      <Button variant="primary" className="mt-4" onClick={onTalk}>
        <MessagesSquare size={14} />
        {t("lead.title")}
      </Button>
    </div>
  );
}

function DirectionCard({ direction }: { direction: Direction }) {
  const {
    worktreesByDirection,
    repos,
    sessions,
    viewDirection,
    needs,
    asks,
    checksByDirection,
    requestSkillReview,
  } = useStore();
  const { t } = useTranslation();
  const writes = worktreesByDirection[direction.id] ?? [];
  const checks = checksByDirection[direction.id];
  const [showProvenance, setShowProvenance] = useState(false);
  const [reviewSent, setReviewSent] = useState(false);

  const allChecks = (checks ?? []).flatMap((rc) => rc.checks);
  const failed = allChecks.filter((c) => c.status === "fail").length;
  const passed = allChecks.filter((c) => c.status === "pass").length;
  const hasNeed =
    needs.some((n) => n.direction_id === direction.id) ||
    asks.some((a) => a.dir === String(direction.id));
  const firstWrite = writes[0];

  const testsKind =
    failed > 0 ? "fail" : allChecks.length > 0 && passed === allChecks.length ? "pass" : "pend";
  const action = hasNeed
    ? { label: t("thread.handle"), variant: "primary" as const }
    : direction.status === "review"
      ? { label: t("thread.reviewPr"), variant: "primary" as const }
      : { label: t("thread.openSession"), variant: "default" as const };

  return (
    <motion.div
      layout
      className={cn(
        "flex flex-col rounded-[var(--radius-lg)] border bg-surface text-left transition-colors hover:border-border-strong",
        hasNeed ? "border-waiting/45" : "border-border",
      )}
    >
      <div className="flex items-start gap-2 px-3 py-2.5">
        <ToolIcon tool={direction.tool} size={15} className="mt-0.5" />
        <div className="min-w-0 flex-1">
          <div className="truncate text-[13px] font-semibold leading-snug text-ink">
            {direction.name}
          </div>
          <div className="mt-1 flex flex-wrap items-center gap-1.5">
            <span className="rounded-full border border-border bg-bg px-1.5 py-0.5 text-[10.5px] text-ink-faint">
              {toolFullName(direction.tool)}
            </span>
            <span className="rounded-full border border-border bg-bg px-1.5 py-0.5 text-[10.5px] text-ink-faint">
              {taskStatusLabel(t, direction.status)}
            </span>
          </div>
        </div>
        <div className="ml-auto flex shrink-0 items-center gap-1.5">
          {hasNeed && (
            <span className="rounded-full bg-waiting/15 px-1.5 py-0.5 text-[10.5px] font-medium text-waiting">
              {t("thread.colNeeds")}
            </span>
          )}
          <StatusMenu direction={direction} />
        </div>
      </div>

      <div className="flex flex-wrap gap-1.5 px-3 pb-2">
        {writes.length === 0 ? (
          <span className="rounded-full border border-dashed border-border px-2 py-0.5 text-[11px] text-ink-faint">
            {t("thread.noWriteCopy")}
          </span>
        ) : (
          writes.map((w) => {
            const repo = repos.find((r) => r.id === w.repo_id);
            const sess = Object.values(sessions).find(
              (s) => s.directionId === direction.id && s.repoId === w.repo_id,
            );
            return (
              <button
                key={w.id}
                onClick={() => viewDirection(direction.id, w.repo_id)}
                className="inline-flex max-w-full items-center gap-1.5 rounded-full border border-accent/30 bg-accent-ghost px-2 py-0.5 text-[11px] text-accent transition-colors hover:border-accent/60"
              >
                <TerminalSquare size={11} className="shrink-0" />
                <span className="truncate font-mono">{repo?.name ?? `repo ${w.repo_id}`}</span>
                {sess && <StatusDot status={sess.status as SessionStatus} />}
              </button>
            );
          })
        )}
      </div>

      <div className="flex flex-wrap gap-1.5 border-y border-border px-3 py-2">
        <TrustSignal
          kind={testsKind}
          label={
            allChecks.length > 0
              ? t("thread.testsProgress", { passed, count: allChecks.length })
              : t("thread.testsPending")
          }
        />
        <TrustSignal
          kind={testsKind}
          label={failed > 0 ? t("thread.acceptFail", { count: failed }) : t("thread.typesSignal")}
        />
        <TrustSignal kind="pend" label={t("thread.contractSignal")} />
      </div>

      <div className="flex flex-wrap items-center gap-x-2 gap-y-1.5 px-3 py-2">
        <button
          type="button"
          onClick={() => setShowProvenance((open) => !open)}
          className="flex shrink-0 items-center gap-1 rounded-[var(--radius-sm)] px-1 py-0.5 text-[11px] text-ink-faint transition-colors hover:bg-brand-ghost hover:text-ink"
        >
          <ChevronRight
            size={13}
            className={cn("transition-transform", showProvenance && "rotate-90")}
          />
          {t("thread.provenance")}
          {writes.length > 0 && (
            <span className="text-ink-faint/70">
              · {t("thread.copiesCount", { count: writes.length })}
            </span>
          )}
        </button>
        <div className="ml-auto flex min-w-0 shrink-0 items-center gap-1.5">
          <button
            onClick={() => {
              setReviewSent(true);
              window.setTimeout(() => setReviewSent(false), 2500);
              void requestSkillReview(direction.id);
            }}
            disabled={writes.length === 0}
            title={t("thread.reviewTip")}
            className="flex shrink-0 items-center gap-1.5 whitespace-nowrap rounded-[var(--radius-sm)] px-1.5 py-1 text-[11px] text-ink-muted outline-none transition-colors hover:bg-brand-ghost hover:text-ink disabled:opacity-40"
          >
            {reviewSent ? (
              <Check size={12} className="text-running" />
            ) : (
              <ScanEye size={12} className="text-brand" />
            )}
            {reviewSent ? t("thread.reviewSent") : t("thread.review")}
          </button>
          <Button
            size="sm"
            variant={action.variant}
            disabled={!firstWrite}
            onClick={() => firstWrite && viewDirection(direction.id, firstWrite.repo_id)}
          >
            <TerminalSquare size={13} />
            {action.label}
          </Button>
        </div>
      </div>

      {showProvenance && (
        <div className="flex flex-col gap-1.5 border-t border-border bg-bg/35 px-3 py-2">
          {checks && checks.length > 0 ? (
            checks.map((rc) => <ChecksRow key={rc.repo} rc={rc} />)
          ) : (
            <div className="text-[11px] text-ink-faint">{t("thread.noChecks")}</div>
          )}
          {writes.map((w) => {
            const sess = Object.values(sessions).find(
              (s) => s.directionId === direction.id && s.repoId === w.repo_id,
            );
            return (
              <div key={w.id} className="flex items-center gap-2 text-[11px] text-ink-faint">
                <span className="min-w-0 flex-1 truncate font-mono">{w.branch}</span>
                <Inspect
                  path={w.path}
                  branch={w.branch}
                  nativeId={sess?.nativeId}
                  tool={sess?.info.tool ?? direction.tool}
                  size={13}
                  className="h-6 w-6 shrink-0"
                />
              </div>
            );
          })}
        </div>
      )}
    </motion.div>
  );
}

type TFn = ReturnType<typeof useTranslation>["t"];

function taskStatusLabel(t: TFn, status: Direction["status"]) {
  if (status === "working") return t("thread.colRunning");
  if (status === "review") return t("thread.colReview");
  if (status === "done") return t("thread.colDone");
  return t("thread.colQueued");
}

type TrustKind = "pass" | "fail" | "pend";

function TrustSignal({ kind, label }: { kind: TrustKind; label: string }) {
  return (
    <span
      className={cn(
        "inline-flex max-w-full items-center gap-1 rounded-full px-1.5 py-0.5 text-[10.5px] font-medium",
        kind === "pass" && "bg-running/15 text-running",
        kind === "fail" && "bg-[oklch(0.64_0.2_25/0.15)] text-danger",
        kind === "pend" && "border border-border bg-bg text-ink-faint",
      )}
    >
      {kind === "pass" ? (
        <Check size={10} className="shrink-0" />
      ) : kind === "fail" ? (
        <X size={10} className="shrink-0" />
      ) : (
        <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-ink-faint/70" />
      )}
      <span className="truncate">{label}</span>
    </span>
  );
}

/** Keyboard/click path to restatus a task. Sets the stored status (§4.6);
 *  "needs" is weft-derived, so it isn't offered. */
function StatusMenu({ direction }: { direction: Direction }) {
  const { setTaskStatus } = useStore();
  const { t } = useTranslation();
  const settable = COLUMNS.filter((c) => c.key !== "needs");
  const current = settable.find((c) => c.key === direction.status) ?? settable[0];
  return (
    <DM.Root>
      <DM.Trigger
        title={t("thread.setStatus")}
        aria-label={t("thread.setStatus")}
        onClick={(e) => e.stopPropagation()}
        className="flex items-center gap-1 rounded-full px-1.5 py-0.5 text-ink-faint outline-none transition-colors hover:bg-brand-ghost hover:text-ink data-[state=open]:bg-brand-ghost data-[state=open]:text-ink"
      >
        <span className={cn("h-2 w-2 rounded-full", current.dot)} />
        <ChevronDown size={11} />
      </DM.Trigger>
      <DM.Portal>
        <DM.Content
          align="end"
          sideOffset={4}
          onClick={(e) => e.stopPropagation()}
          className="weft-pop z-[60] w-40 rounded-[var(--radius-md)] border border-border bg-raised p-1 shadow-[0_8px_24px_-8px_rgba(0,0,0,0.5)]"
        >
          {settable.map((c) => (
            <DM.Item
              key={c.key}
              onSelect={() => void setTaskStatus(direction.id, c.key)}
              className="flex cursor-pointer items-center gap-2 rounded-[var(--radius-sm)] px-2 py-1.5 text-[12px] text-ink-muted outline-none data-[highlighted]:bg-brand-ghost data-[highlighted]:text-ink"
            >
              <span className={cn("h-1.5 w-1.5 rounded-full", c.dot)} />
              {t(c.label)}
              {c.key === current.key && <Check size={12} className="ml-auto text-brand" />}
            </DM.Item>
          ))}
        </DM.Content>
      </DM.Portal>
    </DM.Root>
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
