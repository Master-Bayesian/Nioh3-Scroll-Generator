import {build} from 'esbuild';
import {writeFile,mkdir} from 'node:fs/promises';
const output='deliverables/frontend-v2/search-ui-demo-v2';
await mkdir(output,{recursive:true});
const result=await build({entryPoints:['apps/workshop/main.tsx'],bundle:true,write:false,outdir:'out',format:'iife',platform:'browser',minify:true,jsx:'transform',jsxFactory:'localizedElement',tsconfigRaw:{compilerOptions:{jsx:'react',jsxFactory:'localizedElement'}},inject:['apps/workshop/presentation-jsx.ts'],external:['game-reference.png'],define:{'process.env.NODE_ENV':'"production"'}});
const js=result.outputFiles.find(f=>f.path.endsWith('.js')).text.replaceAll('</script','<\\/script');
const css=result.outputFiles.find(f=>f.path.endsWith('.css')).text;
await writeFile(`${output}/index.html`,`<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>独脚踏鞴工作室 · 绘卷搜索</title><style>${css}</style></head><body><div id="root"></div><script>${js}</script></body></html>`);
console.log(`Built ${output}/index.html`);
