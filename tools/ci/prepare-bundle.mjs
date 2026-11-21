import {copyFileSync, mkdirSync} from 'node:fs';
import {resolve} from 'node:path';
import {parseArgs} from 'node:util';
import {prepareWorkshopBundle} from './workshop-bundle.mjs';

const {values} = parseArgs({options: {out: {type: 'string'}}});
if (!values.out) throw new Error('--out is required');
const output = resolve(values.out);
mkdirSync(output, {recursive: true});
const manifest = prepareWorkshopBundle(process.cwd(), resolve(output, 'workshop.json'));
copyFileSync(manifest.path, resolve(output, 'workshop.html'));
console.log(JSON.stringify({bundle: resolve(output, 'workshop.json'), sha256: manifest.sha256}));
