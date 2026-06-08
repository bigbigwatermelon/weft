import { useEffect } from "react";
import { AnimatePresence, motion } from "motion/react";
import { useTranslation } from "react-i18next";
import { Check, Zap } from "lucide-react";
import { useStore } from "../state/store";

/**
 * The once-a-day nudge (§ permission UX): after granting broad access (Always
 * allow / Full access) without Dangerous mode on, a bottom toast offers to turn
 * it on in place. A second "enabled" toast confirms with an Undo.
 */
export function DangerToast() {
  const { dangerNudge, setDangerNudge, setDangerousMode } = useStore();
  const { t } = useTranslation();

  useEffect(() => {
    if (!dangerNudge) return;
    const h = setTimeout(() => setDangerNudge(null), dangerNudge === "ask" ? 8000 : 5000);
    return () => clearTimeout(h);
  }, [dangerNudge, setDangerNudge]);

  return (
    <AnimatePresence>
      {dangerNudge && (
        <motion.div
          key="danger-toast"
          initial={{ opacity: 0, y: 16 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: 16 }}
          transition={{ duration: 0.22, ease: [0.22, 1, 0.36, 1] }}
          className="fixed bottom-4 right-4 z-[100] w-[320px] overflow-hidden rounded-[var(--radius-lg)] border border-waiting/40 bg-raised shadow-[0_12px_40px_-10px_rgba(0,0,0,0.6)]"
        >
          {dangerNudge === "ask" ? (
            <div className="flex flex-col gap-2.5 p-3.5">
              <div className="flex items-center gap-2">
                <span className="grid h-6 w-6 shrink-0 place-items-center rounded-full bg-waiting/15">
                  <Zap size={13} className="text-waiting" />
                </span>
                <span className="text-[13px] font-semibold text-ink">{t("danger.nudgeTitle")}</span>
              </div>
              <p className="text-[12px] leading-relaxed text-ink-muted">{t("danger.nudgeBody")}</p>
              <div className="flex items-center justify-end gap-2">
                <button
                  onClick={() => setDangerNudge(null)}
                  className="rounded-[var(--radius-md)] px-2.5 py-1.5 text-[12px] text-ink-faint transition-colors hover:bg-brand-ghost hover:text-ink"
                >
                  {t("danger.notNow")}
                </button>
                <button
                  onClick={() => {
                    setDangerousMode(true);
                    setDangerNudge("enabled");
                  }}
                  className="flex items-center gap-1.5 rounded-[var(--radius-md)] bg-waiting/90 px-2.5 py-1.5 text-[12px] font-medium text-[#1a1206] transition-colors hover:bg-waiting"
                >
                  <Zap size={12} />
                  {t("danger.turnOn")}
                </button>
              </div>
            </div>
          ) : (
            <div className="flex items-center gap-2.5 p-3.5">
              <span className="grid h-6 w-6 shrink-0 place-items-center rounded-full bg-waiting/15">
                <Check size={13} className="text-waiting" />
              </span>
              <span className="flex-1 text-[12px] leading-snug text-ink-muted">
                {t("danger.enabled")}
              </span>
              <button
                onClick={() => {
                  setDangerousMode(false);
                  setDangerNudge(null);
                }}
                className="shrink-0 rounded-[var(--radius-md)] px-2 py-1 text-[12px] font-medium text-brand transition-colors hover:bg-brand-ghost"
              >
                {t("danger.undo")}
              </button>
            </div>
          )}
        </motion.div>
      )}
    </AnimatePresence>
  );
}
