import { useState } from "react";
import { motion } from "motion/react";
import { useTranslation } from "react-i18next";
import { AlertTriangle, ArrowLeft, FolderGit2, Layers, Sparkles } from "lucide-react";
import { useStore } from "../state/store";
import type { ResolvedDirection } from "../lib/types";
import { Button } from "../components/ui/Button";
import { ToolIcon } from "../components/ToolIcon";
import { cn } from "../lib/cn";

const TOOL_LABEL: Record<string, string> = {
  claude: "Claude",
  codex: "Codex",
  opencode: "OpenCode",
};

/**
 * The scope-confirm step (§5 M5): the lead proposed a split of the Task into
 * sub-tasks, each scoped to the repos it writes. The human reviews it here and
 * confirms — only then are worktrees created and workers dispatched. This is the
 * one human gate weft keeps (the cross-repo blast-radius decision), not an
 * approval treadmill.
 */
export function ScopeReview({ onBack }: { onBack: () => void }) {
  const { proposal, confirmProposal } = useStore();
  const { t } = useTranslation();
  const [confirming, setConfirming] = useState(false);
  if (!proposal) return null;
  const dirs = proposal.directions;

  async function confirm() {
    setConfirming(true);
    try {
      await confirmProposal();
    } finally {
      setConfirming(false);
    }
  }

  return (
    <div className="min-h-0 flex-1 overflow-auto">
      <div className="mx-auto flex max-w-2xl flex-col gap-4 px-6 py-6">
        <div className="flex items-start gap-3">
          <div className="grid h-9 w-9 shrink-0 place-items-center rounded-[var(--radius-lg)] bg-accent-ghost">
            <Sparkles size={17} className="text-accent" />
          </div>
          <div className="min-w-0">
            <h1 className="text-[15px] font-semibold tracking-tight text-ink">
              {t("scope.title")}
            </h1>
            <p className="mt-0.5 text-[12px] leading-relaxed text-ink-faint">
              {t("scope.subtitle")}
            </p>
          </div>
        </div>

        {proposal.rationale && (
          <div className="rounded-[var(--radius-lg)] border border-border bg-surface/60 px-3.5 py-3">
            <div className="mb-1 text-[10.5px] font-semibold uppercase tracking-wide text-ink-faint">
              {t("scope.rationale")}
            </div>
            <p className="text-[12.5px] leading-relaxed text-ink-muted">
              {proposal.rationale}
            </p>
          </div>
        )}

        <div className="flex flex-col gap-2.5">
          {dirs.map((d, i) => (
            <DirectionRow key={i} direction={d} index={i} />
          ))}
        </div>

        <div className="sticky bottom-0 -mx-6 mt-1 flex items-center justify-between gap-3 border-t border-border bg-bg/90 px-6 py-3 backdrop-blur">
          <button
            onClick={onBack}
            className="flex items-center gap-1.5 rounded-[var(--radius-md)] px-2.5 py-1.5 text-[12.5px] text-ink-muted transition-colors hover:bg-surface hover:text-ink"
          >
            <ArrowLeft size={13} />
            {t("scope.back")}
          </button>
          <Button variant="primary" onClick={() => void confirm()} disabled={confirming || dirs.length === 0}>
            <Layers size={14} />
            {confirming ? t("scope.confirming") : t("scope.confirm", { count: dirs.length })}
          </Button>
        </div>
      </div>
    </div>
  );
}

function DirectionRow({ direction, index }: { direction: ResolvedDirection; index: number }) {
  const { t } = useTranslation();
  return (
    <motion.div
      initial={{ opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.2, delay: index * 0.03 }}
      className="flex flex-col rounded-[var(--radius-lg)] border border-border bg-surface"
    >
      <div className="flex items-center gap-2 border-b border-border px-3.5 py-2.5">
        <Layers size={14} className="shrink-0 text-ink-faint" />
        <span className="min-w-0 flex-1 truncate text-[13px] font-medium text-ink">
          {direction.name}
        </span>
        <span className="flex shrink-0 items-center gap-1.5 rounded-full bg-raised px-2 py-0.5 text-[11px] text-ink-muted">
          <ToolIcon tool={direction.tool} size={12} />
          {TOOL_LABEL[direction.tool] ?? direction.tool}
        </span>
      </div>
      <div className="flex flex-wrap items-center gap-1.5 px-3.5 py-2.5">
        <span className="mr-0.5 text-[10.5px] font-semibold uppercase tracking-wide text-ink-faint">
          {t("scope.writes")}
        </span>
        {direction.writes.length === 0 ? (
          <span className="text-[12px] text-ink-faint">{t("scope.noWrites")}</span>
        ) : (
          direction.writes.map((w) => (
            <span
              key={w.repo_name}
              title={w.known ? undefined : t("scope.unknownRepo")}
              className={cn(
                "flex items-center gap-1 rounded-full px-2 py-0.5 text-[11.5px] font-medium",
                w.known
                  ? "bg-brand-ghost text-brand"
                  : "bg-waiting/15 text-waiting",
              )}
            >
              {w.known ? <FolderGit2 size={11} /> : <AlertTriangle size={11} />}
              {w.repo_name}
            </span>
          ))
        )}
      </div>
    </motion.div>
  );
}
