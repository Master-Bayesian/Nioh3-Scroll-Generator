import { readFile, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { resolve } from "node:path";

// Use the reviewed native capture; historical mixed/wiki catalogs are not inputs.
const sourcePath = process.argv[2];
if (!sourcePath) throw new Error("Usage: node tools/export_loadout_guardians.mjs <guardian_spirits_native.json>");
const bytes = await readFile(sourcePath);
const source = JSON.parse(bytes.toString("utf8"));
if (source.schema !== "nioh3-native-guardian-catalog/v2" ||
    !source.source_policy.native_only || !source.source_policy.eligible_as_native_planner_dataset)
  throw new Error("Expected the reviewed native guardian catalog v2");
const guardians = source.guardians.map(row => ({
  guardian_id: row.guardian_id, name_zh: row.name_zh, style: row.style,
  slots: row.slots.map(slot => ({
    slot: slot.slot, effect_group_id: slot.effect_group_id,
    label_zh: slot.label_zh, description_zh: slot.description_zh,
    activation_threshold: slot.activation_threshold, value_kind: slot.value_kind,
    raw_values_by_level_index: slot.raw_values_by_level_index,
    display_values_by_level_index: slot.display_values_by_level_index,
  })),
}));
if (guardians.length !== 22 || new Set(guardians.map(row => row.guardian_id)).size !== 22 ||
    guardians.some(row => !["samurai", "ninja"].includes(row.style) || row.slots.length !== 5 ||
      row.slots.some(slot => slot.display_values_by_level_index.length !== 101)))
  throw new Error("Unexpected guardian coverage");
const output = {
  schema: "nioh3-loadout-guardian-catalog/v1", status: source.status,
  source_sha256: createHash("sha256").update(bytes).digest("hex"),
  game_identity: source.game_identity, source_policy: source.source_policy,
  coverage: source.coverage, contract: source.contract, guardians,
};
await writeFile(resolve("apps/workshop/loadout-guardians.json"), JSON.stringify(output, null, 2) + "\n");
console.log(`Exported ${guardians.length} guardians and 110 fixed effects`);
