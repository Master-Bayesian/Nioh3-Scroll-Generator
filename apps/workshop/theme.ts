import { useState } from "react";

/**
 * Appearance (#14): light, dark, or follow Windows. Per-device, kept in WebView
 * storage. Light is the default until the dark palette is refined (#35); a
 * stored "system" is kept so following Windows stays an explicit choice.
 */
export type Theme = "system" | "light" | "dark";
const STORAGE = "nioh3-theme";

function stored(): Theme {
  try {
    const value = localStorage.getItem(STORAGE);
    return value === "system" || value === "dark" ? value : "light";
  } catch {
    return "light";
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
      if (next === "light") localStorage.removeItem(STORAGE);
      else localStorage.setItem(STORAGE, next);
    } catch {
      // The choice still holds for this session.
    }
  }];
}
