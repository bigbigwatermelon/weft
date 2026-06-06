import { motion } from "motion/react";
import { useTranslation } from "react-i18next";
import { ArrowLeft, RotateCcw, Square } from "lucide-react";
import { useStore } from "../state/store";
import type { SessionStatus } from "../lib/types";
import { TerminalPanel } from "../panels/TerminalPanel";
import { StatusChip } from "../components/ui/StatusChip";
import { Button } from "../components/ui/Button";
import { Inspect } from "../components/Inspect";
import { ToolIcon } from "../components/ToolIcon";

export function SessionView() {
  const {
    sessions,
    activeSessionId,
    resumeSession,
    killSession,
    backToBoard,
    repos,
    directionsByThread,
    activeThreadId,
  } = useStore();
  const { t } = useTranslation();
  const active = activeSessionId != null ? sessions[activeSessionId] : null;

  if (!active) return null;

  const { info, status, nativeId } = active;
  const isLead = active.kind === "lead";
  // Product words, not plumbing: "<repo> · <direction>". The real worktree
  // path / branch / native id live in Inspect (§4.7).
  const repoName =
    repos.find((r) => r.id === active.repoId)?.name ?? "working copy";
  const dirName =
    (activeThreadId != null ? directionsByThread[activeThreadId] : undefined)?.find(
      (d) => d.id === active.directionId,
    )?.name ?? "task";

  return (
    <section className="flex min-w-0 flex-1 flex-col bg-bg">
      {/* session header */}
      <header className="flex items-center gap-3 border-b border-border bg-surface px-3 py-2">
        <button
          onClick={backToBoard}
          aria-label={t("session.back")}
          className="-ml-1 grid h-7 w-7 place-items-center rounded-[var(--radius-md)] text-ink-faint transition-colors hover:bg-brand-ghost hover:text-ink"
        >
          <ArrowLeft size={15} />
        </button>
        <span className="flex items-center gap-1.5 rounded-[var(--radius-sm)] bg-raised px-2 py-0.5 text-[11px] font-medium capitalize text-ink-muted">
          <ToolIcon tool={info.tool} size={12} />
          {info.tool}
        </span>
        {isLead ? (
          <span className="flex items-center gap-1.5 text-[13px]">
            <span className="rounded-full bg-accent-ghost px-2 py-0.5 text-[11px] font-medium text-accent">
              {t("session.lead")}
            </span>
            <span className="text-ink-muted">{t("session.leadPlanning")}</span>
          </span>
        ) : (
          <span className="flex min-w-0 items-center gap-1.5 text-[13px] text-ink">
            <span className="truncate font-medium">{repoName}</span>
            <span className="text-ink-faint">·</span>
            <span className="truncate text-ink-muted">{dirName}</span>
          </span>
        )}

        <div className="ml-auto flex items-center gap-2">
          <StatusChip status={status as SessionStatus} />
          <Button
            size="sm"
            variant="default"
            onClick={() => void resumeSession(info.session_id)}
            disabled={!nativeId}
            title={nativeId ? t("session.resumeReady") : t("session.starting")}
          >
            <RotateCcw size={12} />
            {t("session.resume")}
          </Button>
          <Button
            size="sm"
            variant="danger"
            onClick={() => void killSession(info.session_id)}
          >
            <Square size={11} />
            {t("session.kill")}
          </Button>
          <Inspect
            path={info.worktree}
            branch={info.branch}
            nativeId={nativeId}
            className="h-7 w-7"
          />
        </div>
      </header>

      {/* embedded native TUI — keyed so each session gets a fresh terminal */}
      <motion.div
        key={info.session_id}
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ duration: 0.16 }}
        className="min-h-0 flex-1 p-1.5"
      >
        <TerminalPanel sessionId={info.session_id} />
      </motion.div>
    </section>
  );
}

