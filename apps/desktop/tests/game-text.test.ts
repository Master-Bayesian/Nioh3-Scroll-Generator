import { test } from 'node:test';
import assert from 'node:assert/strict';
import resources from '../../workshop/ui-locales.json';
import { fillTemplateSlots, plainGameText, withTemplateArgument, withoutTemplateArguments } from '../../workshop/game-text';
import effectArguments from '../../workshop/effect-arguments.json';

test('Japanese ruby retains base spelling without leaking game markup or readings', () => {
  assert.equal(plainGameText('サルタヒコの^20~default~^FE~RUBY~^21~default~恩寵^FF~RUBY,おんちょう~'), 'サルタヒコの恩寵');
  assert.equal(plainGameText('^20~default~^FE~RUBY~^21~default~柳生^FF~RUBY,やぎゅう~の^20~default~^FE~RUBY~^21~default~影働^FF~RUBY,かげばたら~き'), '柳生の影働き');
  for (const value of Object.values(resources.game['ja-JP'])) assert.doesNotMatch(value, /\^(?:20|21|FE|FF)~|RUBY/);
  for (const value of ['体力 +313', 'Attack +35', 'ダメージ反映（忍術威力）', '100%']) assert.equal(plainGameText(value), value);
});

test('resolved buff and ailment arguments read like the game, unresolved ones stay generic', () => {
  const buff = '吸收精华后赋予^09~BUFF~{}^09~~';
  const ailment = '使敌人陷入^09~DEBUFF~{}^09~~状态时增加灵力';
  assert.equal(fillTemplateSlots(buff, '增益效果', '异常状态'), '吸收精华后赋予增益效果');
  assert.equal(fillTemplateSlots(ailment, '增益效果', '异常状态'), '使敌人陷入异常状态时增加灵力');
  assert.equal(fillTemplateSlots(withTemplateArgument(buff, '承受伤害减少'), '增益效果', '异常状态'), '吸收精华后赋予承受伤害减少');
  assert.equal(fillTemplateSlots(withTemplateArgument(ailment, '毒'), '增益效果', '异常状态'), '使敌人陷入毒状态时增加灵力');
  assert.equal(withoutTemplateArguments(withTemplateArgument(ailment, '毒')), ailment);
  const names = Object.values(effectArguments.names);
  assert.equal(names.length, 620);
  for (const name of names) assert.doesNotMatch(name, /[{}~^]/);
});
