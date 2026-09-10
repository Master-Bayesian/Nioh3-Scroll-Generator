import { useSyncExternalStore } from "react";
import resources from "./ui-locales.json";
import { publicError } from "./public-errors";
import { plainGameText } from "./game-text";
export type UiLocale = "zh-CN" | "en-US" | "ja-JP";
let locale: UiLocale =
  typeof localStorage !== "undefined" &&
  ["en-US", "ja-JP"].includes(localStorage.getItem("nioh3-ui-locale") || "")
    ? (localStorage.getItem("nioh3-ui-locale") as UiLocale)
    : "zh-CN";
const listeners = new Set<() => void>();
const subscribe = (listener: () => void) => {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
};
export const currentLocale = () => locale;
export const useUiLocale = () => useSyncExternalStore(subscribe, currentLocale);
export function setUiLocale(value: UiLocale) {
  locale = value;
  localStorage.setItem("nioh3-ui-locale", value);
  document.documentElement.lang = value;
  listeners.forEach((fn) => fn());
}
const maps = new Map<string, Record<string, string>>(),
  patterns = new Map<string, RegExp>();
export function localize(text: string): string {
  text=text.replace(/\s+/g,' ');
  text = publicError(text);
  if (locale === "zh-CN" || !text) return plainGameText(text);
  if (!maps.has(locale)) {
    const entries = {
      ...resources.game[locale],
      ...Object.fromEntries(
        Object.entries(resources.ui).map(([key, values]) => [
          key,
          values[locale === "en-US" ? 0 : 1],
        ]),
      ),
    };
    maps.set(locale, entries);
    patterns.set(
      locale,
      new RegExp(
        Object.keys(entries)
          .sort((a, b) => b.length - a.length)
          .map((s) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"))
          .join("|"),
        "g",
      ),
    );
  }
  const entries = maps.get(locale)!;
  if (entries[text.trim()])
    return plainGameText(text.replace(text.trim(), entries[text.trim()]));
  const translated = text.replace(
    patterns.get(locale)!,
    (part) => entries[part],
  );
  return plainGameText(locale === "en-US"
    ? translated.replaceAll("（", "(").replaceAll("）", ")")
    : translated);
}
