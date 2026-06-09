import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { ArrowLeft, GitCompare, Play } from "lucide-react";
import { useStore } from "../state/store";
import { api } from "../lib/api";
import type { ObserveRef, SessionStatus } from "../lib/types";
import { Transcript } from "./Transcript";
import { DiffPanel } from "./DiffPanel";
import { StatusChip } from "../components/ui/StatusChip";
import { Button } from "../components/ui/Button";
import { Inspect } from "../components/Inspect";
import { RailToggle } from "../components/RailToggle";
import { ToolIcon } from "../components/ToolIcon";

export function ObserveView() {
  const {
    viewing,
    closeObserve,
    driveDirection,
    repos,
    directionsByThread,
    sessions,
    needs,
    answerAsk,
  } = useStore();
  const { t } = useTranslation();
  const [ref, setRef] = useState<ObserveRef | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [driveError, setDriveError] = useState<string | null>(null);
  const [showDiff, setShowDiff] = useState(false);
  const [driving, setDriving] = useState(false);

  const directionId = viewing?.directionId ?? null;
  const repoId = viewing?.repoId ?? null;

  useEffect(() => {
    setShowDiff(false);
    if (directionId == null || repoId == null) {
      setRef(null);
      return;
    }
    let alive = true;
    const load = () =>
      api
        .sessionFor(directionId, repoId)
        .then((r) => {
          if (alive) {
            setRef(r);
            setLoadError(null);
          }
        })
        .catch((e: unknown) => {
          if (alive) setLoadError(String(e));
        });
    void load();
    const h = setInterval(load, 2000);
    return () => {
      alive = false;
      clearInterval(h);
    };
  }, [directionId, repoId]);

  if (viewing == null) return null;

  const liveSession = Object.values(sessions).find(
    (s) => s.directionId === directionId && s.repoId === repoId && s.status !== "exited",
  );
  const repoName = repos.find((r) => r.id === repoId)?.name ?? "working copy";
  const dirName =
    Object.values(directionsByThread)
      .flat()
      .find((d) => d.id === directionId)?.name ?? "task";
  const openAsks = needs.filter((n) => n.direction_id === directionId);

  // Label: attach (live) → continue (has native id) → start (never dispatched).
  const driveLabel = liveSession
    ? t("observe.attach")
    : ref?.native_id
      ? t("observe.continue")
      : t("observe.start");
  const uiStatus: SessionStatus =
    (liveSession?.status as SessionStatus) ??
    (ref?.status === "running" ? "running" : "idle");

  const onDrive = async () => {
    if (directionId == null || repoId == null) return;
    setDriving(true);
    setDriveError(null);
    try {
      await driveDirection(directionId, repoId, true);
    } catch (e) {
      setDriveError(String(e));
    } finally {
      setDriving(false);
    }
  };

  return (
    <div className="flex min-w-0 flex-1">
      <section className="flex min-w-0 flex-1 flex-col bg-bg">
        <header className="flex items-center gap-3 border-b border-border bg-surface px-3 py-2">
          <RailToggle />
          <button
            onClick={closeObserve}
            aria-label={t("session.back")}
            className="-ml-1 grid h-7 w-7 place-items-center rounded-[var(--radius-md)] text-ink-faint transition-colors hover:bg-brand-ghost hover:text-ink"
          >
            <ArrowLeft size={15} />
          </button>
          {ref && (
            <span className="flex shrink-0 items-center gap-1.5 whitespace-nowrap rounded-[var(--radius-sm)] bg-raised px-2 py-0.5 text-[11px] font-medium capitalize text-ink-muted">
              <ToolIcon tool={ref.tool} size={12} />
              {ref.tool}
            </span>
          )}
          <span className="flex min-w-0 items-center gap-1.5 text-[13px] text-ink">
            <span className="truncate font-medium">{repoName}</span>
            <span className="text-ink-faint">·</span>
            <span className="truncate text-ink-muted">{dirName}</span>
          </span>

          <div className="ml-auto flex shrink-0 items-center gap-2">
            {ref && (
              <button
                onClick={() => setShowDiff(true)}
                title={t("diff.tab")}
                aria-label={t("diff.tab")}
                className="grid h-7 w-7 shrink-0 place-items-center rounded-[var(--radius-md)] border border-border text-ink-muted transition-colors hover:bg-surface hover:text-ink"
              >
                <GitCompare size={13} />
              </button>
            )}
            <StatusChip status={uiStatus} />
            <Button size="sm" variant="primary" disabled={driving} onClick={() => void onDrive()}>
              <Play size={11} />
              {driveLabel}
            </Button>
            {ref && (
              <Inspect
                path={ref.worktree}
                branch={ref.branch}
                nativeId={ref.native_id}
                tool={ref.tool}
                className="h-7 w-7 shrink-0"
              />
            )}
          </div>
        </header>

        {driveError && (
          <div className="border-b border-border bg-[oklch(0.64_0.2_25/0.12)] px-3 py-1.5 text-[12px] text-danger">
            {t("observe.driveFailed")}: {driveError}
          </div>
        )}

        {openAsks.length > 0 && (
          <div className="border-b border-border bg-surface/60 px-3 py-2">
            {openAsks.map((a) => (
              <AskInline key={a.ask_id} text={a.text} onAnswer={(txt) => void answerAsk(a, txt)} />
            ))}
          </div>
        )}

        {ref ? (
          <Transcript cwd={ref.worktree} tool={ref.tool} running={!!liveSession} />
        ) : (
          <div className="grid flex-1 place-items-center text-[13px] text-ink-faint">
            {loadError ?? t("observe.empty")}
          </div>
        )}
      </section>

      {ref && (
        <DiffPanel cwd={ref.worktree} open={showDiff} onClose={() => setShowDiff(false)} />
      )}
    </div>
  );
}

function AskInline({ text, onAnswer }: { text: string; onAnswer: (answer: string) => void }) {
  const { t } = useTranslation();
  const [val, setVal] = useState("");
  return (
    <div className="flex items-center gap-2 py-1">
      <span className="min-w-0 flex-1 truncate text-[13px] text-ink">{text}</span>
      <input
        value={val}
        onChange={(e) => setVal(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && val.trim()) {
            onAnswer(val.trim());
            setVal("");
          }
        }}
        placeholder={t("observe.answerPlaceholder")}
        className="w-64 rounded-[var(--radius-sm)] border border-border bg-bg px-2 py-1 text-[12px] text-ink"
      />
    </div>
  );
}
