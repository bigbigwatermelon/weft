import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { ArrowRight, ChevronRight, Slash, Sparkles } from "lucide-react";
import type { LeadMessage } from "../lib/types";
import { Markdown } from "../components/Markdown";
import { cn } from "../lib/cn";
import { compactToolTarget, toolIcon, toolLabel } from "./transcriptBits";

/**
 * The chat-engine timeline: renders weft-owned LeadMessage rows (no polling,
 * no jsonl). Structured cards (proposal/approval/worker events) live inline in
 * the flow, where they happened — the conversation IS the console.
 */
export function ChatTimeline({
  messages,
  busy,
  onReviewProposal,
}: {
  messages: LeadMessage[];
  busy: boolean;
  onReviewProposal: () => void;
}) {
  const { t } = useTranslation();
  const endRef = useRef<HTMLDivElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const atBottomRef = useRef(true);
  const lastTextLen = messages
    .filter((m) => m.kind === "text")
    .reduce((n, m) => n + m.content.length, 0);

  // Stick to the bottom while the user is already there (streaming included);
  // never yank them down when they've scrolled up to read history.
  useEffect(() => {
    if (atBottomRef.current) endRef.current?.scrollIntoView({ block: "end" });
  }, [messages.length, lastTextLen, busy]);

  const onScroll = () => {
    const el = scrollRef.current;
    if (el) atBottomRef.current = el.scrollHeight - el.scrollTop - el.clientHeight < 80;
  };

  const visible = messages.filter((m) => m.kind !== "meta");

  if (visible.length === 0) {
    return (
      <div className="flex flex-1 items-center justify-center px-6 text-center">
        <p className="text-[12px] leading-relaxed text-ink-faint">
          {t("lead.transcriptEmpty")}
        </p>
      </div>
    );
  }

  return (
    <div
      ref={scrollRef}
      onScroll={onScroll}
      className="flex min-h-0 flex-1 flex-col gap-2.5 overflow-y-auto px-4 py-4"
    >
      <div className="mx-auto flex w-full max-w-[820px] flex-col gap-2.5">
        {visible.map((m) => (
          <TimelineRow key={m.id} m={m} onReviewProposal={onReviewProposal} />
        ))}
        {busy && (
          <div className="flex items-center gap-1.5 px-1 text-[11px] text-ink-faint">
            <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-running" />
            {t("lead.working")}
          </div>
        )}
      </div>
      <div ref={endRef} />
    </div>
  );
}

function parse(content: string): Record<string, unknown> {
  try {
    return JSON.parse(content) as Record<string, unknown>;
  } catch {
    return {};
  }
}

function TimelineRow({
  m,
  onReviewProposal,
}: {
  m: LeadMessage;
  onReviewProposal: () => void;
}) {
  const { t } = useTranslation();
  const c = parse(m.content);

  if (m.kind === "tool") {
    const name = String(c.name ?? "tool");
    const Icon = toolIcon(name);
    const { target, added, removed } = compactToolTarget(name, String(c.summary ?? ""));
    return (
      <div className="flex max-w-full items-center gap-2 px-1.5 py-1 text-[13px] text-ink-faint">
        <Icon size={15} className="shrink-0 text-ink-faint" />
        <span className="shrink-0 font-medium text-ink-muted">{toolLabel(name)}</span>
        <span className="min-w-0 truncate font-mono text-brand">{target}</span>
        {added && <span className="shrink-0 font-mono text-running">+{added}</span>}
        {removed && <span className="shrink-0 font-mono text-danger">-{removed}</span>}
        <ChevronRight size={15} className="shrink-0 text-ink-faint/70" />
      </div>
    );
  }

  if (m.kind === "command") {
    return (
      <div className="flex justify-end">
        <span
          className={cn(
            "inline-flex max-w-[72%] items-center gap-1.5 rounded-[var(--radius-md)] border border-brand/25 bg-brand-ghost px-3 py-2 font-mono text-[12.5px] text-ink",
            m.status === "queued" && "opacity-60",
          )}
        >
          <Slash size={12} className="shrink-0 text-brand" />
          <span className="truncate">
            {String(c.command ?? "")} {String(c.args ?? "")}
          </span>
          {m.status === "queued" && <QueuedChip />}
        </span>
      </div>
    );
  }

  if (m.kind === "proposal") {
    const count = Number(c.count ?? 0);
    return (
      <button
        onClick={onReviewProposal}
        className="group flex items-center gap-2.5 rounded-[var(--radius-md)] border border-accent/40 bg-accent-ghost px-3 py-2.5 text-left transition-colors hover:border-accent/70"
      >
        <Sparkles size={15} className="shrink-0 text-accent" />
        <div className="min-w-0 flex-1">
          <p className="text-[12.5px] font-medium text-ink">
            {t("lead.proposalReady", { count })}
          </p>
          <p className="truncate text-[11px] text-ink-muted">
            {String(c.rationale ?? "") || t("lead.reviewCreate")}
          </p>
        </div>
        <span className="flex shrink-0 items-center gap-1 text-[11px] font-medium text-accent">
          {t("lead.reviewCreate")}
          <ArrowRight size={12} className="transition-transform group-hover:translate-x-0.5" />
        </span>
      </button>
    );
  }

  if (m.role === "user") {
    return (
      <div className="flex justify-end">
        <p
          className={cn(
            "max-w-[72%] whitespace-pre-wrap break-words rounded-[var(--radius-lg)] border border-brand/25 bg-brand-ghost px-3.5 py-2.5 text-[13px] leading-relaxed text-ink",
            m.status === "queued" && "opacity-60",
          )}
        >
          {String(c.text ?? "")}
          {m.status === "queued" && (
            <span className="ml-2 inline-flex align-middle">
              <QueuedChip />
            </span>
          )}
        </p>
      </div>
    );
  }

  // assistant / system text
  return (
    <div className="flex items-start gap-2.5">
      <span className="mt-0.5 grid h-7 w-7 shrink-0 place-items-center rounded-[var(--radius-md)] bg-brand-ghost text-brand">
        <Sparkles size={14} />
      </span>
      <div className="min-w-0 flex-1 rounded-[var(--radius-lg)] border border-border bg-surface px-3.5 py-3 shadow-[0_12px_34px_-28px_rgba(0,0,0,0.65)]">
        <Markdown text={String(c.text ?? "")} />
        {m.status === "streaming" && (
          <span className="ml-0.5 inline-block h-3.5 w-[2px] animate-pulse rounded bg-brand align-text-bottom" />
        )}
        {m.status === "interrupted" && (
          <p className="mt-1.5 text-[11px] text-waiting">{t("lead.interrupted")}</p>
        )}
        {m.status === "error" && (
          <p className="mt-1.5 text-[11px] text-danger">{t("lead.errored")}</p>
        )}
      </div>
    </div>
  );
}

function QueuedChip() {
  const { t } = useTranslation();
  return (
    <span className="rounded-full bg-bg px-1.5 py-px text-[10px] text-ink-faint">
      {t("lead.queuedChip")}
    </span>
  );
}
