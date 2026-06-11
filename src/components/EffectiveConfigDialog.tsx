import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { FileText, FolderGit2, Layers, Loader2 } from "lucide-react";
import { Dialog, DialogContent } from "./ui/Dialog";
import { useStore } from "../state/store";
import { api } from "../lib/api";
import type { ConfigItem, RepoRef } from "../lib/types";
import { cn } from "../lib/cn";

/**
 * Effective-config preview (M6 有效配置预览): for each repo in the workspace, the
 * skills + rules that actually apply, tagged with the layer they come from
 * (personal / repo) — a shadowed personal skill is shown struck through. Answers
 * "where does this skill/rule come from?" without leaving weft.
 */
export function EffectiveConfigDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
}) {
  const { repos } = useStore();
  const { t } = useTranslation();

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        title={t("config.title")}
        description={t("config.subtitle")}
        className="w-[min(560px,calc(100vw-2rem))]"
      >
        <div className="flex max-h-[60vh] flex-col gap-4 overflow-y-auto">
          {repos.length === 0 ? (
            <p className="py-6 text-center text-[12.5px] text-ink-faint">{t("config.noRepos")}</p>
          ) : (
            repos.map((r) => <RepoConfig key={r.id} repo={r} open={open} />)
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}

function RepoConfig({ repo, open }: { repo: RepoRef; open: boolean }) {
  const { t } = useTranslation();
  const { activeWorkspaceId } = useStore();
  const [items, setItems] = useState<ConfigItem[] | null>(null);

  useEffect(() => {
    if (!open) return;
    let alive = true;
    setItems(null);
    void api
      .effectiveConfig(repo.local_git_path, activeWorkspaceId ?? undefined)
      .then((r) => alive && setItems(r))
      .catch(() => alive && setItems([]));
    return () => {
      alive = false;
    };
  }, [open, repo.local_git_path, activeWorkspaceId]);

  const skills = items?.filter((i) => i.kind === "skill") ?? [];
  const rules = items?.filter((i) => i.kind === "rule") ?? [];

  return (
    <div className="rounded-[var(--radius-lg)] border border-border bg-surface/50 p-3">
      <div className="mb-2 flex items-center gap-1.5">
        <FolderGit2 size={13} className="text-brand" />
        <span className="font-mono text-[12px] text-ink">{repo.name}</span>
      </div>
      {items === null ? (
        <div className="flex items-center gap-1.5 px-1 py-2 text-[11.5px] text-ink-faint">
          <Loader2 size={11} className="animate-spin" />
          {t("config.loading")}
        </div>
      ) : items.length === 0 ? (
        <p className="px-1 py-1.5 text-[11.5px] text-ink-faint">{t("config.empty")}</p>
      ) : (
        <div className="flex flex-col gap-2">
          {skills.length > 0 && (
            <Section icon={<Layers size={11} />} label={t("config.skills")}>
              {skills.map((s) => (
                <Item key={`${s.layer}-${s.name}`} item={s} />
              ))}
            </Section>
          )}
          {rules.length > 0 && (
            <Section icon={<FileText size={11} />} label={t("config.rules")}>
              {rules.map((s) => (
                <Item key={`${s.layer}-${s.name}`} item={s} />
              ))}
            </Section>
          )}
        </div>
      )}
    </div>
  );
}

function Section({
  icon,
  label,
  children,
}: {
  icon: React.ReactNode;
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center gap-1.5 px-0.5 text-[10.5px] font-semibold uppercase tracking-wide text-ink-faint">
        {icon}
        {label}
      </div>
      <div className="flex flex-wrap gap-1.5">{children}</div>
    </div>
  );
}

function Item({ item }: { item: ConfigItem }) {
  const { t } = useTranslation();
  return (
    <span
      title={item.path}
      className={cn(
        "flex items-center gap-1.5 rounded-[var(--radius-md)] border px-2 py-1 text-[12px]",
        item.overridden
          ? "border-border bg-bg text-ink-faint line-through"
          : "border-border bg-bg text-ink",
      )}
    >
      {item.name}
      <span
        className={cn(
          "rounded-full px-1.5 py-px text-[10px] font-medium",
          item.layer === "repo"
            ? "bg-brand-ghost text-brand"
            : "bg-raised text-ink-muted",
        )}
      >
        {t(`config.layer_${item.layer}`, item.layer)}
      </span>
    </span>
  );
}
