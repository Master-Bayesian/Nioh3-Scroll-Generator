import { test } from 'node:test';
import assert from 'node:assert/strict';
import catalog from '../../workshop/item-names.json';
import ui from '../../workshop/ui-locales.json';
import chinese from '../../../nioh3_scroll_editor/data/auxiliary_names/zh-CN.json';
import english from '../../../nioh3_scroll_editor/data/auxiliary_names/en-US.json';
import japanese from '../../../nioh3_scroll_editor/data/auxiliary_names/ja-JP.json';

test('owner-confirmed 0x3336 keeps soul-core classification and Sudama identity in each UI locale', () => {
  assert.deepEqual(catalog.items['13110'], ['魑魅的魂核', '魂核', '', '魂核']);
  assert.equal(catalog.items['48491'][0], '魑魅魂核');
  const key = '0X00007B82';
  assert.equal(chinese.enemies[key].name, '魑魅');
  assert.equal(english.enemies[key].name, 'Sudama');
  assert.equal(japanese.enemies[key].name, '魑魅');
  assert.equal(chinese.enemies[key].text_id, english.enemies[key].text_id);
  assert.equal(chinese.enemies[key].text_id, japanese.enemies[key].text_id);
  const names = (ui.ui as Record<string, string[]>)['魑魅的魂核'];
  assert.deepEqual(names, ['Sudama Soul Core', '魑魅の魂核']);
  assert.ok(!names.some(name => /魍魉|魍魎|Kodama/.test(name)));
});
