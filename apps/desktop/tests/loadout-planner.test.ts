import assert from "node:assert/strict";
import { test } from "node:test";
import {
  canAddSkill,
  canSelectClanProtection,
  clans,
  clanRanks,
  clanProtectionValue,
  clanValues,
  ESSENCE_EFFECTS,
  essenceLimit,
  initialLoadoutSelection,
  setClanProtection,
  skills,
  skillUsage,
} from "../../workshop/loadout-state";
import type { LoadoutPiece } from "../../workshop/loadout-code";
import catalog from "../../workshop/catalog.json";

const piece = (...ids: number[]): LoadoutPiece => ({ item_id: 424, level: 180, level_before_forge: 180, plus: 20, rarity: 4, effects: ids.map(effect_id => ({ effect_id, value: 0 })) });

test("native catalogs preserve named rows, duplicate names, zero costs and essence types", () => {
  assert.equal(clans.length, 50);
  assert.equal(clans.reduce((total, clan) => total + clan.slots.length, 0), 100);
  assert.equal(skills.length, 149);
  assert.equal(new Set(skills.map(skill => skill.id)).size, 149);
  assert.equal(skills.find(skill => skill.id === 53749)?.cost, 0);
  assert.equal(skills.find(skill => skill.id === 53030)?.cost, 0);
  assert.equal(skills.filter(skill => skill.name === "重来之法").length, 2);
});

test("only equipped exact effect IDs raise the corresponding active style limit", () => {
  const selection = initialLoadoutSelection();
  for (const category of ["common", "samurai", "ninja"] as const) assert.equal(essenceLimit(selection, "samurai", category), 1);
  selection.equipment.samurai.melee0 = piece(ESSENCE_EFFECTS.samurai);
  assert.equal(essenceLimit(selection, "samurai", "samurai"), 2);
  assert.equal(essenceLimit(selection, "samurai", "common"), 1);
  assert.equal(essenceLimit(selection, "ninja", "samurai"), 1);
  selection.equipment.samurai.melee1 = piece(ESSENCE_EFFECTS.samurai);
  assert.equal(essenceLimit(selection, "samurai", "samurai"), 2);
  selection.equipment.shared.soul0 = piece(ESSENCE_EFFECTS.common, ESSENCE_EFFECTS.ninja);
  assert.equal(essenceLimit(selection, "ninja", "common"), 2);
  assert.equal(essenceLimit(selection, "ninja", "ninja"), 2);
  delete selection.equipment.shared.soul0;
  assert.equal(essenceLimit(selection, "ninja", "common"), 1);
});

test("skill selection uses independent native budgets and retains invalid choices on equipment removal", () => {
  const selection = initialLoadoutSelection();
  const common = skills.filter(skill => skill.category === "common" && skill.type === "essence");
  assert(canAddSkill(selection, "samurai", common[0].id));
  selection.skills.common.push(common[0].id);
  assert(!canAddSkill(selection, "samurai", common[0].id));
  assert(!canAddSkill(selection, "samurai", common[1].id));
  selection.equipment.samurai.melee0 = piece(ESSENCE_EFFECTS.common);
  assert(canAddSkill(selection, "samurai", common[1].id));
  selection.skills.common.push(common[1].id);
  assert(!canAddSkill(selection, "samurai", common[2].id));
  assert.equal(skillUsage(selection, "common").essences, 2);
  assert.equal(essenceLimit(selection, "ninja", "common"), 1);
  delete selection.equipment.samurai.melee0;
  assert.equal(skillUsage(selection, "common").essences, 2);
  assert.equal(essenceLimit(selection, "samurai", "common"), 1);
  selection.capacities.samurai = 0;
  assert(!canAddSkill(selection, "samurai", 36588));
  assert(canAddSkill(selection, "samurai", 53749));
  assert(!canAddSkill(selection, "samurai", 53030));
  assert.equal(skillUsage(selection, "samurai").cost, 0);
});

test("clan values use an explicit rank and the joined clan's native arrays", () => {
  const selection = initialLoadoutSelection();
  assert(!("ranks" in selection));
  assert.equal(clanValues(selection, 1)[0].display, "+10.0%");
  assert.equal(clanValues(selection, 1, 1)[0].display, "+10.6%");
  selection.joinedClan = 1;
  assert.equal(clanValues(selection, 1)[0].display, "+10.6%");
  assert.equal(clanValues(selection, 1, 9)[0].display, "+14.2%");
  assert.equal(clanValues(selection, 2, 0)[0].display, clans.find(clan => clan.clan_id === 2)!.slots[0].display_values_by_rank_index.other_clan[0]);
});

test("clan protection slots select one native effect and preserve mixed-clan choices", () => {
  const selection = initialLoadoutSelection();
  const odaSlot0 = { clanId: 1, slotIndex: 0, rankIndex: 1 };
  const tokugawaSlot1 = { clanId: 2, slotIndex: 1, rankIndex: 3 };
  const nativeSnapshot = JSON.stringify(clans);

  const withOda = setClanProtection(selection, "samurai", 0, odaSlot0);
  const mixed = setClanProtection(withOda, "samurai", 1, tokugawaSlot1);

  assert.deepEqual(mixed.clans.samurai, [odaSlot0, tokugawaSlot1]);
  assert.deepEqual(mixed.clans.ninja, [null, null]);
  assert.equal(clanProtectionValue(mixed, odaSlot0)?.clan_name, "织田");
  assert.equal(clanProtectionValue(mixed, odaSlot0)?.slot_index, 0);
  assert.equal(clanProtectionValue(mixed, odaSlot0)?.display, clanValues(mixed, 1, 1)[0].display);
  assert.equal(clanProtectionValue(mixed, tokugawaSlot1)?.clan_name, "德川");
  assert.equal(clanProtectionValue(mixed, tokugawaSlot1)?.slot_index, 1);
  assert.equal(clanProtectionValue(mixed, tokugawaSlot1)?.display, clanValues(mixed, 2, 3)[1].display);
  assert.equal(JSON.stringify(clans), nativeSnapshot);
});

test("each clan row keeps its own rank, and different effects from one clan are allowed", () => {
  const selection = initialLoadoutSelection();
  const odaSlot0 = { clanId: 1, slotIndex: 0, rankIndex: 1 };
  const odaSlot1 = { clanId: 1, slotIndex: 1, rankIndex: 3 };
  const duplicateAtAnotherRank = { clanId: 1, slotIndex: 0, rankIndex: 8 };
  const withFirst = setClanProtection(selection, "samurai", 0, odaSlot0);
  const withBoth = setClanProtection(withFirst, "samurai", 1, odaSlot1);

  assert.deepEqual(withBoth.clans.samurai, [odaSlot0, odaSlot1]);
  assert.equal(clanProtectionValue(withBoth, odaSlot0)?.display, clanValues(withBoth, 1, 1)[0].display);
  assert.equal(clanProtectionValue(withBoth, odaSlot1)?.display, clanValues(withBoth, 1, 3)[1].display);
  assert(canSelectClanProtection(withBoth, "samurai", 0, odaSlot0));
  assert(canSelectClanProtection(withBoth, "samurai", 1, odaSlot1));
  assert(!canSelectClanProtection(withBoth, "samurai", 1, duplicateAtAnotherRank));
  assert.strictEqual(setClanProtection(withBoth, "samurai", 1, duplicateAtAnotherRank), withBoth);
});

test("pending clan choices, replacement, clearing and style isolation retain only named slots", () => {
  const selection = initialLoadoutSelection();
  const odaSlot0 = { clanId: 1, slotIndex: 0, rankIndex: 1 };
  const freshTokugawaChoice = { clanId: 2, slotIndex: null, rankIndex: 0 };
  assert(canSelectClanProtection(selection, "samurai", 0, freshTokugawaChoice));
  const pending = setClanProtection(selection, "samurai", 0, freshTokugawaChoice);
  assert.deepEqual(pending.clans.samurai, [freshTokugawaChoice, null]);
  assert.equal(clanProtectionValue(pending, freshTokugawaChoice), null);

  const first = setClanProtection(selection, "samurai", 0, odaSlot0);
  const reselected = setClanProtection(first, "samurai", 0, odaSlot0);
  assert.deepEqual(reselected.clans.samurai, [odaSlot0, null]);

  const replaced = setClanProtection(reselected, "samurai", 0, freshTokugawaChoice);
  assert.deepEqual(replaced.clans.samurai, [freshTokugawaChoice, null]);
  const cleared = setClanProtection(replaced, "samurai", 0, null);
  assert.deepEqual(cleared.clans.samurai, [null, null]);

  const ninja = setClanProtection(cleared, "ninja", 1, odaSlot0);
  assert.deepEqual(ninja.clans.samurai, [null, null]);
  assert.deepEqual(ninja.clans.ninja, [null, odaSlot0]);
});

test("joined-clan changes chosen values without changing each row's rank", () => {
  const odaSlot0 = { clanId: 1, slotIndex: 0, rankIndex: 1 };
  const tokugawaSlot0 = { clanId: 2, slotIndex: 0, rankIndex: 2 };
  const selection = setClanProtection(
    setClanProtection(initialLoadoutSelection(), "samurai", 0, odaSlot0),
    "samurai",
    1,
    tokugawaSlot0,
  );
  const before = selection.clans.samurai.map(choice => choice && clanProtectionValue(selection, choice)?.display);

  assert.deepEqual(selection.clans.samurai.map(choice => choice?.rankIndex), [1, 2]);
  assert.deepEqual(before, [clanValues(selection, 1, 1)[0].display, clanValues(selection, 2, 2)[0].display]);
  selection.joinedClan = 1;
  assert.deepEqual(selection.clans.samurai.map(choice => choice?.rankIndex), [1, 2]);
  assert.deepEqual(
    selection.clans.samurai.map(choice => choice && clanProtectionValue(selection, choice)?.display),
    [clanValues(selection, 1, 1)[0].display, clanValues(selection, 2, 2)[0].display],
  );
});

test("invalid clan, slot, rank and row choices are refused", () => {
  const selection = initialLoadoutSelection();
  const valid = { clanId: 1, slotIndex: 0, rankIndex: 0 };
  const invalidChoices = [
    { clanId: 1, slotIndex: 2, rankIndex: 0 },
    { clanId: 1, slotIndex: null, rankIndex: -1 },
    { clanId: 1, slotIndex: null, rankIndex: clanRanks.length },
    { clanId: 999, slotIndex: null, rankIndex: 0 },
  ];

  for (const choice of invalidChoices) {
    assert(!canSelectClanProtection(selection, "samurai", 0, choice));
    assert.strictEqual(setClanProtection(selection, "samurai", 0, choice), selection);
  }
  assert(!canSelectClanProtection(selection, "samurai", 2, valid));
  assert.strictEqual(setClanProtection(selection, "samurai", 2, valid), selection);
});

test("selected scroll effects participate by exact ID and remain style-specific", () => {
  const selection = initialLoadoutSelection();
  selection.scrolls.samurai = {
    ...catalog.samples[0],
    effects: [{ ...catalog.samples[0].effects[0], id: String(ESSENCE_EFFECTS.common) }],
  };
  assert.equal(essenceLimit(selection, "samurai", "common"), 2);
  assert.equal(essenceLimit(selection, "ninja", "common"), 1);
  selection.scrolls.samurai = null;
  assert.equal(essenceLimit(selection, "samurai", "common"), 1);
});
