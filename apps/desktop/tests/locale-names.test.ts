import { test } from 'node:test';
import assert from 'node:assert/strict';

// presentation.ts reads the stored locale and sets the page language.
const stored = new Map<string, string>();
Object.assign(globalThis, {
  localStorage: { getItem: (key: string) => stored.get(key) ?? null, setItem: (key: string, value: string) => stored.set(key, value) },
  document: { documentElement: {} },
});
const { localize, localizeName, setUiLocale } = await import('../../workshop/presentation');
const { data } = await import('../../workshop/model');
const rawName = (id: number) => data.editorEffects.find(effect => effect.id === String(id))!.name;

test('game names read whole in English and Japanese, never piece by piece', () => {
  setUiLocale('en-US');
  assert.equal(localize('手里剑造成的伤害'), 'Shuriken Damage');
  assert.equal(localize('缝影的持有上限'), 'Max Shadowstitch Stock');
  assert.equal(localize(rawName(63277)), 'Max Shadowstitch Stock');
  assert.equal(localize('使敌人陷入火状态时精华槽增加'), 'Amrita Bonus (Inflict Scorched)');
  assert.equal(localize('守护灵技的伤害、缝影的持有上限'), 'Guardian Spirit Skill Damage, Max Shadowstitch Stock');
  assert.equal(localize('一目连的魂核'), 'Ichimokuren Soul Core');
  assert.equal(localizeName('怪童大铠　腿甲') !== null, true);
  setUiLocale('ja-JP');
  assert.equal(localize('手里剑造成的伤害'), '手裏剣のダメージ');
  assert.equal(localize('火炎龙计量槽增加量'), '火炎龍のゲージ加算量');
  assert.equal(localize('一目连的魂核'), '一目連の魂代');
  assert.equal(localize('可使用道具'), '消費アイテム');
  setUiLocale('zh-CN');
  assert.equal(localize(rawName(63277)), '缝影的持有上限');
});
