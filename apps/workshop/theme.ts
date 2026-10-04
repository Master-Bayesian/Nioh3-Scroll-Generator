import { useState } from "react";

/** Appearance (#14): follow Windows, or force light or dark. Per-device, kept in WebView storage. */
export type Theme = "system" | "light" | "dark";
const STORAGE = "nioh3-theme";

function stored(): Theme {
  try {
    const value = localStorage.getItem(STORAGE);
    return value === "light" || value === "dark" ? value : "system";
  } catch {
    return "system";
  }
}

function apply(theme: Theme) {
  if (theme === "system") delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = theme;
}

apply(stored());

export function useTheme(): [Theme, (theme: Theme) => void] {
  const [theme, setTheme] = useState(stored);
  return [theme, next => {
    setTheme(next);
    apply(next);
    try {
      if (next === "system") localStorage.removeItem(STORAGE);
      else localStorage.setItem(STORAGE, next);
    } catch {
      // The choice still holds for this session.
    }
  }];
}
