import { useEffect, useState } from "react";

export type Theme = "dark" | "light";
const KEY = "atlas-theme";

/** Saved choice, else the OS preference, else dark. */
export function resolveInitialTheme(): Theme {
  try {
    const saved = localStorage.getItem(KEY);
    if (saved === "dark" || saved === "light") return saved;
    return window.matchMedia("(prefers-color-scheme: light)").matches
      ? "light"
      : "dark";
  } catch {
    return "dark";
  }
}

export function applyTheme(t: Theme) {
  document.documentElement.dataset.theme = t;
}

/** Theme + toggle. Persists the explicit choice; reflects it on <html>. */
export function useTheme(): { theme: Theme; toggle: () => void } {
  const [theme, setTheme] = useState<Theme>(
    () => (document.documentElement.dataset.theme as Theme) || resolveInitialTheme(),
  );
  useEffect(() => {
    applyTheme(theme);
  }, [theme]);
  const toggle = () =>
    setTheme((t) => {
      const next: Theme = t === "dark" ? "light" : "dark";
      try {
        localStorage.setItem(KEY, next);
      } catch {
        /* private mode / no storage */
      }
      return next;
    });
  return { theme, toggle };
}
