import { useSyncExternalStore } from "react";
import resources from "./ui-locales.json";
import { publicError } from "./public-errors";
import { fillTemplateSlots, plainGameText as plainText, withoutTemplateArguments } from "./game-text";
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
// Generic names for a template's unfilled buff/ailment argument, per locale.
const slotWord = (word: "增益效果" | "异常状态") =>
  locale === "zh-CN"
    ? word
    : (resources.ui as Record<string, string[]>)[word]?.[locale === "en-US" ? 0 : 1] ?? word;
const plainGameText = (text: string) =>
  fillTemplateSlots(plainText(text), slotWord("增益效果"), slotWord("异常状态"));
const maps = new Map<string, Record<string, string>>(),
  patterns = new Map<string, RegExp>();
/**
 * A whole game name in the current locale, or null when the locale has no
 * exact entry. Names must not be translated piecewise: a substring such as
 * 忍者 or 无 inside an item name is not the interface word.
 */
export function localizeName(text: string): string | null {
  if (locale === "zh-CN" || !text) return text;
  localize(text);
  const names = maps.get(locale);
  return names?.[text.trim()] ?? names?.[text.replace(/\s+/g, " ").trim()] ?? null;
}
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
  // A whole game name, its arguments filled as the Chinese UI shows it
  // ("缝影的持有上限"), has the game's own name in this locale.
  const filled = fillTemplateSlots(plainText(text), "增益效果", "异常状态").trim();
  if (entries[filled]) return plainGameText(text.replace(text.trim(), entries[filled]));
  // Otherwise resolved arguments are Chinese game names; drop them for the
  // generic wording.
  text = withoutTemplateArguments(text);
  if (entries[text.trim()])
    return plainGameText(text.replace(text.trim(), entries[text.trim()]));
  const translated = text.replace(
    patterns.get(locale)!,
    (part) => entries[part],
  );
  return plainGameText(locale === "en-US"
    ? translated.replace(/(?<=[A-Za-z0-9])（/g, " (").replaceAll("（", "(").replaceAll("）", ")").replaceAll("、", ", ")
    : translated);
}
