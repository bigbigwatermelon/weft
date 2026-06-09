import { useEffect, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { listen } from "@tauri-apps/api/event";
import { api } from "../lib/api";
import "@xterm/xterm/css/xterm.css";

function b64ToBytes(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

/**
 * The interactive drive surface for ONE session — the raw native TUI. Read-only
 * "observe" is the Chat tab's job (the sidecar transcript), so this terminal is
 * always live: it takes focus and forwards keystrokes directly.
 *
 * On mount it fits, resizes, and NUDGES a repaint (resize ±1 row) so a TUI that
 * only redraws on SIGWINCH (Ink / Bubble Tea) paints its current frame even on
 * remount.
 */
export function TerminalPanel({ sessionId }: { sessionId: number }) {
  const hostRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const term = new Terminal({
      fontFamily: 'ui-monospace, "SF Mono", "JetBrains Mono", Menlo, monospace',
      fontSize: 12.5,
      lineHeight: 1.2,
      // LOOM: warm-graphite well, teal "warp" cursor. Always dark (TUIs assume dark).
      theme: {
        background: "#1a1814",
        foreground: "#f0eee8",
        cursor: "#54c0cc",
        selectionBackground: "#21343a",
      },
      cursorBlink: true,
      scrollback: 8000,
      allowProposedApi: true,
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(hostRef.current!);
    fit.fit();
    term.focus();

    const dataSub = term.onData((data) => {
      void api.writePty(sessionId, data);
    });

    const pushResize = () => {
      try {
        fit.fit();
        void api.resizePty(sessionId, term.rows, term.cols);
      } catch {
        /* not ready */
      }
    };
    // Force a repaint: a TUI that only redraws on SIGWINCH paints a blank/stale
    // frame after spawn-resize or remount otherwise.
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
    };
  }, [sessionId]);

  return <div ref={hostRef} className="term-host h-full" />;
}
