import type { CharacterEquipment } from '../../packages/contracts/protected-responses';

/** The acquisition key and seed identify a physical owned record across wear edits.
 * The item id identifies only its type: a replacement can have that same id.
 */
export interface LoadoutUndoItem {
  slot_index: number;
  slot: number | null;
  item_id: number;
  inventory_key: number;
  seed: number;
}
export interface LoadoutUndo {
  school: 'samurai' | 'ninja';
  restore: LoadoutUndoItem[];
}

function integer(value: unknown, maximum: number): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 && value <= maximum;
}

/** Old entries without physical-item identity cannot authorize an exact restore. */
export function parseLoadoutUndo(value: unknown): LoadoutUndo | null {
  if (!value || typeof value !== 'object') return null;
  const candidate = value as Partial<LoadoutUndo>;
  if ((candidate.school !== 'samurai' && candidate.school !== 'ninja') ||
      !Array.isArray(candidate.restore) || candidate.restore.length === 0) return null;
  const indices = new Set<number>();
  for (const item of candidate.restore) {
    if (!item || !integer(item.slot_index, 2499) || !integer(item.item_id, 0xffff) ||
        !integer(item.inventory_key, 0xffffffff) || !integer(item.seed, 0xffff) ||
        !(item.slot === null || integer(item.slot, 10)) || indices.has(item.slot_index)) return null;
    indices.add(item.slot_index);
  }
  return candidate as LoadoutUndo;
}

export function matchesLoadoutUndoItem(previous: LoadoutUndoItem, current: CharacterEquipment | undefined): boolean {
  return !!current && current.slot_index === previous.slot_index && current.item_id === previous.item_id &&
    current.inventory_key === previous.inventory_key && current.seed === previous.seed;
}
