import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { ArrowRight, Check } from "lucide-react";
import { Button } from "./ui/Button";
import { useStore } from "../state/store";
import { cn } from "../lib/cn";

const STORAGE_KEY = "weft-first-run-onboarding-v2-dismissed";

export function FirstRunOnboarding() {
  const { workspaces, createWorkspace } = useStore();
  const { t } = useTranslation();
  const [ready, setReady] = useState(false);
  const [dismissed, setDismissed] = useState(() => localStorage.getItem(STORAGE_KEY) === "1");
  const [step, setStep] = useState(0);
  const [workspaceName, setWorkspaceName] = useState(t("onboarding.defaultWorkspaceName"));
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const steps = t("onboarding.steps", { returnObjects: true }) as string[];
  const open = ready && workspaces.length === 0 && !dismissed;
  const last = step === steps.length - 1;

  useEffect(() => {
    const timer = window.setTimeout(() => setReady(true), 300);
    return () => window.clearTimeout(timer);
  }, []);

  function dismiss() {
    localStorage.setItem(STORAGE_KEY, "1");
    setDismissed(true);
  }

  async function enter() {
    if (busy) return;
    setBusy(true);
    setErr(null);
    try {
      const name = workspaceName.trim();
      if (name) await createWorkspace(name);
      dismiss();
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  }

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex flex-col bg-bg text-ink">
      <div className="flex h-[54px] shrink-0 items-center gap-3 border-b border-border px-5">
        <button
          className="flex items-center gap-2 rounded-[var(--radius-md)] px-1 py-1 text-[13px] font-semibold text-ink"
          onClick={() => setStep(0)}
        >
          <img src="/weft-mark.svg" alt="" className="h-[18px] w-[18px]" draggable={false} />
          Weft
          <span className="text-[12px] font-normal text-ink-faint">{t("onboarding.kicker")}</span>
        </button>
        <div className="mx-auto hidden min-w-0 items-center gap-2 lg:flex">
          {steps.map((label, i) => (
            <button
              key={label}
              onClick={() => setStep(i)}
              className={cn(
                "flex min-w-0 items-center gap-2 rounded-[var(--radius-sm)] px-1.5 py-1 text-[12px] transition-colors",
                i === step ? "text-ink" : i < step ? "text-ink-muted" : "text-ink-faint",
              )}
            >
              <span
                className={cn(
                  "grid h-5 w-5 shrink-0 place-items-center rounded-full border font-mono text-[10px]",
                  i === step
                    ? "border-brand bg-brand text-brand-ink"
                    : i < step
                      ? "border-brand/60 bg-brand-ghost text-brand"
                      : "border-border bg-surface text-ink-faint",
                )}
              >
                {i < step ? <Check size={11} /> : i + 1}
              </span>
              <span className="truncate">{label}</span>
            </button>
          ))}
        </div>
        <button
          onClick={dismiss}
          className="ml-auto rounded-[var(--radius-md)] px-3 py-1.5 text-[12.5px] font-medium text-ink-muted transition-colors hover:bg-brand-ghost hover:text-ink lg:ml-0"
        >
          {t("onboarding.skip")}
        </button>
      </div>

      <main className="min-h-0 flex-1 overflow-y-auto">
        <div className="mx-auto flex min-h-full w-full max-w-[900px] items-center justify-center px-6 py-10">
          <OnboardingStage
            step={step}
            workspaceName={workspaceName}
            setWorkspaceName={setWorkspaceName}
          />
        </div>
      </main>

      <div className="flex h-[58px] shrink-0 items-center gap-3 border-t border-border bg-surface px-5">
        <Button variant="default" onClick={() => setStep((s) => Math.max(s - 1, 0))} disabled={step === 0}>
          {t("onboarding.back")}
        </Button>
        {err && <span className="max-w-[360px] truncate text-[12px] text-danger">{err}</span>}
        <span className="ml-auto text-[12px] tabular-nums text-ink-faint">
          {step + 1} / {steps.length}
        </span>
        {last ? (
          <button
            onClick={() => void enter()}
            disabled={busy}
            className="inline-flex h-8 items-center gap-1.5 rounded-[var(--radius-md)] border border-accent bg-accent px-3 text-[13px] font-medium text-[oklch(0.18_0.02_40)] transition-[filter,opacity] hover:brightness-105 disabled:pointer-events-none disabled:opacity-45"
          >
            <ArrowRight size={14} />
            {busy ? t("dialog.creating") : t("onboarding.enterWorkspace")}
          </button>
        ) : (
          <Button variant="primary" onClick={() => setStep((s) => Math.min(s + 1, steps.length - 1))}>
            {step === 0 ? t("onboarding.start") : t("onboarding.next")}
            <ArrowRight size={14} />
          </Button>
        )}
      </div>
    </div>
  );
}

function OnboardingStage({
  step,
  workspaceName,
  setWorkspaceName,
}: {
  step: number;
  workspaceName: string;
  setWorkspaceName: (v: string) => void;
}) {
  const { t } = useTranslation();

  if (step === 0) {
    return (
      <section className="flex max-w-[440px] flex-col items-center text-center">
        <img src="/weft-mark.svg" alt="" className="h-14 w-14" draggable={false} />
        <h1 className="mt-5 text-[42px] font-semibold leading-none text-ink">Weft</h1>
        <p className="mt-5 text-[15px] text-ink-muted">{t("onboarding.heroSubtitle")}</p>
        <p className="mt-3 max-w-[42ch] text-[13px] leading-relaxed text-ink-faint">
          {t("onboarding.heroBody")}
        </p>
      </section>
    );
  }

  if (step === 1) {
    return (
      <section className="w-full max-w-[560px]">
        <h2 className="text-[17px] font-semibold text-ink">{t("onboarding.workspaceTitle")}</h2>
        <p className="mt-1 text-[12px] leading-relaxed text-ink-faint">
          {t("onboarding.workspaceBody")}
        </p>
        <label className="mt-5 flex flex-col gap-2">
          <span className="text-[10.5px] font-semibold uppercase tracking-wider text-ink-faint">
            {t("dialog.workspaceName")}
          </span>
          <input
            value={workspaceName}
            onChange={(e) => setWorkspaceName(e.currentTarget.value)}
            className="h-11 rounded-[var(--radius-md)] border border-border bg-bg px-3 text-[15px] text-ink outline-none transition-colors focus:border-brand focus:ring-2 focus:ring-brand/25"
          />
        </label>
      </section>
    );
  }

  if (step === 2) {
    return (
      <section className="w-full max-w-[560px] text-center">
        <h2 className="text-[17px] font-semibold text-ink">{t("onboarding.startTitle")}</h2>
        <p className="mx-auto mt-1 max-w-md text-[12px] leading-relaxed text-ink-faint">
          {t("onboarding.startBody")}
        </p>
      </section>
    );
  }

  return null;
}
