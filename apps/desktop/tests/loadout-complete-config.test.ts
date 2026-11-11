import assert from "node:assert/strict";
import { test } from "node:test";
import { loadoutWarnings } from "../../workshop/loadout-warnings";
import { savedConfigReview, applySavedLoadoutImport } from "../../workshop/loadout-save-config";
import { savedLoadoutPiece } from "../../workshop/loadout-equipment";
import { soulCoreDefinition, soulCoreEffects } from "../../workshop/loadout-soul-cores";
import { initialLoadoutSelection, skills, clans, clanProtectionValue, ESSENCE_EFFECTS } from "../../workshop/loadout-state";
import { decodeLoadout, encodeLoadout, type LoadoutPiece } from "../../workshop/loadout-code";
import type { CharacterEquipment, SaveCharacter, SavedLoadout } from "../../../packages/contracts/protected-responses";
import nativeSkillScope from "../../workshop/loadout-native-skill-scope.json";
const attributes = [5, 5, 5, 6, 5, 5, 5].map(value => ({ value }));
const piece = (id = 424): LoadoutPiece => ({ item_id: id, level: 180, level_before_forge: 180, plus: 20, rarity: 4, effects: [] });
const warnings = (selection = initialLoadoutSelection()) => loadoutWarnings(selection, "samurai", attributes);
const character = (): SaveCharacter => ({ save_id: "one", source_sha256: "a".repeat(64), currencies: { amrita: null, gold: null }, equipment_slots: 2500, equipment: [] });
const row = (): CharacterEquipment => ({ ...piece(2669), slot_index: 4, inventory_key: 45, appearance_id: 2669, quantity: 1, seed: 1, familiarity: 0,
  effects: [{ index: 0, effect_id: 29053, value: 150 }], audit: { natural: false, verdict: "unnatural", findings: [{ code: "missing_innate" }] } });

test("empty planner checks are pending rather than a claim of complete legality", () => {
  const result = warnings();
  assert.equal(result.filter(row => row.severity === "conflict").length, 0);
  assert.equal(result.filter(row => row.code === "capacity_unknown").length, 0);
  assert.equal(loadoutWarnings(initialLoadoutSelection(), "samurai", [{ value: 0 }]).some(row => row.code === "attributes" && row.severity === "conflict"), true);
});
test("removing essence coexistence rechecks the existing selections", () => {
  const selection = initialLoadoutSelection();
  selection.capacities.common = 9999;
  selection.skills.common = skills.filter(row => row.category === "common" && row.type === "essence").slice(0, 2).map(row => row.id);
  assert.equal(warnings(selection).some(row => row.code === "essence"), true);
  selection.equipment.samurai.melee0 = { ...piece(), effects: [{ effect_id: ESSENCE_EFFECTS.common, value: 0 }] };
  assert.equal(warnings(selection).some(row => row.code === "essence"), false);
  selection.capacities.common = 0;
  assert.equal(warnings(selection).some(row => row.code === "capacity"), true);
});
test("known duplicate clan effects are rejected but the two different effects are allowed", () => {
  const selection = initialLoadoutSelection();
  selection.clans.samurai = [{ clanId: 1, rankIndex: 0, slotIndex: 0 }, { clanId: 1, rankIndex: 9, slotIndex: 0 }];
  assert.equal(warnings(selection).some(row => row.code === "clan_duplicate"), true);
  selection.clans.samurai[1]!.slotIndex = 1;
  assert.equal(warnings(selection).some(row => row.code === "clan_duplicate"), false);
});
test("save record identity and native audit survive selection without sharing mutable evidence", () => {
  const saved = row(), selected = savedLoadoutPiece(saved, character());
  assert.equal(selected.source?.slotIndex, 4);
  saved.audit!.findings[0].code = "changed";
  assert.equal(selected.audit?.findings[0].code, "missing_innate");
  const selection = initialLoadoutSelection();
  selection.equipment.shared = { soul0: selected, soul1: structuredClone(selected) };
  assert.equal(warnings(selection).filter(row => row.code === "equipment_duplicate").length, 1);
  assert.equal(warnings(selection).filter(row => row.code === "audit_missing_innate" && row.severity === "conflict").length, 2);
  const decoded = decodeLoadout(encodeLoadout([selected]))[0];
  assert.equal(decoded.source, undefined);
  assert.equal(decoded.audit, undefined);
  assert.deepEqual(decoded.effects.map(effect => [effect.effect_id, effect.value, !!effect.star]), selected.effects.map(effect => [effect.effect_id, effect.value, !!effect.star]));
});
test("core fixed identities come from the native item row; values remain the selected record's values", () => {
  assert.deepEqual(soulCoreDefinition(2669)?.innate_effect_ids, [29053, 4080]);
  const selected = savedLoadoutPiece(row(), character());
  const effects = soulCoreEffects(selected);
  assert.equal(effects.length, 1);
  assert.equal(effects[0].innate, true);
  assert.notEqual(effects[0].display, "数值待确认");
  assert.equal(soulCoreDefinition(0), undefined);
});
test("a natural audited low-rarity core is not rejected by requiring every table innate", () => {
  const selection = initialLoadoutSelection();
  selection.equipment.shared.soul0 = { ...piece(2669), audit: { natural: true, verdict: "natural", findings: [] } };
  assert.equal(warnings(selection).some(row => row.code === "soul_innate"), false);
});
test("current checks do not leak equipment, clan or guardian errors from the inactive style", () => {
  const selection = initialLoadoutSelection();
  selection.equipment.ninja.head = piece();
  selection.clans.ninja = [{ clanId: 1, rankIndex: 0, slotIndex: 0 }, { clanId: 1, rankIndex: 0, slotIndex: 0 }];
  assert.equal(warnings(selection).some(row => row.code === "equipment_slot" || row.code === "clan_duplicate"), false);
  assert.equal(loadoutWarnings(selection, "ninja", attributes).some(row => row.code === "equipment_slot"), true);
});
test("read-only config review never restores an incomplete or old payload", () => {
  const saved = character();
  assert.equal(savedConfigReview(saved).canRestore, false);
  const selected = skills.find(row => row.category === "common")!;
  saved.loadout = { profile: "pc_v2_02", complete: true, attributes: [5, 5, 5, 6, 5, 5, 5], initial_attributes: [5, 5, 5, 6, 5, 5, 5], level: 1,
    capacities: [37, 38, 38], skills: [{ id: selected.id, stored_rank: 1, selected_rank: 1 }, { id: 65535, stored_rank: 1, selected_rank: 1 }],
    guardian_ownership: [], guardian_selection: null, clan_membership: null, equipped_indices: null, soul_core_indices: null, clan_protections: [], destroyed_spike_count: 84, unavailable: [] };
  const before = structuredClone(saved), review = savedConfigReview(saved);
  assert.equal(review.canRestore, false);
  assert.equal(review.selectedSkills.length, 1);
  assert.equal(review.issues.some(row => row.includes("守护灵")), false);
  assert.deepEqual(saved, before);
  saved.loadout!.level = 246;
  assert.equal(savedConfigReview(saved).issues.some(row => row.includes("不一致")), true);
});
function completeCharacter(): SaveCharacter {
  const saved = character(), clan = clans[0], effect = clan.slots[0];
  saved.equipment = [{ ...row(), item_id: 424, set_positions: [0, 17] }, { ...row(), slot_index: 8, inventory_key: 49 }];
  const indices = [Array(17).fill(0xffffffff), Array(17).fill(0xffffffff)];
  indices[0][0] = 4;
  saved.loadout = { profile: "pc_v2_02", complete: true, attributes: [5,5,5,6,5,5,5], initial_attributes: [5,5,5,6,5,5,5], level: 1,
    capacities: [37,38,38], skills: [{ id: skills.find(row => row.category === "common")!.id, selected_rank: 1, stored_rank: 1 }],
    guardian_selection: [292,115], guardian_ownership: [{ id: 292, level: 1, owned: true }, { id: 115, level: 2, owned: true }],
    clan_membership: { clan_id: 0, cached_rank_index: 0 },
    clan_protections: [{ clan_id: clan.clan_id, group_id: effect.effect_group_id, effect_index: effect.slot_index, raw: effect.raw_values_by_rank_index.other_clan[0] },
      ...Array.from({length: 3}, () => ({ clan_id: 0, group_id: 0, effect_index: 0, raw: 0 }))] as NonNullable<SavedLoadout["clan_protections"]>,
    equipped_indices: indices as NonNullable<SavedLoadout["equipped_indices"]>, soul_core_indices: [8,...Array(9).fill(0xffffffff)] as NonNullable<SavedLoadout["soul_core_indices"]>, destroyed_spike_count: 84, unavailable: [] };
  return saved;
}
test("represented configuration builds a detached replacement without guardian selection or cached clan rank", () => {
  const saved = completeCharacter(), before = structuredClone(saved), review = savedConfigReview(saved);
  assert.deepEqual(review.issues, []);
  assert.ok(review.restoration);
  assert.equal(review.restoration.selection.equipment.samurai.melee0.source?.slotIndex, 4);
  assert.equal(review.restoration.selection.equipment.shared.soul0.source?.slotIndex, 8);
  assert.deepEqual(review.restoration.selection.guardians, { samurai: null, ninja: null });
  assert.equal(review.restoration.selection.clans.samurai[0]?.rankIndex, null);
  assert.deepEqual(saved, before);
  review.restoration.selection.equipment.samurai.melee0.audit!.findings[0].code = "changed";
  assert.deepEqual(saved, before);
});
test("complete marker cannot hide missing or unrepresented slots; invalid imports have no partial replacement", () => {
  for (const mutate of [
    (saved: SaveCharacter) => { saved.loadout!.skills!.push({ id: 0xffffffff, selected_rank: 1, stored_rank: 1 }); },
    (saved: SaveCharacter) => { saved.loadout!.equipped_indices![0]![14] = 4; },
    (saved: SaveCharacter) => { saved.loadout!.soul_core_indices![8] = 8; },
    (saved: SaveCharacter) => { saved.loadout!.equipped_indices![0]![15] = 3; saved.loadout!.equipped_indices![0]![16] = 4; },
    (saved: SaveCharacter) => { saved.equipment[0].set_positions = [1,17]; },
  ]) {
    const saved = completeCharacter(); mutate(saved);
    const before = structuredClone(saved), review = savedConfigReview(saved);
    assert.equal(review.canRestore, false);
    assert.equal(review.restoration, null);
    assert.deepEqual(saved, before);
  }
});

test("extra native records remain internal without entering verified inputs or player warnings", () => {
  const saved = completeCharacter();
  saved.equipment.push({ ...row(), slot_index: 9, item_id: 424, set_positions: [2,17] }, { ...row(), slot_index: 10 });
  saved.loadout!.equipped_indices![0]![2] = 9;
  saved.loadout!.soul_core_indices![2] = 10;
  const before = structuredClone(saved), review = savedConfigReview(saved);
  assert.ok(review.restoration);
  const selection = review.restoration.selection;
  assert.equal(selection.savedExtras!.equipment.samurai[0].position, 2);
  assert.equal(selection.savedExtras!.equipment.samurai[0].piece.source?.slotIndex, 9);
  assert.equal(selection.savedExtras!.soulCores[0].piece.source?.slotIndex, 10);
  assert.equal(Object.values(selection.equipment.samurai).length, 1);
  assert.equal(review.notes.some(note => note.includes("额外原生栏位")), false);
  assert.equal(loadoutWarnings(selection, "samurai", attributes).some(row => row.code === "native_extra_pending"), false);
  assert.deepEqual(saved, before);
});

test("duplicate active save slots reject the complete import atomically", () => {
  for (const field of ["weapon", "soul"] as const) {
    const saved = completeCharacter();
    if (field === "weapon") { saved.loadout!.equipped_indices![0]![1] = 4; delete saved.equipment[0].set_positions; }
    else saved.loadout!.soul_core_indices![1] = 8;
    const before = structuredClone(saved), review = savedConfigReview(saved);
    assert.equal(review.canRestore, false);
    assert.equal(review.restoration, null);
    assert.ok(review.issues.some(issue => issue.includes("重复占用")));
    assert.deepEqual(saved, before);
  }
});
test("ignored guardian records never block import; manual guardian levels retain the 1-50 boundary", () => {
  const saved = completeCharacter();
  saved.loadout!.guardian_ownership![0] = { id: 292, level: 50, owned: false };
  assert.equal(savedConfigReview(saved).restoration?.selection.guardians.samurai, null);
  saved.loadout!.guardian_ownership![0].level = 51;
  assert.equal(savedConfigReview(saved).canRestore, true);
  const selection = initialLoadoutSelection();
  selection.guardians.samurai = { guardianId: 292, level: 50 };
  selection.skills.common = [skills.find(row => row.category === "common")!.id];
  assert.equal(warnings(selection).some(row => row.code === "unlocks_pending" || row.code === "guardian_identity"), false);
  selection.guardians.samurai.level = 51;
  assert.equal(warnings(selection).some(row => row.code === "guardian_identity"), true);
});
test("saved clan effects retain their actual value without inventing a rank or requiring cache verification", () => {
  const saved = completeCharacter(), clan = clans.find(row => row.slots.some(effect => new Set(effect.raw_values_by_rank_index.other_clan).size > 1))!;
  const effect = clan.slots.find(effect => new Set(effect.raw_values_by_rank_index.other_clan).size > 1)!;
  const raw = effect.raw_values_by_rank_index.other_clan[9];
  saved.loadout!.clan_protections![0] = { clan_id: clan.clan_id, effect_index: effect.slot_index, group_id: effect.effect_group_id, raw };
  const review = savedConfigReview(saved);
  assert.ok(review.restoration);
  const selection = review.restoration.selection, choice = selection.clans.samurai[0]!;
  assert.equal(choice.rankIndex, null);
  assert.equal(choice.savedRaw, raw);
  assert.ok(clanProtectionValue(selection, choice)?.display);
  assert.equal(loadoutWarnings(selection, "samurai", attributes).some(row => row.code === "clan_rank_pending"), false);
});

test("missing or malformed skipped fields do not bypass validation of required import data", () => {
  const saved = completeCharacter();
  saved.loadout!.guardian_selection = null; saved.loadout!.guardian_ownership = null; saved.loadout!.clan_membership = null;
  saved.loadout!.unavailable = ["guardian_selection", "guardian_ownership", "clan_membership"];
  saved.loadout!.complete = false;
  const before = structuredClone(saved);
  assert.equal(savedConfigReview(saved).canRestore, true);
  assert.deepEqual(saved, before);
  saved.loadout!.unavailable.push("capacities");
  assert.equal(savedConfigReview(saved).canRestore, false);
  saved.loadout!.unavailable = [];
  assert.equal(savedConfigReview(saved).canRestore, false);
});

test("cached membership is ignored; saved clan values remain visible without guessing rank", () => {
  const saved = completeCharacter(), clan = clans[0], effect = clan.slots[0];
  const raw = effect.raw_values_by_rank_index.current_clan[9];
  saved.loadout!.clan_membership = { clan_id: 99999, cached_rank_index: 99999 };
  saved.loadout!.clan_protections![0] = { clan_id: clan.clan_id, group_id: effect.effect_group_id, effect_index: effect.slot_index, raw };
  const review = savedConfigReview(saved);
  assert.ok(review.restoration);
  const selection = review.restoration.selection, choice = selection.clans.samurai[0]!;
  assert.equal(selection.joinedClan, null); assert.equal(choice.rankIndex, null); assert.equal(choice.savedRaw, raw);
  assert.equal(clanProtectionValue(selection, choice)?.display, effect.display_values_by_rank_index.current_clan[effect.raw_values_by_rank_index.current_clan.indexOf(raw)]);
  saved.loadout!.clan_protections![0].raw = 0xffffffff;
  assert.equal(savedConfigReview(saved).canRestore, false);
});

test("applying import preserves both manual guardians and matching-clan ranks but never transfers rank to another clan", () => {
  const review = savedConfigReview(completeCharacter()); assert.ok(review.restoration);
  const current = initialLoadoutSelection(); current.joinedClan = clans[0].clan_id;
  current.guardians = { samurai: { guardianId: 292, level: 50 }, ninja: { guardianId: 115, level: 40 } };
  current.clans.samurai[0] = { clanId: clans[0].clan_id, slotIndex: 1, rankIndex: 4 };
  const before = structuredClone(current), imported = structuredClone(review.restoration);
  const applied = applySavedLoadoutImport(current, review.restoration);
  assert.deepEqual(applied.guardians, current.guardians); assert.equal(applied.joinedClan, current.joinedClan);
  assert.equal(applied.clans.samurai[0]?.rankIndex, 4); assert.equal(applied.clans.samurai[0]?.slotIndex, imported.selection.clans.samurai[0]?.slotIndex);
  assert.equal(applied.clans.samurai[0]?.savedRaw, undefined);
  applied.guardians.samurai!.level = 1;
  assert.deepEqual(current, before); assert.deepEqual(review.restoration, imported);
  current.clans.samurai[0]!.clanId = clans[1].clan_id;
  assert.equal(applySavedLoadoutImport(current, review.restoration).clans.samurai[0]?.rankIndex, null);
});

test("verified native records outside the planner menu are preserved without using its budget", () => {
  const saved = completeCharacter();
  const record = { id: nativeSkillScope.outside_planner_ids[0], selected_rank: 1, stored_rank: 3 };
  saved.loadout!.skills!.push(record);
  const before = structuredClone(saved), review = savedConfigReview(saved);
  assert.ok(review.restoration);
  assert.deepEqual(review.restoration.selection.savedExtras!.otherSkills, [record]);
  assert.equal(Object.values(review.restoration.selection.skills).flat().length, 1);
  review.restoration.selection.savedExtras!.otherSkills![0].stored_rank = 4;
  assert.deepEqual(saved, before);
});
