import { useEffect } from "react";
import { AnimatePresence, motion } from "motion/react";
import { useStore } from "../state/store";
import type { Direction } from "../lib/types";
import { CoordinationPanel } from "./CoordinationPanel";

const EXPO = [0.16, 1, 0.3, 1] as const;

/**
 * The thread bus, demoted from a permanent rail to an on-demand right drawer:
 * agent↔agent coordination is observability, not a primary surface. Toggled from
 * the thread header; Esc or the backdrop closes it.
 */
export function BusDrawer({ directions }: { directions: Direction[] }) {
  const { showBus, setShowBus } = useStore();

  useEffect(() => {
    if (!showBus) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setShowBus(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [showBus, setShowBus]);

  return (
    <AnimatePresence>
      {showBus && (
        <div className="fixed inset-0 z-50 flex justify-end">
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.18 }}
            onClick={() => setShowBus(false)}
            className="absolute inset-0 bg-[oklch(0_0_0/0.4)]"
          />
          <motion.div
            initial={{ x: 32, opacity: 0 }}
            animate={{ x: 0, opacity: 1 }}
            exit={{ x: 32, opacity: 0 }}
            transition={{ duration: 0.22, ease: EXPO }}
            className="relative h-full"
          >
            <CoordinationPanel directions={directions} onClose={() => setShowBus(false)} />
          </motion.div>
        </div>
      )}
    </AnimatePresence>
  );
}
