/** Planner storage schema; used only by the standalone validator generator. */
const integer = { type: "integer", minimum: 0, maximum: 0xffffffff };
const nullable = (schema) => ({ anyOf: [schema, { type: "null" }] });
const array = (items) => ({ type: "array", items });
const object = (properties, required = Object.keys(properties)) => ({ type: "object", properties, required });
const styles = (schema) => object({ samurai: schema, ninja: schema });
const categories = (schema) => object({ common: schema, samurai: schema, ninja: schema });
const piece = object({
  item_id: integer, level: integer, level_before_forge: integer, plus: integer, rarity: integer,
  effects: array(object({ effect_id: integer, value: integer, star: { type: "boolean" } }, ["effect_id", "value"])),
  familiarity: integer, armor_remodel: { ...array(integer), minItems: 2, maxItems: 2 }, native_flags: integer,
  remodel_stage: { type: "integer", minimum: 1, maximum: 5 },
  origin: { enum: ["save", "favorites", "code"] }, hell: { type: "boolean" }, hell_skill: integer,
  source: object({ saveId: { type: "string" }, digest: { type: "string" }, slotIndex: integer, inventoryKey: integer }),
  audit: object({ natural: { type: "boolean" }, verdict: { enum: ["natural", "unverified", "unnatural"] },
    findings: array(object({ code: { type: "string" }, slot: integer }, ["code"])),
    unverified: array(object({ code: { type: "string" }, slot: integer }, ["code"])) }, ["natural", "findings"]),
}, ["item_id", "level", "level_before_forge", "plus", "rarity", "effects"]);
const scroll = object({
  seed: { type: "string" }, rarity: integer, capacity: integer, terrainKeys: array(integer),
  enemyKeys: array(integer), enemySlotKeys: array(integer), level: integer, playthrough: integer,
  enemies: array({ type: "string" }), rules: array(object({ name: { type: "string" }, value: { type: "string" } })),
  effects: array(object({ id: { type: "string", pattern: "^[0-9]+$" }, raw: integer, name: { type: "string" },
    role: { type: "string" }, roll: { type: "number" } })),
}, ["seed", "rarity", "capacity", "terrainKeys", "enemies", "rules", "effects"]);
const selection = object({
  amritaGaugePercent: nullable({ type: "number", minimum: 0, maximum: 100 }),
  remodelStage: { type: "integer", minimum: 1, maximum: 5 },
  armorRemodels: styles({ type: "object", properties: Object.fromEntries(["head", "body", "arms", "legs", "feet"].map(slot =>
    [slot, { enum: ["0/0", "1/0", "2/0", "1/1", "2/2", "1/2"] }])), additionalProperties: false }),
  equipment: object({ ...styles({ type: "object", additionalProperties: piece }).properties, shared: { type: "object", additionalProperties: piece } }),
  scrolls: styles(nullable(scroll)),
  clans: styles({ ...array(nullable(object({ clanId: integer, slotIndex: nullable(integer), rankIndex: nullable(integer), savedRaw: integer }, ["clanId", "slotIndex", "rankIndex"]))), minItems: 2, maxItems: 2 }),
  joinedClan: nullable(integer), guardians: styles(nullable(object({ guardianId: integer, level: { type: "integer", minimum: 1, maximum: 50 } }))),
  destroyedSpikeCount: nullable(integer), skills: categories(array(integer)), capacities: categories(nullable({ type: "integer", minimum: 0, maximum: Number.MAX_SAFE_INTEGER })),
  savedExtras: object({ equipment: styles(array(object({ position: integer, piece }))), scrolls: styles(array(scroll)),
    soulCores: array(object({ position: integer, piece })), otherSkills: array(object({ id: integer, stored_rank: integer, selected_rank: integer })) }, ["equipment", "scrolls", "soulCores"]),
}, ["equipment", "scrolls", "clans", "joinedClan", "guardians", "destroyedSpikeCount", "skills", "capacities"]);
export default object({ version: { const: 1 }, activeId: integer,
  drafts: { ...array(object({ id: { type: "integer", minimum: 1 }, name: { type: "string", minLength: 1, maxLength: 60 }, style: { enum: ["samurai", "ninja"] },
    attributes: { ...array(object({ name: { type: "string" }, value: { type: "integer", minimum: 5, maximum: 150 } })), minItems: 7, maxItems: 7 }, selection })), minItems: 1 } });
