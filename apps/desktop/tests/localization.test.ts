import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { english, locales, messages, translate, normalizeLocale, localizedError, type MessageKey } from '../src/locales';
import { PreferencesStore } from '../src/preferences';
import responseSchema from '../../../packages/contracts/response.schema.json';

test('all shipped languages cover lifecycle states and interpolation contracts', () => {
  const placeholders = (value: string) => [...value.matchAll(/\{(\w+)\}/g)].map(match => match[1]).sort();
  for (const locale of locales) {
    assert.deepEqual(Object.keys(messages[locale]).sort(), Object.keys(english).sort());
    for (const key of Object.keys(english) as MessageKey[]) {
      assert.ok(messages[locale][key].trim());
      assert.deepEqual(placeholders(messages[locale][key]), placeholders(english[key]), `${locale}:${key}`);
    }
    for (const state of responseSchema.definitions.JobSnapshot.properties.state.enum) assert.ok(state in messages[locale]);
    for (const state of responseSchema.definitions.JobSnapshot.properties.stop_reason.anyOf[0].enum!) assert.ok(state in messages[locale]);
    assert.equal(translate(locale, 'candidateCount', { count: 10000 }).includes('10,000'), true);
    assert.throws(() => translate(locale, 'cursor'), /MISSING_MESSAGE_ARGUMENT/);
  }
  assert.equal(normalizeLocale('ja_JP'), 'ja-JP');
  assert.equal(normalizeLocale('zh-TW'), 'zh-CN');
  assert.equal(normalizeLocale('fr-FR'), 'en-US');
  assert.equal(localizedError('ja-JP', 'INVALID_REQUEST'), messages['ja-JP'].invalidInput);
});

test('language preference survives reopening and concurrent writes without accepting arbitrary data', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'nioh-v2-locale-'));
  const path = join(directory, 'prefs.json');
  const store = new PreferencesStore(path, 'ja-JP');
  assert.equal(await store.getLocale(), 'ja-JP');
  await Promise.all([store.setLocale('en-US'), store.setLocale('zh-CN'), store.setLocale('ja-JP')]);
  assert.equal(await new PreferencesStore(path, 'en-US').getLocale(), 'ja-JP');
  const original = await readFile(path, 'utf8');
  await assert.rejects(store.setLocale('../other-file'), /INVALID_REQUEST/);
  assert.equal(await readFile(path, 'utf8'), original);
  await writeFile(path, '{broken');
  assert.equal(await store.getLocale(), 'ja-JP');
});
