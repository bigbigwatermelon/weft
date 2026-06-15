import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { FolderOpen } from "lucide-react";
import { Dialog, DialogContent } from "../components/ui/Dialog";
import { Button } from "../components/ui/Button";
import { Input, Field } from "../components/ui/Input";
import { Select } from "../components/ui/Select";
import { useStore } from "../state/store";
import { api } from "../lib/api";
import { cn } from "../lib/cn";

type RepoMode = "local" | "clone" | "new";

const basename = (p: string) => p.trim().replace(/\/+$/, "").split("/").filter(Boolean).pop() ?? "";
const repoNameFromUrl = (u: string) =>
  u.trim().replace(/\.git$/, "").replace(/\/+$/, "").split(/[/:]/).filter(Boolean).pop() ?? "";

export function CreateWorkspaceDialog({ open, onOpenChange }: DProps) {
  const { createWorkspace } = useStore();
  const { t } = useTranslation();
  const [value, setValue] = useState("");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    if (!open) {
      setValue("");
      setBusy(false);
      setErr(null);
    }
  }, [open]);

  async function submit() {
    const name = value.trim();
    if (!name || busy) return;
    setBusy(true);
    setErr(null);
    try {
      await createWorkspace(name);
      onOpenChange(false);
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent title={t("dialog.newWorkspaceTitle")}>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            void submit();
          }}
          className="flex flex-col gap-4"
        >
          <Field label={t("dialog.workspaceName")}>
            <Input
              autoFocus
              placeholder={t("dialog.workspaceNamePlaceholder")}
              value={value}
              onChange={(e) => setValue(e.currentTarget.value)}
            />
          </Field>
          {err && <p className="text-[12px] text-danger">{err}</p>}
          <div className="flex justify-end gap-2">
            <Button type="button" variant="ghost" onClick={() => onOpenChange(false)}>
              {t("common.cancel")}
            </Button>
            <Button type="submit" variant="primary" disabled={!value.trim() || busy}>
              {busy ? t("dialog.creating") : t("dialog.createWorkspace")}
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}

export function AddRepoDialog({ open, onOpenChange }: DProps) {
  const { addRepo, cloneRepo, createRepo, projectsDir } = useStore();
  const { t } = useTranslation();
  const [mode, setMode] = useState<RepoMode>("local");
  const [path, setPath] = useState(""); // local
  const [url, setUrl] = useState(""); // clone
  const [dest, setDest] = useState(""); // clone/new parent
  const [name, setName] = useState(""); // all
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  // Reset on close; default the destination to the configured projects dir.
  useEffect(() => {
    if (!open) {
      setMode("local");
      setPath("");
      setUrl("");
      setDest("");
      setName("");
      setErr(null);
      setBusy(false);
    } else {
      setDest(projectsDir);
    }
  }, [open, projectsDir]);

  const finalName =
    name.trim() ||
    (mode === "local" ? basename(path) : mode === "clone" ? repoNameFromUrl(url) : "");

  const canSubmit =
    !busy &&
    (mode === "local"
      ? !!path.trim()
      : mode === "clone"
        ? !!url.trim() && !!dest.trim()
        : !!name.trim() && !!dest.trim());

  async function submit() {
    if (!canSubmit) return;
    setBusy(true);
    setErr(null);
    try {
      const n = finalName || "repo";
      if (mode === "local") await addRepo(n, path.trim());
      else if (mode === "clone") await cloneRepo(url.trim(), dest.trim(), n);
      else await createRepo(n, dest.trim());
      onOpenChange(false);
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function pickInto(setter: (v: string) => void, derive?: (v: string) => void) {
    const d = await api.pickFolder(t("dialog.addRepoTitle"));
    if (!d) return;
    setter(d);
    if (derive) derive(d);
  }

  const cta =
    mode === "local"
      ? busy
        ? t("dialog.creating")
        : t("dialog.addRepo")
      : mode === "clone"
        ? busy
          ? t("dialog.cloning")
          : t("dialog.cloneRepo")
        : busy
          ? t("dialog.creating")
          : t("dialog.createRepoCta");

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent title={t("dialog.addRepoTitle")}>
        <div className="mb-4 flex items-center rounded-[var(--radius-md)] bg-bg p-0.5">
          {(["local", "clone", "new"] as RepoMode[]).map((m) => (
            <button
              key={m}
              type="button"
              onClick={() => {
                setMode(m);
                setErr(null);
              }}
              className={cn(
                "flex-1 rounded-[var(--radius-sm)] px-2 py-1.5 text-[12.5px] transition-colors",
                mode === m
                  ? "bg-raised text-ink shadow-[0_1px_2px_rgba(0,0,0,0.2)]"
                  : "text-ink-faint hover:text-ink-muted",
              )}
            >
              {t(`dialog.repoMode_${m}`)}
            </button>
          ))}
        </div>

        <form
          onSubmit={(e) => {
            e.preventDefault();
            void submit();
          }}
          className="flex flex-col gap-4"
        >
          {mode === "local" && (
            <Field label={t("dialog.repoPath")}>
              <PathInput
                value={path}
                placeholder="/Users/you/code/web-app"
                onChange={setPath}
                onPick={() => pickInto(setPath)}
              />
            </Field>
          )}

          {mode === "clone" && (
            <>
              <Field label={t("dialog.repoUrl")}>
                <Input
                  autoFocus
                  placeholder="https://github.com/acme/web-app.git"
                  value={url}
                  onChange={(e) => setUrl(e.currentTarget.value)}
                />
              </Field>
              <Field label={t("dialog.repoLocation")}>
                <PathInput
                  value={dest}
                  placeholder="/Users/you/code"
                  onChange={setDest}
                  onPick={() => pickInto(setDest)}
                />
              </Field>
            </>
          )}

          {mode === "new" && (
            <Field label={t("dialog.repoLocation")}>
              <PathInput
                value={dest}
                placeholder="/Users/you/code"
                onChange={setDest}
                onPick={() => pickInto(setDest)}
              />
            </Field>
          )}

          {mode !== "local" && (
            <Field label={t("dialog.repoName")}>
              <Input
                autoFocus={mode === "new"}
                placeholder="web-app"
                value={name}
                onChange={(e) => setName(e.currentTarget.value)}
              />
            </Field>
          )}
          {mode === "local" && (
            <Field label={t("dialog.repoName")}>
              <Input
                placeholder={basename(path) || "web-app"}
                value={name}
                onChange={(e) => setName(e.currentTarget.value)}
              />
            </Field>
          )}

          {err && <p className="text-[12px] leading-relaxed text-danger">{err}</p>}
          <div className="flex justify-end gap-2">
            <Button type="button" variant="ghost" onClick={() => onOpenChange(false)}>
              {t("common.cancel")}
            </Button>
            <Button type="submit" variant="primary" disabled={!canSubmit}>
              {cta}
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}

/** A path input with a trailing native folder-picker button. */
function PathInput({
  value,
  placeholder,
  onChange,
  onPick,
}: {
  value: string;
  placeholder: string;
  onChange: (v: string) => void;
  onPick: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="flex items-center gap-2">
      <Input
        placeholder={placeholder}
        value={value}
        onChange={(e) => onChange(e.currentTarget.value)}
      />
      <button
        type="button"
        onClick={onPick}
        title={t("settings.choose")}
        className="grid h-9 w-9 shrink-0 place-items-center rounded-[var(--radius-md)] border border-border text-ink-muted transition-colors hover:bg-surface hover:text-ink"
      >
        <FolderOpen size={15} />
      </button>
    </div>
  );
}

export function CreateThreadDialog({ open, onOpenChange }: DProps) {
  const { createThread } = useStore();
  const { t } = useTranslation();
  const [title, setTitle] = useState("");
  const [kind, setKind] = useState("feature");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  async function submit() {
    if (!title.trim() || busy) return;
    setBusy(true);
    setErr(null);
    try {
      await createThread(title.trim(), kind);
      setTitle("");
      onOpenChange(false);
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        title={t("dialog.newTaskTitle")}
        description={t("dialog.newTaskDesc")}
      >
        <form
          onSubmit={(e) => {
            e.preventDefault();
            void submit();
          }}
          className="flex flex-col gap-4"
        >
          <Field label={t("dialog.taskTitle")}>
            <Input
              autoFocus
              placeholder={t("dialog.taskTitleHint")}
              value={title}
              onChange={(e) => setTitle(e.currentTarget.value)}
            />
          </Field>
          <Field label={t("dialog.threadType")}>
            <Select
              value={kind}
              onValueChange={setKind}
              ariaLabel={t("dialog.threadType")}
              options={[
                { value: "feature", label: t("kind.feature") },
                { value: "bugfix", label: t("kind.bugfix") },
                { value: "refactor", label: t("kind.refactor") },
                { value: "spike", label: t("kind.spike") },
              ]}
            />
          </Field>
          {err && <p className="text-[12px] text-danger">{err}</p>}
          <div className="flex justify-end gap-2">
            <Button type="button" variant="ghost" onClick={() => onOpenChange(false)}>
              {t("common.cancel")}
            </Button>
            <Button type="submit" variant="primary" disabled={!title.trim() || busy}>
              {busy ? t("dialog.creating") : t("dialog.createTask")}
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}

export function RenameDialog({
  open,
  onOpenChange,
  title,
  label,
  initial,
  onSubmit,
}: DProps & {
  title: string;
  label: string;
  initial: string;
  onSubmit: (value: string) => Promise<void>;
}) {
  const { t } = useTranslation();
  const [value, setValue] = useState(initial);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  // Seed `value` only on the false→true edge so an external refresh that
  // changes `initial` while the dialog is open doesn't clobber what the user
  // is typing. We read the latest `initial` via a ref to avoid stale closures.
  const initialRef = useRef(initial);
  initialRef.current = initial;
  const wasOpen = useRef(false);
  useEffect(() => {
    if (open && !wasOpen.current) {
      setValue(initialRef.current);
      setBusy(false);
      setErr(null);
    }
    wasOpen.current = open;
  }, [open]);

  async function submit() {
    const v = value.trim();
    if (!v || busy) return;
    if (v === initial.trim()) {
      onOpenChange(false);
      return;
    }
    setBusy(true);
    setErr(null);
    try {
      await onSubmit(v);
      onOpenChange(false);
    } catch (e) {
      const raw = String(e);
      // Backend uses anyhow::bail!("…cannot be empty") / "…already" for the
      // two known rejections — translate them; fall back to a generic message
      // (the raw Rust string is logged for debugging, not shown).
      if (/empty/i.test(raw)) setErr(t("error.renameEmpty"));
      else if (/already/i.test(raw)) setErr(t("error.renameDuplicate"));
      else setErr(t("error.renameFailed"));
      if (import.meta.env.DEV) console.error("rename failed:", raw);
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent title={title}>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            void submit();
          }}
          className="flex flex-col gap-4"
        >
          <Field label={label}>
            <Input
              autoFocus
              value={value}
              onChange={(e) => setValue(e.currentTarget.value)}
              onFocus={(e) => e.currentTarget.select()}
            />
          </Field>
          {err && <p className="text-[12px] text-danger">{err}</p>}
          <div className="flex justify-end gap-2">
            <Button type="button" variant="ghost" onClick={() => onOpenChange(false)}>
              {t("common.cancel")}
            </Button>
            <Button type="submit" variant="primary" disabled={!value.trim() || busy}>
              {busy ? t("dialog.renaming") : t("common.rename")}
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}

interface DProps {
  open: boolean;
  onOpenChange: (o: boolean) => void;
}
