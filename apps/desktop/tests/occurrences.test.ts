import test from "node:test";
import assert from "node:assert/strict";
import {
  conditionKey,
  data,
  enemyCanBePossessed,
  initialQuery,
  matchesEffectOccurrences,
  queryProblem,
} from "../../workshop/model";
import { formQuery, workerQuery } from "../../workshop/desktop-bridge";
import { publicError } from "../../workshop/public-errors";

test("Duplicate effects keep separate thresholds and restore distinct draggable choices", () => {
  const q = initialQuery();
  q.unrestricted = true;
  q.effects = [
    {
      id: "1",
      choiceId: "first",
      name: "Life",
      mode: 0,
      roll: 90,
      cross: false,
    },
    {
      id: "1",
      choiceId: "second",
      name: "Life",
      mode: 0,
      roll: 80,
      cross: false,
    },
  ];
  const wire = workerQuery(q);
  assert.equal(wire.effect_occurrences?.length, 2);
  assert.deepEqual(wire.required_secondary_ids, [1]);
  assert.deepEqual(wire.minimum_roll_percent_by_effect_id, [[1, 80]]);
  const restored = formQuery({ query: wire, result_count: 25 } as any);
  assert.equal(restored.effects.length, 2);
  assert.notEqual(
    conditionKey(restored.effects[0]),
    conditionKey(restored.effects[1]),
  );
  const sample = {
    ...data.samples[0],
    effects: [{ id: "1", name: "Life", role: "主词条", raw: 0, roll: 95 }],
  };
  assert.equal(matchesEffectOccurrences(sample, q), false);
  sample.effects.push({ ...sample.effects[0], role: "副词条", roll: 85 });
  assert.equal(matchesEffectOccurrences(sample, q), true);
});

test("Enemy occurrence groups use the solo product mode and exact Possessed keys", () => {
  const q = initialQuery();
  q.enemyVariant = "expedition";
  q.enemies = [
    {
      id: "possessed",
      name: "Possessed enemy",
      keys: [3903],
      possessedKeys: [3903],
      mode: 7,
      state: "possessed",
      availability: "expedition_only",
    },
    {
      id: "alternative",
      name: "Alternative enemy",
      keys: [3904],
      possessedKeys: [],
      mode: 7,
      state: "any",
      availability: "base",
    },
  ];

  const wire = workerQuery(q);
  assert.equal(wire.enemy_variant, "solo");
  assert.deepEqual(wire.enemy_occurrence_groups, [
    [
      {
        lookup_keys: [3903],
        state: "possessed",
        availability: "any",
      },
      { lookup_keys: [3904], state: "any", availability: "any" },
    ],
  ]);

  const restored = formQuery({ query: wire, result_count: 25 } as any);
  assert.equal(restored.enemyVariant, "solo");
  assert.equal(restored.enemies.length, 2);
  assert.ok(restored.enemies.every((enemy) => enemy.mode === 1));
  assert.equal(restored.enemies[0].state, "possessed");
  assert.equal(restored.enemies[0].availability, "any");
  assert.equal(restored.enemies[1].availability, "any");
});

test("Possessed conditions are offered only for catalog-confirmed eligible enemies", () => {
  const eligible = data.enemies.find((enemy) => enemy.possessedKeys.length)!;
  const ineligibleLow = data.enemies.find(
    (enemy) => enemy.tier === "低手" && !enemy.possessedKeys.length,
  )!;
  assert.equal(
    enemyCanBePossessed({ id: eligible.id, keys: eligible.keys }),
    true,
  );
  assert.equal(
    enemyCanBePossessed({ id: ineligibleLow.id, keys: ineligibleLow.keys }),
    false,
  );

  const q = initialQuery();
  q.enemies = [{
    ...ineligibleLow,
    mode: 0,
    state: "possessed",
    availability: "any",
  }];
  assert.match(queryProblem(q, true), /这个敌人没有地狱附身变体/);
  assert.equal(
    publicError(
      "INVALID_REQUEST: Possessed is available only for eligible low-pool enemy variants",
    ),
    "这个敌人没有地狱附身变体，请关闭开关或选择标有“可附身”的低手敌人。",
  );
});
