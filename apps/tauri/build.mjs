import { build } from 'esbuild';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { withDarkTheme, arcTone } from '../workshop/dark-theme.mjs';
await mkdir('apps/tauri/dist', { recursive: true });
await build({entryPoints:['apps/tauri/entry.ts'],outfile:'apps/tauri/dist/app.js',bundle:true,platform:'browser',format:'esm',minify:true,jsx:'transform',jsxFactory:'localizedElement',tsconfigRaw:{compilerOptions:{jsx:'react',jsxFactory:'localizedElement'}},inject:['apps/workshop/presentation-jsx.ts'],define:{'process.env.NODE_ENV':'"production"'}});
// The dark theme (#14) is derived from the built light stylesheet, as the
// browser build does; without it the 深色 setting changed nothing here.
await writeFile('apps/tauri/dist/app.css', withDarkTheme(await readFile('apps/tauri/dist/app.css', 'utf8'), arcTone));
await writeFile('apps/tauri/dist/index.html', '<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>独脚踏鞴工作室</title><link rel="stylesheet" href="app.css"></head><body><div id="root"></div><script type="module" src="app.js"></script></body></html>');
