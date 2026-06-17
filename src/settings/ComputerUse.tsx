import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "../components/ui/Button";
import { Toggle } from "../components/ui/Toggle";
import { api } from "../lib/api";
import { cn } from "../lib/cn";
import type { ComputerUseStatus, ComputerUseStatusKind } from "../lib/types";

export function ComputerUseSettings() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<ComputerUseStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [runningDoctor, setRunningDoctor] = useState(false);
  const [output, setOutput] = useState<string | null>(null);

  async function refreshStatus({
    clearOutput = false,
    preserveOutputOnError = false,
  }: {
    clearOutput?: boolean;
    preserveOutputOnError?: boolean;
  } = {}) {
    if (clearOutput) setOutput(null);
    setLoading(true);
    try {
      setStatus(await api.computerUseGetStatus());
    } catch (err: unknown) {
      setStatus(null);
      if (!preserveOutputOnError) setOutput(String(err));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void refreshStatus();
  }, []);

  async function setEnabled(enabled: boolean) {
    if (saving) return;
    setSaving(true);
    setOutput(null);
    try {
      await api.computerUseSetEnabled(enabled);
      await refreshStatus();
    } catch (err: unknown) {
      setOutput(String(err));
      await refreshStatus();
    } finally {
      setSaving(false);
    }
  }

  const enabled = status?.enabled ?? false;
  const busy = loading || saving || runningDoctor;
  const canRunDoctor =
    Boolean(
      status?.enabled &&
        status.supported &&
        !DOCTOR_BLOCKED_STATUSES.has(status.status),
    ) && !busy;
  const visibleOutput = output ?? status?.doctor_summary ?? status?.error;

  async function runDoctor() {
    if (!canRunDoctor) return;
    setRunningDoctor(true);
    try {
      const text = await api.computerUseRunDoctor();
      setOutput(text);
      await refreshStatus();
    } catch (err: unknown) {
      setOutput(String(err));
      await refreshStatus({ preserveOutputOnError: true });
    } finally {
      setRunningDoctor(false);
    }
  }

  return (
    <div className="flex flex-col gap-10">
      <SettingsGroup title={t("settings.computerUseGroup")}>
        <SettingRow
          label={t("settings.computerUseEnable")}
          hint={t("settings.computerUseEnableHint")}
        >
          <div className={cn(saving && "pointer-events-none opacity-60")}>
            <Toggle
              on={enabled}
              onChange={(value) => void setEnabled(value)}
              label={t("settings.computerUseEnable")}
            />
          </div>
        </SettingRow>
        <div className="px-3 py-3">
          <p className="text-[12px] leading-relaxed text-ink-faint">
            {t("settings.computerUseTrustHint")}
          </p>
        </div>
      </SettingsGroup>

      <SettingsGroup title={t("settings.computerUseDiagnostics")}>
        <div className="flex flex-col gap-3 px-3 py-3">
          <DiagnosticLine label={t("settings.computerUseStatus")}>
            <StatusPill status={status?.status} loading={loading && !status} />
          </DiagnosticLine>
          <DiagnosticLine label={t("settings.computerUseHelperPath")}>
            <span className="block break-all font-mono text-[11px] text-ink-muted">
              {status?.helper_path ?? t("settings.computerUseNoHelper")}
            </span>
          </DiagnosticLine>
          <DiagnosticLine label={t("settings.computerUseVersion")}>
            <span className="font-mono text-[11px] text-ink-muted">
              {status?.helper_version ?? t("settings.computerUseUnknown")}
            </span>
          </DiagnosticLine>

          <div className="flex justify-end gap-2 pt-1">
            <Button
              variant="default"
              onClick={() => void refreshStatus({ clearOutput: true })}
              disabled={busy}
            >
              {loading ? t("settings.computerUseChecking") : t("settings.computerUseRecheck")}
            </Button>
            <Button
              variant="primary"
              onClick={() => void runDoctor()}
              disabled={!canRunDoctor}
            >
              {runningDoctor
                ? t("settings.computerUseChecking")
                : t("settings.computerUseRunDoctor")}
            </Button>
          </div>

          {visibleOutput && visibleOutput.trim().length > 0 && (
            <pre className="max-h-48 overflow-auto rounded-[var(--radius-md)] border border-border bg-bg p-3 whitespace-pre-wrap break-words font-mono text-[11px] leading-relaxed text-ink-muted">
              {visibleOutput}
            </pre>
          )}
        </div>
      </SettingsGroup>
    </div>
  );
}

const DOCTOR_BLOCKED_STATUSES = new Set<ComputerUseStatusKind>([
  "disabled",
  "unsupported_platform",
  "missing",
  "not_executable",
]);

function DiagnosticLine({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex items-start justify-between gap-4 text-[12px]">
      <span className="shrink-0 text-ink-faint">{label}</span>
      <div className="min-w-0 flex-1 text-right">{children}</div>
    </div>
  );
}

function StatusPill({
  status,
  loading,
}: {
  status?: ComputerUseStatusKind;
  loading: boolean;
}) {
  const { t } = useTranslation();
  const key = loading ? "computerUseChecking" : `computerUse_${status ?? "unknown"}`;
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-full border px-2 py-0.5 text-[11px] font-medium",
        statusTone(status, loading),
      )}
    >
      {t(`settings.${key}`)}
    </span>
  );
}

function statusTone(status?: ComputerUseStatusKind, loading = false): string {
  if (loading) return "border-border bg-bg text-ink-faint";
  if (status === "ready" || status === "found") {
    return "border-success/30 bg-success/15 text-success";
  }
  if (
    status === "disabled" ||
    status === "unsupported_platform" ||
    status === "permission_missing"
  ) {
    return "border-waiting/30 bg-waiting/15 text-waiting";
  }
  if (status === "missing" || status === "not_executable" || status === "doctor_failed") {
    return "border-danger/30 bg-danger/15 text-danger";
  }
  return "border-border bg-bg text-ink-faint";
}

function SettingsGroup({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-3">
      <h2 className="text-[13px] font-semibold text-ink">{title}</h2>
      <div
        className={cn(
          "flex flex-col rounded-[var(--radius-lg)] border border-border bg-surface",
          "[&>div+div]:border-t [&>div+div]:border-border",
        )}
      >
        {children}
      </div>
    </section>
  );
}

function SettingRow({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children?: ReactNode;
}) {
  return (
    <div className="flex min-h-[72px] items-center gap-4 px-3 py-3">
      <div className="min-w-0">
        <div className="text-[12.5px] font-semibold text-ink">{label}</div>
        {hint && <p className="mt-1 max-w-[58ch] text-[12px] leading-relaxed text-ink-faint">{hint}</p>}
      </div>
      <span className="min-w-4 flex-1" />
      {children && <div className="shrink-0">{children}</div>}
    </div>
  );
}
