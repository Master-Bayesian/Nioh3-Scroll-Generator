import { build } from 'esbuild';
import { mkdir, writeFile } from 'node:fs/promises';
await mkdir('apps/tauri/dist', { recursive: true });
await build({entryPoints:['apps/tauri/entry.ts'],outfile:'apps/tauri/dist/app.js',bundle:true,platform:'browser',format:'esm',minify:true,jsx:'transform',jsxFactory:'localizedElement',tsconfigRaw:{compilerOptions:{jsx:'react',jsxFactory:'localizedElement'}},inject:['apps/workshop/presentation-jsx.ts'],define:{'process.env.NODE_ENV':'"production"'}});
await writeFile('apps/tauri/dist/index.html', '<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>独脚踏鞴工作室</title><link rel="stylesheet" href="app.css"></head><body><div id="root"></div><script type="module" src="app.js"></script></body></html>');
