import type { SaveCharacter, SaveReference } from "../../packages/contracts/protected-responses";

export interface LoadoutSaveEquipmentGateway {
  discover(): Promise<SaveReference[]>;
  pick(): Promise<SaveReference | null>;
  read(reference: SaveReference): Promise<SaveCharacter>;
}

export interface LoadoutSaveEquipmentSnapshot {
  saves: SaveReference[];
  selected: SaveReference | null;
  character: SaveCharacter | null;
  busy: boolean;
  error: string | null;
}

function initialSnapshot(): LoadoutSaveEquipmentSnapshot {
  return { saves: [], selected: null, character: null, busy: false, error: null };
}

function copyReference(reference: SaveReference): SaveReference {
  return {
    save_id: reference.save_id,
    path: reference.path,
    account_id: reference.account_id,
    save_slot: reference.save_slot,
  };
}

function copyCharacter(character: SaveCharacter): SaveCharacter {
  return structuredClone(character);
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function validateCharacter(value: unknown, expectedSaveId: string): SaveCharacter {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error("SAVE_CHARACTER_EXPECTED");
  }
  const candidate = value as Partial<SaveCharacter>;
  if (candidate.save_id !== expectedSaveId) throw new Error("SAVE_IDENTITY_MISMATCH");
  if (typeof candidate.source_sha256 !== "string") throw new Error("SAVE_CHARACTER_SOURCE_SHA256_EXPECTED");
  const equipmentSlots = candidate.equipment_slots;
  if (typeof equipmentSlots !== "number" || !Number.isInteger(equipmentSlots) || equipmentSlots < 0 || equipmentSlots > 2500) {
    throw new Error("SAVE_CHARACTER_EQUIPMENT_SLOTS_INVALID");
  }
  if (!Array.isArray(candidate.equipment) || candidate.equipment.length > 2500) {
    throw new Error("SAVE_CHARACTER_EQUIPMENT_EXPECTED");
  }
  return candidate as SaveCharacter;
}

function saveIdMatches(left: SaveReference | null, right: SaveReference): boolean {
  return left?.save_id === right.save_id;
}

/** Read-only save equipment state owned by one loadout view. */
export class LoadoutSaveEquipmentSource {
  private state: LoadoutSaveEquipmentSnapshot = initialSnapshot();
  private listeners = new Set<() => void>();
  private epoch = 0;
  private disposed = false;

  constructor(private readonly gateway: LoadoutSaveEquipmentGateway) {}

  getSnapshot = () => this.state;

  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => { this.listeners.delete(listener); };
  };

  private update(values: Partial<LoadoutSaveEquipmentSnapshot>) {
    if (this.disposed) return;
    this.state = { ...this.state, ...values };
    this.listeners.forEach(listener => listener());
  }

  private begin(): number | null {
    if (this.disposed) return null;
    const epoch = ++this.epoch;
    this.update({ busy: true, error: null });
    return epoch;
  }

  private current(epoch: number): boolean {
    return !this.disposed && epoch === this.epoch;
  }

  private fail(epoch: number, error: unknown) {
    if (this.current(epoch)) this.update({ busy: false, error: errorMessage(error) });
  }

  private async readFor(epoch: number, reference: SaveReference) {
    try {
      const character = validateCharacter(await this.gateway.read(reference), reference.save_id);
      if (!this.current(epoch)) return;
      this.update({ character: copyCharacter(character), busy: false, error: null });
    } catch (error) {
      this.fail(epoch, error);
    }
  }

  async discover(preferred?: SaveReference | null): Promise<void> {
    const epoch = this.begin();
    if (epoch === null) return;

    let discovered: SaveReference[];
    try {
      const result = await this.gateway.discover();
      if (!Array.isArray(result)) throw new Error("SAVE_DISCOVER_EXPECTED");
      discovered = result.map(copyReference);
    } catch (error) {
      this.fail(epoch, error);
      return;
    }
    if (!this.current(epoch)) return;

    const previous = this.state.selected;
    let selected: SaveReference | null = null;
    if (preferred !== undefined && preferred !== null) {
      selected = copyReference(preferred);
    } else if (previous) {
      selected = discovered.find(reference => saveIdMatches(previous, reference)) ?? null;
    }
    if (!selected && (preferred === undefined || preferred === null)) {
      selected = discovered.length === 1 ? discovered[0] : null;
    }

    if (!selected) {
      this.update({ saves: discovered, selected: null, character: null, busy: false, error: null });
      return;
    }

    this.update({ saves: discovered, selected, character: null, busy: true, error: null });
    await this.readFor(epoch, selected);
  }

  async select(reference: SaveReference | null): Promise<void> {
    const epoch = this.begin();
    if (epoch === null) return;
    if (reference === null) {
      this.update({ selected: null, character: null, busy: false, error: null });
      return;
    }

    const selected = copyReference(reference);
    this.update({ selected, character: null });
    await this.readFor(epoch, selected);
  }

  async refresh(): Promise<void> {
    const epoch = this.begin();
    if (epoch === null) return;
    const selected = this.state.selected;
    if (!selected) {
      this.update({ busy: false, error: "SAVE_SELECTION_REQUIRED" });
      return;
    }

    this.update({ character: null });
    await this.readFor(epoch, selected);
  }

  async pick(): Promise<void> {
    const epoch = this.begin();
    if (epoch === null) return;

    let picked: SaveReference | null;
    try {
      picked = await this.gateway.pick();
    } catch (error) {
      this.fail(epoch, error);
      return;
    }
    if (!this.current(epoch)) return;
    if (picked === null) {
      // Closing the native picker is not a selection change.
      this.update({ busy: false });
      return;
    }

    try {
      const selected = copyReference(picked);
      const saves = this.state.saves.slice();
      const index = saves.findIndex(reference => reference.save_id === selected.save_id);
      if (index < 0) saves.push(selected);
      else saves[index] = selected;
      this.update({ saves, selected, character: null, busy: true, error: null });
      await this.readFor(epoch, selected);
    } catch (error) {
      this.fail(epoch, error);
    }
  }

  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    ++this.epoch;
    this.state = { ...this.state, busy: false };
    this.listeners.clear();
  }
}
