/** Project reviewed native item identities and fixed core effects, without inventing abilities. */
import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
const root = new URL('../', import.meta.url);
const source = new URL('nioh3_scroll_editor/data/r4_finalizer/pc_v2_02/resource_v1/', root);
const manifest = JSON.parse(await readFile(new URL('manifest.json', source), 'utf8'));
const bytes = await readFile(new URL('tables/item.bin', source));
const hash = createHash('sha256').update(bytes).digest('hex');
assert.equal(manifest.game_version, 'PC v2.02');
const table = manifest.tables.find(table => table.name === 'item');
assert.equal(table?.file.filename, 'tables/item.bin');
assert.equal(table?.file.sha256.toLowerCase(), hash, 'Item table must match the retained native manifest');
assert.equal(table.file.size, bytes.length);
assert.equal(table.row_size, 0x1A0);
const count = bytes.readUInt32LE(4);
assert.equal(bytes.length, 8 + count * 0x1A0);
assert.equal(table.row_count, count);
const cores = {};
for (let index = 0; index < count; index++) {
  const row = bytes.subarray(8 + index * 0x1A0, 8 + (index + 1) * 0x1A0);
  const type = row.readUInt16LE(0x15C);
  if (type < 54 || type > 57) continue;
  const id = row.readUInt16LE(0x152);
  assert(!cores[id], 'Duplicate native item identity');
  cores[id] = { item_id: id, weight_slot: type, yokai_key: row.readUInt16LE(0x180),
    innate_effect_ids: [row.readUInt16LE(0x158), row.readUInt16LE(0x15A)].filter(Boolean),
    row_sha256: createHash('sha256').update(row).digest('hex') };
}
await writeFile(new URL('apps/workshop/loadout-soul-cores.json', root), JSON.stringify({
  schema: 'nioh3-loadout-soul-cores/v1', game_version: manifest.game_version,
  item_table_sha256: hash, scope: 'Native item identity and innate slots. Selected record values are used unchanged; no ability strength or unlock inference.', cores,
}, null, 2) + '\n');
console.log(`Projected ${Object.keys(cores).length} native soul-core definitions`);
