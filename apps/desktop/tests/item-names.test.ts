import { test } from 'node:test';
import assert from 'node:assert/strict';
import catalog from '../../workshop/item-names.json';
import ui from '../../workshop/ui-locales.json';
import yokai from '../../../test_fixtures/soul_core_yokai.json';

type Core = { placeholder?: boolean; 'zh-CN'?: string; 'en-US'?: string; 'ja-JP'?: string };
const cores = yokai.cores as Record<string, Core>;
const items = catalog.items as Record<string, string[]>;

test('every real soul core carries the name the game builds from its yokai ({}的魂核)', () => {
  const real = Object.entries(cores).filter(([, core]) => !core.placeholder);
  assert.equal(real.length, 84);
  for (const [id, core] of real) {
    assert.deepEqual(items[id], [core['zh-CN'] + '的魂核', '魂核', '', '魂核'], id);
    assert.ok(core['en-US'] && core['ja-JP'], id);
  }
});

test('placeholder soul cores stay unnamed and therefore hidden', () => {
  for (const [id, core] of Object.entries(cores)) if (core.placeholder) assert.equal(items[id]?.[0] ?? '', '', id);
});

test('0x3336 is the Hell Wind jailer core and Sudama keeps its own item and translations', () => {
  assert.equal(items['13110'][0], '狱卒鬼（业风）的魂核');
  assert.equal(items['48491'][0], '魑魅的魂核');
  assert.deepEqual((ui.ui as Record<string, string[]>)['魑魅的魂核'], ['Sudama Soul Core', '魑魅の魂核']);
});
