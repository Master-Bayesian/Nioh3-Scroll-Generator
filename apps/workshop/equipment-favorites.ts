import { useSyncExternalStore } from "react";
import type { CharacterEquipment } from "../../packages/contracts/protected-responses";

/**
 * Saved equipment (#21): the fields a new item is built from, so a favorite can
 * be added to any character later through "Add new equipment". Kept in the
 * app's WebView storage, which lives in the application data directory and
 * survives updates.
 */
export interface EquipmentFavorite {
  key: string;
  saved_at: number;
  item_id: number;
  level: number;
  level_before_forge: number;
  plus: number;
  rarity: number;
  hell: boolean;
  hell_skill: number;
  effects: { effect_id: number; value: number; star?: boolean }[];
}

const STORAGE = "nioh3-equipment-favorites";
export const FAVORITE_LIMIT = 100;
const listeners = new Set<() => void>();
let cache: EquipmentFavorite[] | null = null;

function read(): EquipmentFavorite[] {
  if (cache) return cache;
  try {
    const parsed = JSON.parse(localStorage.getItem(STORAGE) || "[]");
    cache = Array.isArray(parsed) ? parsed.filter(entry => Number.isInteger(entry?.item_id)) : [];
  } catch {
    cache = [];
  }
  return cache;
}
function write(next: EquipmentFavorite[]) {
  cache = next;
  try {
    localStorage.setItem(STORAGE, JSON.stringify(next));
  } catch {
    // The list still works for this session.
  }
  listeners.forEach(listener => listener());
}

/** The same item with the same values is one favorite. */
export function favoriteKey(item: Pick<CharacterEquipment, "item_id" | "level" | "plus" | "rarity"> & { effects: readonly { effect_id: number; value: number; star?: boolean }[]; hell?: boolean; hell_skill?: number }) {
  return [item.item_id, item.level, item.plus, item.rarity, item.hell ? item.hell_skill ?? 0 : "-",
    ...item.effects.map(effect => effect.effect_id + ":" + effect.value + (effect.star ? "*" : ""))].join("|");
}

export function useEquipmentFavorites() {
  return useSyncExternalStore(
    listener => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    read,
  );
}

/** Add the item as it is now, or remove it if it is already saved. Returns whether it is saved afterwards. */
export function toggleEquipmentFavorite(item: CharacterEquipment): boolean {
  const key = favoriteKey(item);
  const current = read();
  if (current.some(entry => entry.key === key)) {
    write(current.filter(entry => entry.key !== key));
    return false;
  }
  if (current.length >= FAVORITE_LIMIT) throw new Error("装备收藏最多 " + FAVORITE_LIMIT + " 件，请先移除一些。");
  write([{
    key,
    saved_at: Date.now(),
    item_id: item.item_id,
    level: item.level,
    level_before_forge: item.level_before_forge,
    plus: item.plus,
    rarity: item.rarity,
    hell: !!item.hell,
    hell_skill: item.hell_skill ?? 0,
    effects: item.effects.map(effect => ({ effect_id: effect.effect_id, value: effect.value, ...(effect.star != null ? { star: effect.star } : {}) })),
  }, ...current]);
  return true;
}

export function removeEquipmentFavorite(key: string) {
  write(read().filter(entry => entry.key !== key));
}

/** Save pieces from a loadout code; already saved pieces are skipped. */
export function importEquipmentFavorites(pieces: Omit<EquipmentFavorite, "key" | "saved_at">[]): { added: number; existing: number } {
  const current = read();
  const keys = new Set(current.map(entry => entry.key));
  const fresh: EquipmentFavorite[] = [];
  for (const piece of pieces) {
    const key = favoriteKey(piece);
    if (keys.has(key)) continue;
    keys.add(key);
    fresh.push({ ...piece, key, saved_at: Date.now() });
  }
  if (current.length + fresh.length > FAVORITE_LIMIT)
    throw new Error("装备收藏最多 " + FAVORITE_LIMIT + " 件，这个配装码有 " + fresh.length + " 件新装备，请先移除一些收藏。");
  if (fresh.length) write([...fresh, ...current]);
  return { added: fresh.length, existing: pieces.length - fresh.length };
}
