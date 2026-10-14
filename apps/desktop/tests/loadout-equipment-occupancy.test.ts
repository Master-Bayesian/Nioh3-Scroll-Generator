import assert from "node:assert/strict";
import { test } from "node:test";
import { initialLoadoutSelection } from "../../workshop/loadout-state";
import { equipmentSourceLabel, sameLoadoutPiece, setLoadoutEquipment } from "../../workshop/loadout-equipment";
import { decodeLoadout, encodeLoadout, type LoadoutPiece } from "../../workshop/loadout-code";

const piece = (item_id = 424): LoadoutPiece => ({ item_id, level: 180, level_before_forge: 180, plus: 0, rarity: 4, effects: [] });
test("a selected code or favorite cannot occupy both weapon or soul slots", () => {
  for (const [id, owner, first, second] of [[424, "samurai", "melee0", "melee1"], [2669, "shared", "soul0", "soul1"]] as const) {
    const chosen = { ...piece(id), origin: "favorites" as const };
    const state = setLoadoutEquipment(initialLoadoutSelection(), "samurai", owner, first, chosen);
    assert.ok(state.equipment[owner][first]);
    assert.equal(setLoadoutEquipment(state, "samurai", owner, second, decodeLoadout(encodeLoadout([chosen]))[0]), state);
    const cleared = setLoadoutEquipment(state, "samurai", owner, first, null);
    assert.ok(setLoadoutEquipment(cleared, "samurai", owner, second, chosen).equipment[owner][second]);
    assert.ok(setLoadoutEquipment(state, "samurai", owner, first, chosen).equipment[owner][first]);
  }
});
test("inventory identity differentiates two identical saves and survives a refreshed digest", () => {
  const a = { ...piece(), source: { saveId: "one", digest: "old", slotIndex: 4, inventoryKey: 45 } };
  const same = { ...a, source: { ...a.source, digest: "new" } };
  const other = { ...a, source: { ...a.source, slotIndex: 5, inventoryKey: 46 } };
  const moved = { ...a, source: { ...a.source, digest: "new", slotIndex: 7 } };
  assert.ok(sameLoadoutPiece(a, same));
  assert.ok(sameLoadoutPiece(a, moved));
  assert.equal(sameLoadoutPiece(a, { ...other, source: { ...other.source, slotIndex: 4 } }), false);
  assert.equal(sameLoadoutPiece(a, other), false);
  let state = setLoadoutEquipment(initialLoadoutSelection(), "samurai", "samurai", "melee0", a);
  assert.equal(setLoadoutEquipment(state, "samurai", "samurai", "melee1", same), state);
  assert.equal(setLoadoutEquipment(state, "samurai", "samurai", "melee1", moved), state);
  state = setLoadoutEquipment(state, "samurai", "samurai", "melee1", other);
  assert.equal(state.equipment.samurai.melee1.source?.slotIndex, 5);
});
test("sources are explicit metadata and never change equipment codes", () => {
  const base = piece();
  for (const [origin, label] of [["save", "存档"], ["favorites", "装备收藏"], ["code", "装备码"]] as const) {
    assert.equal(equipmentSourceLabel({ ...base, origin }), label);
    assert.equal(encodeLoadout([{ ...base, origin }]), encodeLoadout([base]));
  }
  assert.equal(equipmentSourceLabel(base), "未记录");
});
test("only two soul slots are equipable and incompatible items fail closed", () => {
  const state = initialLoadoutSelection();
  assert.equal(setLoadoutEquipment(state, "samurai", "shared", "soul2", piece(2669)), state);
  assert.equal(setLoadoutEquipment(state, "samurai", "samurai", "head", piece()), state);
});
