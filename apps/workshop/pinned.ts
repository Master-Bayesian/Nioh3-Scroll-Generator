import { useSyncExternalStore } from "react";

/**
 * Starred entries of the player's own save (scrolls, equipment, items): a star
 * keeps an entry at the top of its list and lets "favorites only" find it among
 * hundreds. Only identity keys are stored, in the app's WebView storage, which
 * lives in the application data directory and survives updates. This is not
 * the 收藏夹 page, which saves complete pieces to add later.
 */
export type PinnedKind = "scroll" | "equipment" | "item";

const listeners = new Set<() => void>();
const cache = new Map<PinnedKind, ReadonlySet<string>>();
const storageKey = (kind: PinnedKind) => "nioh3-pinned-" + kind;

function read(kind: PinnedKind): ReadonlySet<string> {
  const cached = cache.get(kind);
  if (cached) return cached;
  let keys: ReadonlySet<string>;
  try {
    const parsed = JSON.parse(localStorage.getItem(storageKey(kind)) || "[]");
    keys = new Set(Array.isArray(parsed) ? parsed.filter((key): key is string => typeof key === "string") : []);
  } catch {
    keys = new Set();
  }
  cache.set(kind, keys);
  return keys;
}

export function usePinned(kind: PinnedKind): ReadonlySet<string> {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    () => read(kind),
  );
}

export function togglePinned(kind: PinnedKind, key: string) {
  const next = new Set(read(kind));
  if (!next.delete(key)) next.add(key);
  cache.set(kind, next);
  try {
    localStorage.setItem(storageKey(kind), JSON.stringify([...next]));
  } catch {
    // The stars still work for this session.
  }
  listeners.forEach((listener) => listener());
}

/** Starred entries first, each group keeping its original order. */
export function pinnedFirst<T>(entries: readonly T[], isPinned: (entry: T) => boolean): T[] {
  return [...entries.filter(isPinned), ...entries.filter((entry) => !isPinned(entry))];
}
