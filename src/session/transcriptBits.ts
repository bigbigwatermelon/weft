// Shared tool-event presentation bits for the transcript views (the legacy
// jsonl-projection Transcript and the chat-engine ChatTimeline).

import type { ComponentType } from "react";
import {
  FilePen,
  FileText,
  ListTodo,
  type LucideProps,
  Radio,
  Search,
  SquareTerminal,
  Wrench,
} from "lucide-react";

/** Map a (cleaned) tool name to a glyph so the pills are scannable. */
export function toolIcon(name: string): ComponentType<LucideProps> {
  const n = name.toLowerCase();
  if (/(bash|exec_command|shell|run)/.test(n)) return SquareTerminal;
  if (/(write|edit|apply_patch|patch)/.test(n)) return FilePen;
  if (/(grep|glob|rg|ripgrep|ls|find|list)/.test(n)) return Search;
  if (/read|view|cat/.test(n)) return FileText;
  if (/(bus_|broadcast|ask_human|announce|interface|inbox|status)/.test(n)) return Radio;
  if (/todo/.test(n)) return ListTodo;
  return Wrench;
}

export function toolLabel(name: string) {
  const n = name.toLowerCase();
  if (/(write|edit|apply_patch|patch)/.test(n)) return "正在编辑";
  if (/(read|view|cat)/.test(n)) return "正在读取";
  if (/(grep|glob|rg|ripgrep|ls|find|list|search)/.test(n)) return "正在搜索";
  if (/(bash|exec_command|shell|run)/.test(n)) return "正在运行";
  if (/(bus_|broadcast|ask_human|announce|interface|inbox|status)/.test(n)) return "正在同步";
  if (/todo/.test(n)) return "正在整理";
  return "正在调用";
}

/** Squeeze a tool call's target into a compact, scannable fragment. */
export function compactToolTarget(name: string, summary: string) {
  const raw = summary || name;
  const file =
    raw.match(/(?:^|[\\s"'`])([\\w./-]+\\.(?:tsx|ts|jsx|js|rs|css|md|json|toml|yaml|yml|html))/)?.[1] ??
    raw.match(/(?:^|[\\s"'`])([\\w./-]+\/[\\w./-]+)/)?.[1];
  const target = file ? file.split("/").slice(-2).join("/") : raw.replace(/\s+/g, " ").slice(0, 90);
  const added = raw.match(/(?:\+|added[:= ]+)(\d+)/i)?.[1];
  const removed = raw.match(/(?:-|removed[:= ]+)(\d+)/i)?.[1];
  return { target, added, removed };
}
