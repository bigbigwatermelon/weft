import { useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { useTranslation } from "react-i18next";
import {
  ArrowRight,
  MessagesSquare,
  RotateCcw,
  Sparkles,
  Square,
  SquareTerminal,
} from "lucide-react";
import { useStore } from "../state/store";
import type { SessionStatus } from "../lib/types";
import { TerminalPanel } from "../panels/TerminalPanel";
import { Transcript } from "./Transcript";
import { StatusChip } from "../components/ui/StatusChip";
import { Button } from "../components/ui/Button";
import { Inspect } from "../components/Inspect";
import { ResumeMenu } from "../components/ResumeMenu";
import { ToolIcon } from "../components/ToolIcon";
import { cn } from "../lib/cn";

/**
 * The thread's lead conversation as a full tab, mirroring a worker session:
 * observe (Chat) by default, switch to interactive (Terminal). When the lead
 * proposes a plan, a card surfaces here; reviewing happens on the Board tab.
 */
export function LeadTab({ onReview }: { onReview: () => void }) {
  const { leadSession, killSession, proposal, reviewingProposal, setReviewingProposal } =
    useStore();
  const { t } = useTranslation();
  const [view, setView] = useState<"chat" | "terminal">("chat");

  if (!leadSession) return <LeadStart />;

  const { info, status, nativeId } = leadSession;
  const running = status === "running" || status === "starting";
  const proposalPending =
    proposal?.status === "proposed" &&
    proposal.directions.length > 0 &&
    !reviewingProposal;

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-center gap-2 border-b border-border bg-surface px-3 py-2">
        <span className="flex items-center gap-1.5 rounded-[var(--radius-sm)] bg-raised px-2 py-0.5 text-[11px] capitalize text-ink-muted">
          <ToolIcon tool={info.tool} size={12} />
          {info.tool}
        </span>
        <div className="ml-auto flex items-center gap-2">
          <div className="flex items-center rounded-[var(--radius-md)] bg-bg p-0.5">
            <ViewTab active={view === "chat"} onClick={() => setView("chat")} title={t("lead.viewChat")}>
              <MessagesSquare size={13} />
            </ViewTab>
            <ViewTab active={view === "terminal"} onClick={() => setView("terminal")} title={t("lead.viewTerminal")}>
              <SquareTerminal size={13} />
            </ViewTab>
          </div>
          <StatusChip status={status as SessionStatus} />
          {running ? (
            <Button size="sm" variant="danger" onClick={() => void killSession(info.session_id)}>
              <Square size={11} />
              {t("session.kill")}
            </Button>
          ) : nativeId ? (
            <ResumeMenu
              tool={info.tool}
              cwd={info.worktree}
              nativeId={nativeId}
              trigger={
                <button className="flex h-7 items-center gap-1.5 rounded-[var(--radius-md)] border border-border px-2.5 text-[12px] text-ink-muted transition-colors hover:bg-surface hover:text-ink">
                  <RotateCcw size={12} />
                  {t("session.resumeMenu")}
                </button>
              }
            />
          ) : null}
          <Inspect path={info.worktree} nativeId={nativeId} className="h-7 w-7" />
        </div>
      </div>

      <AnimatePresence>
        {proposalPending && (
          <motion.button
            initial={{ opacity: 0, y: -6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -6 }}
            transition={{ duration: 0.18 }}
            onClick={() => {
              setReviewingProposal(true);
              onReview();
            }}
            className="group mx-3 mt-3 flex items-center gap-2.5 rounded-[var(--radius-md)] border border-accent/40 bg-accent-ghost px-3 py-2.5 text-left transition-colors hover:border-accent/70"
          >
            <Sparkles size={15} className="shrink-0 text-accent" />
            <div className="min-w-0 flex-1">
              <p className="text-[12.5px] font-medium text-ink">
                {t("lead.proposalReady", { count: proposal!.directions.length })}
              </p>
              <p className="truncate text-[11px] text-ink-muted">
                {proposal!.rationale || t("lead.reviewCreate")}
              </p>
            </div>
            <span className="flex shrink-0 items-center gap-1 text-[11px] font-medium text-accent">
              {t("lead.reviewCreate")}
              <ArrowRight size={12} className="transition-transform group-hover:translate-x-0.5" />
            </span>
          </motion.button>
        )}
      </AnimatePresence>

      {view === "chat" ? (
        <Transcript cwd={info.worktree} tool={info.tool} running={running} />
      ) : (
        <motion.div
          key={info.session_id}
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{ duration: 0.16 }}
          className="min-h-0 flex-1 p-1.5"
        >
          <TerminalPanel sessionId={info.session_id} />
        </motion.div>
      )}
    </div>
  );
}

function ViewTab({
  active,
  onClick,
  title,
  children,
}: {
  active: boolean;
  onClick: () => void;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      title={title}
      aria-label={title}
      aria-pressed={active}
      className={cn(
        "grid h-6 w-7 place-items-center rounded-[var(--radius-sm)] transition-colors",
        active ? "bg-raised text-ink shadow-[0_1px_2px_rgba(0,0,0,0.3)]" : "text-ink-faint hover:text-ink-muted",
      )}
    >
      {children}
    </button>
  );
}

function LeadStart() {
  const { startLead } = useStore();
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);
  return (
    <div className="flex flex-1 flex-col items-center justify-center px-6 text-center">
      <div className="grid h-11 w-11 place-items-center rounded-[var(--radius-lg)] bg-accent-ghost">
        <Sparkles size={20} className="text-accent" />
      </div>
      <h2 className="mt-3 text-[14px] font-semibold text-ink">{t("lead.startTitle")}</h2>
      <p className="mt-1.5 max-w-[320px] text-[12px] leading-relaxed text-ink-faint">
        {t("lead.startBody")}
      </p>
      <Button
        variant="primary"
        className="mt-4"
        disabled={busy}
        onClick={async () => {
          setBusy(true);
          try {
            await startLead();
          } finally {
            setBusy(false);
          }
        }}
      >
        <Sparkles size={14} />
        {busy ? t("lead.starting") : t("lead.start")}
      </Button>
    </div>
  );
}
