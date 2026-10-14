import type { SaveCharacter, SaveReference } from "../../packages/contracts/protected-responses";
import { desktop } from "./desktop-bridge";
import { saveObserver, saveSession } from "./save-workspace";
import type { LoadoutSaveEquipmentGateway } from "./loadout-save-equipment-source";
import type { OperationController } from "../desktop/src/operation-controller";
import type { OperationsApi } from "../desktop/src/operations-api";

/** Planner reads share the protected observer without changing reviewed save plans. */
export function createLoadoutSaveGateway(observer: Pick<OperationController, "run">, api: Pick<OperationsApi, "execute" | "selectSave">): LoadoutSaveEquipmentGateway {
  return {
  async discover() {
    const result = await observer.run(() => api.execute({ method: "save.discover", params: {} }));
    if (!result || !("saves" in result)) throw new Error("未能检测存档。请重试或浏览存档文件。");
    return result.saves;
  },
  async pick() {
    const result = await observer.run(() => api.selectSave());
    if (result === null) return null;
    if (!result || !("save_id" in result) || !("path" in result) || !("account_id" in result)) throw new Error("未能选择存档。请重试。");
    return result as SaveReference;
  },
  async read(reference) {
    const result = await observer.run(() => api.execute({ method: "save.character", params: { save_id: reference.save_id } }));
    if (!result || !("equipment_slots" in result) || !("save_id" in result)) throw new Error("未能读取存档装备。请重新读取。");
    return result as SaveCharacter;
  },
  };
}

export const loadoutSaveGateway = desktop ? createLoadoutSaveGateway(saveObserver!, window.operations) : null;

let plannerSave: SaveReference | null = null;
export const rememberLoadoutSave = (reference: SaveReference | null) => { plannerSave = reference ? { ...reference } : null; };
export const preferredLoadoutSave = () => plannerSave ?? saveSession?.getSnapshot().selected ?? null;
