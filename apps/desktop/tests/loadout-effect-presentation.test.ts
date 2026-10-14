import assert from "node:assert/strict";
import { test } from "node:test";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { EffectLabel, EffectValue, effectText, equipmentEffectPreview } from "../../workshop/effect-presentation";
import { equipmentEffectDetail } from "../../workshop/loadout-effect-summary";
import { setUiLocale } from "../../workshop/presentation";
import { data } from "../../workshop/model";
import catalog from "../../workshop/loadout-effect-catalog.json";
import { clans, guardians } from "../../workshop/loadout-state";
import sets from "../../workshop/loadout-sets.json";

test("all planner effect families render without native markup in every UI locale", () => {
  Object.assign(globalThis, {
    localStorage: { setItem() {} },
    document: { documentElement: { lang: "" } },
  });
  const labels = [
    ...data.editorEffects.map(effect => effect.name),
    ...clans.flatMap(clan => clan.slots.flatMap(effect => [effect.label_zh, effect.description_zh])),
    ...guardians.flatMap(guardian => guardian.slots.flatMap(effect => [effect.label_zh, effect.description_zh])),
    ...Object.values(sets.sets).flatMap(set => set.tiers.map(tier => tier.label_zh)),
  ];
  try {
    for (const locale of ["zh-CN", "en-US", "ja-JP"] as const) {
      setUiLocale(locale);
      for (const label of labels) {
        assert.doesNotMatch(effectText(label), /\^(?:09|20|21|FE|FF)~|~(?:BUFF|DEBUFF)~|[{}]|<dot>/, label);
      }
      for (const id of Object.keys(catalog.effect_to_group).map(Number)) {
        const detail = equipmentEffectDetail(id, 10);
        assert.equal(equipmentEffectPreview(id, 10), [effectText(detail.label), detail.display].filter(Boolean).join(" "));
      }
    }
    setUiLocale("zh-CN");
    assert.equal(effectText("打倒敌人时赋予^09~BUFF~{昂灵}^09~~"), "打倒敌人时赋予昂灵");
    assert.equal(effectText("对处于^09~DEBUFF~{水}^09~~状态敌人的精力伤害"), "对处于水状态敌人的精力伤害");
    assert.equal(effectText("{}计量槽增加量"), "（对象待确认）计量槽增加量");
    assert.equal(renderToStaticMarkup(React.createElement(EffectLabel, { text: "赋予^09~BUFF~{昂灵}^09~~" })), "<span>赋予昂灵</span>");
  } finally {
    setUiLocale("zh-CN");
  }
});

test("effect values distinguish mechanisms, native units, grades, signed and unknown values", () => {
  const render = (display: string | null, raw = 42) => renderToStaticMarkup(React.createElement(EffectValue, { display, raw }));
  assert.equal(render(""), "");
  assert.equal(render("+4.2%"), "<strong>+4.2%</strong>");
  assert.equal(render("AAA+"), "<strong>AAA+</strong>");
  assert.equal(render("-1"), "<strong>-1</strong>");
  assert.match(render(null), /原始数值：42；单位尚未解析.*数值待确认/);
  assert.match(equipmentEffectPreview(0xabcdef, 42), /词条 0xABCDEF 数值待确认/);
});
