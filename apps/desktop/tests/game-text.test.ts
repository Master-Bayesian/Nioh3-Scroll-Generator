import { test } from 'node:test';
import assert from 'node:assert/strict';
import resources from '../../workshop/ui-locales.json';
import { plainGameText } from '../../workshop/game-text';

test('Japanese ruby retains base spelling without leaking game markup or readings', () => {
  assert.equal(plainGameText('サルタヒコの^20~default~^FE~RUBY~^21~default~恩寵^FF~RUBY,おんちょう~'), 'サルタヒコの恩寵');
  assert.equal(plainGameText('^20~default~^FE~RUBY~^21~default~柳生^FF~RUBY,やぎゅう~の^20~default~^FE~RUBY~^21~default~影働^FF~RUBY,かげばたら~き'), '柳生の影働き');
  for (const value of Object.values(resources.game['ja-JP'])) assert.doesNotMatch(value, /\^(?:20|21|FE|FF)~|RUBY/);
  for (const value of ['体力 +313', 'Attack +35', 'ダメージ反映（忍術威力）', '100%']) assert.equal(plainGameText(value), value);
});
