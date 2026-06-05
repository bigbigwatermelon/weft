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
 * Embeds the native Claude TUI for ONE session. Bidirectional: PTY output
 * (filtered by sessionId) renders here; keystrokes forward to that session's
 * stdin. Keys pass straight through — the full key-ownership table is M4.
 */
export function TerminalPanel({ sessionId }: { sessionId: number }) {
  const hostRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const term = new Terminal({
      fontFamily:
        'ui-monospace, "SF Mono", "JetBrains Mono", Menlo, monospace',
      fontSize: 12.5,
      lineHeight: 1.2,
      theme: {
        background: "#16151c",
        foreground: "#e9e8f2",
        cursor: "#8b7bff",
        selectionBackground: "#2c2747",
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
      dataSub.dispose();
      ro.disconnect();
      void unOut.then((f) => f());
      term.dispose();
    };
  }, [sessionId]);

  return <div ref={hostRef} className="term-host" />;
}
