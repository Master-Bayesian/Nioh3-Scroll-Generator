import { build } from 'esbuild';
import { mkdir, copyFile } from 'node:fs/promises';
await mkdir('apps/desktop/dist', { recursive: true });
await build({ entryPoints: ['apps/desktop/src/main.ts', 'apps/desktop/src/preload.ts'], outdir: 'apps/desktop/dist', bundle: true, platform: 'node', format: 'cjs', outExtension: { '.js': '.cjs' }, external: ['electron', 'original-fs'], sourcemap: true });
await build({ entryPoints: ['apps/desktop/src/renderer.tsx'], outdir: 'apps/desktop/dist', bundle: true, platform: 'browser', format: 'esm', sourcemap: true, define: { 'process.env.NODE_ENV': '"production"' } });
await copyFile('apps/desktop/index.html', 'apps/desktop/dist/index.html');

await build({entryPoints:['apps/workshop/main.tsx'],outfile:'apps/desktop/dist/review.js',bundle:true,platform:'browser',format:'esm',sourcemap:true,jsx:'transform',jsxFactory:'localizedElement',tsconfigRaw:{compilerOptions:{jsx:'react',jsxFactory:'localizedElement'}},inject:['apps/workshop/presentation-jsx.ts'],define:{'process.env.NODE_ENV':'"production"'}});
await copyFile('apps/desktop/review.html','apps/desktop/dist/review.html');

for(const name of ['extract-update.ps1','apply-update.ps1'])await copyFile('apps/desktop/'+name,'apps/desktop/dist/'+name);
