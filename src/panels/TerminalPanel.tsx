import { useEffect, useRef, useState } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import { MousePointerClick, Lock } from "lucide-react";
import { api } from "../lib/api";
import "@xterm/xterm/css/xterm.css";

function b64ToBytes(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

/**
 * Embeds a native CLI TUI for ONE session. Observe/drive are decoupled: the
 * terminal renders read-only by default (it never steals focus or keystrokes),
 * and you opt into interaction explicitly.
 *
 * - mode="interactive" (workers): read-only until you click "interact"; a small
 *   indicator lets you drop back to read-only.
 * - mode="readonly" (the lead): always read-only — the dock composer is the
 *   input, so the embedded TUI is purely a live view.
 *
 * On mount it fits, resizes, and NUDGES a repaint (resize ±1 row) so a TUI that
 * only redraws on SIGWINCH paints its current frame even on remount.
 */
export function TerminalPanel({
  sessionId,
  mode = "interactive",
}: {
  sessionId: number;
  mode?: "interactive" | "readonly";
}) {
  const hostRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const drivingRef = useRef(false);
  const [driving, setDriving] = useState(false);
  const { t } = useTranslation();
  const canDrive = mode === "interactive";

  useEffect(() => {
    const term = new Terminal({
      fontFamily: 'ui-monospace, "SF Mono", "JetBrains Mono", Menlo, monospace',
      fontSize: 12.5,
      lineHeight: 1.2,
      theme: {
        background: "#16151c",
        foreground: "#e9e8f2",
        cursor: "#8b7bff",
        selectionBackground: "#2c2747",
      },
      cursorBlink: false,
      disableStdin: true, // observe by default; enabled on "interact"
      scrollback: 8000,
      allowProposedApi: true,
    });
    termRef.current = term;
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(hostRef.current!);
    fit.fit();

    // Keystrokes only forward while driving.
    const dataSub = term.onData((data) => {
      if (drivingRef.current) void api.writePty(sessionId, data);
    });

    const pushResize = () => {
      try {
        fit.fit();
        void api.resizePty(sessionId, term.rows, term.cols);
      } catch {
        /* not ready */
      }
    };
    // Force a repaint: a TUI that only redraws on SIGWINCH (Ink / Bubble Tea)
    // paints a blank/stale frame after spawn-resize or remount otherwise.
    const nudge = () => {
      try {
        fit.fit();
        const { rows, cols } = term;
        if (rows > 1) {
          void api
            .resizePty(sessionId, rows - 1, cols)
            .then(() => api.resizePty(sessionId, rows, cols));
        }
      } catch {
        /* not ready */
      }
    };
    pushResize();
    const t1 = setTimeout(nudge, 150);
    const t2 = setTimeout(nudge, 500);

    const ro = new ResizeObserver(pushResize);
    ro.observe(hostRef.current!);

    const unOut = listen<{ session_id: number; data: string }>(
      "pty://output",
      (e) => {
        if (e.payload.session_id === sessionId)
          term.write(b64ToBytes(e.payload.data));
      },
    );

    return () => {
      clearTimeout(t1);
      clearTimeout(t2);
      dataSub.dispose();
      ro.disconnect();
      void unOut.then((f) => f());
      term.dispose();
      termRef.current = null;
    };
  }, [sessionId]);

  function setDrive(on: boolean) {
    drivingRef.current = on;
    setDriving(on);
    const term = termRef.current;
    if (!term) return;
    term.options.disableStdin = !on;
    term.options.cursorBlink = on;
    if (on) term.focus();
    else term.blur();
  }

  return (
    <div className="relative h-full">
      <div ref={hostRef} className="term-host h-full" />

      {canDrive && !driving && (
        <button
          onClick={() => setDrive(true)}
          className="group absolute inset-0 z-10 flex items-end justify-center bg-transparent pb-4 transition-colors hover:bg-[oklch(0.16_0.02_285/0.12)]"
        >
          <span className="flex items-center gap-1.5 rounded-full border border-border-strong bg-raised/90 px-3 py-1.5 text-[11px] font-medium text-ink-muted shadow-[0_4px_16px_-6px_rgba(0,0,0,0.6)] backdrop-blur transition-colors group-hover:text-ink">
            <MousePointerClick size={12} />
            {t("session.clickToInteract")}
          </span>
        </button>
      )}

      {canDrive && driving && (
        <button
          onClick={() => setDrive(false)}
          title={t("session.backToReadonly")}
          className="absolute right-2 top-2 z-10 flex items-center gap-1 rounded-full bg-raised/90 px-2 py-1 text-[10px] font-medium text-running backdrop-blur transition-colors hover:text-ink"
        >
          <Lock size={10} />
          {t("session.interacting")}
        </button>
      )}
    </div>
  );
}
