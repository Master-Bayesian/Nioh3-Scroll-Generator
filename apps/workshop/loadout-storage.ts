/** Versioned, read-only planner snapshots in the application's WebView profile. */
import validate from "./loadout-storage-validator.mjs";
import type { LoadoutAttribute } from "./LoadoutPlanner";
import type { LoadoutSelection, LoadoutStyle } from "./loadout-state";

export const LOADOUT_STORAGE_KEY = "nioh3-loadout-drafts-v1";
export interface LoadoutDraft {
  id: number; name: string; style: LoadoutStyle;
  attributes: LoadoutAttribute[]; selection: LoadoutSelection;
}
export interface LoadoutWorkspace { version: 1; activeId: number; drafts: LoadoutDraft[] }
export type PlannerStorage = Pick<Storage, "getItem" | "setItem">;


export function parseLoadoutWorkspace(raw: string): LoadoutWorkspace {
  const value: unknown = JSON.parse(raw);
  if (!validate(value)) throw new Error("Invalid planner storage schema");
  const workspace = value as LoadoutWorkspace;
  const ids = new Set(workspace.drafts.map(draft => draft.id));
  const names = new Set(workspace.drafts.map(draft => draft.name.trim()));
  if (ids.size !== workspace.drafts.length || names.size !== ids.size || !ids.has(workspace.activeId)
    || workspace.drafts.some(draft => !draft.name.trim() || !Number.isSafeInteger(draft.id)
      || draft.attributes.map(row => row.name).join("") !== "体心刚武技智咒"
      || draft.attributes.reduce((sum, row) => sum + row.value, 0) < 36
      || draft.attributes.reduce((sum, row) => sum + row.value, 0) > 485)) throw new Error("Invalid planner draft identity or attributes");
  return workspace;
}

export function snapshotLoadoutWorkspace(drafts: LoadoutDraft[], activeId: number, current: Omit<LoadoutDraft, "id" | "name">): LoadoutWorkspace {
  const workspace: LoadoutWorkspace = { version: 1, activeId, drafts: structuredClone(drafts.map(draft => draft.id === activeId ? { ...draft, ...current } : draft)) };
  // Broker references are session capabilities, never portable planner data.
  for (const draft of workspace.drafts) for (const scroll of [...Object.values(draft.selection.scrolls), ...Object.values(draft.selection.savedExtras?.scrolls ?? {}).flat()]) {
    if (scroll) delete scroll.backend;
  }
  return workspace;
}

export function readLoadoutWorkspace(storage: PlannerStorage): LoadoutWorkspace | null {
  const raw = storage.getItem(LOADOUT_STORAGE_KEY);
  return raw === null ? null : parseLoadoutWorkspace(raw);
}
export function saveLoadoutWorkspace(storage: PlannerStorage, workspace: LoadoutWorkspace, preserveOriginal = false) {
  const raw = JSON.stringify(workspace);
  parseLoadoutWorkspace(raw);
  if (preserveOriginal) {
    const original = storage.getItem(LOADOUT_STORAGE_KEY);
    if (original !== null) {
      const key = `${LOADOUT_STORAGE_KEY}-recovery-${Date.now()}-${crypto.randomUUID()}`;
      storage.setItem(key, original);
      if (storage.getItem(key) !== original) throw new Error("Planner recovery copy verification failed");
    }
  }
  storage.setItem(LOADOUT_STORAGE_KEY, raw);
  if (storage.getItem(LOADOUT_STORAGE_KEY) !== raw) throw new Error("Planner storage verification failed");
}
