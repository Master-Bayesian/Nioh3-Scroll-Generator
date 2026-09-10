import assert from "node:assert/strict";
import { test } from "node:test";
import {
  ANY_RULE_VALUE,
  initialQuery,
  ruleFamilyKeys,
  ruleFamilyValues,
} from "../../workshop/model";
import { workerQuery } from "../../workshop/desktop-bridge";

test("rule families support any target with a shared value", () => {
  assert.deepEqual(ruleFamilyValues("一难横行"), ["50%", "65%", "80%"]);
  assert.equal(ruleFamilyKeys("一难横行", ANY_RULE_VALUE).length, 69);
  assert.equal(ruleFamilyKeys("一难横行", "80%").length, 23);
  assert.deepEqual(ruleFamilyValues("造成的属性伤害增加"), ["10%", "15%", "30%"]);
  assert.equal(ruleFamilyKeys("造成的属性伤害增加", "30%").length, 4);
});

test("worker query preserves a complete rule-family alternative group", () => {
  const keys = ruleFamilyKeys("一难横行", ANY_RULE_VALUE);
  const query = workerQuery({
    ...initialQuery(),
    rules: [
      {
        id: "category:一难横行",
        name: "一难横行",
        keys,
        variant: ANY_RULE_VALUE,
      },
    ],
  });
  assert.deepEqual(query.auxiliary.required_special_rule_key_groups, [keys]);
});
