// Rebuild the UI half of apps/workshop/ui-locales.json from ui-translations.tsv.
// The game-name half is game data: it is kept exactly as committed.
import {readFileSync, writeFileSync} from 'node:fs';
import {dirname, join} from 'node:path';
import {fileURLToPath} from 'node:url';

const base = join(dirname(fileURLToPath(import.meta.url)), '../apps/workshop');
const target = join(base, 'ui-locales.json');
const ui = {};
for (const line of readFileSync(join(base, 'ui-translations.tsv'), 'utf8').split(/\r?\n/)) {
  if (!line) continue;
  const fields = line.split('\t');
  if (fields.length !== 3) throw new Error(`Expected key, English and Japanese: ${line}`);
  const [key, english, japanese] = fields;
  if (Object.hasOwn(ui, key)) throw new Error(`Duplicate UI source key: ${key}`);
  ui[key] = [english, japanese];
}
const {game} = JSON.parse(readFileSync(target, 'utf8'));
writeFileSync(target, JSON.stringify({ui, game}, null, 2) + '\n', 'utf8');
console.log(`Exported ${Object.keys(ui).length} UI messages and kept ${Object.values(game).reduce((n, names) => n + Object.keys(names).length, 0)} game names`);
