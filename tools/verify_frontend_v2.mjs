import { resolve } from 'node:path';
import { verifyPortable } from '../packages/packaging/integrity.mjs';
if (!process.argv[2]) throw new Error('Pass the portable directory to verify');
console.log(JSON.stringify(await verifyPortable(resolve(process.argv[2])), null, 2));
