// Inject the current style.css + arc.css into the running app, then audit .
// node tools/ui-audit/live-inject-css.mjs <shot> <locale> <WxH> [steps]
import {chromium} from 'playwright';import fs from 'node:fs';import {detectTextFit as detect} from '../../apps/workshop/text-fit-detect.mjs';
const [out, loc, size, steps=''] = process.argv.slice(2);
const css = fs.readFileSync(new URL('../../apps/workshop/style.css', import.meta.url),'utf8') + '\n' + fs.readFileSync(new URL('../../apps/workshop/arc.css', import.meta.url),'utf8');
const b = await chromium.connectOverCDP('http://127.0.0.1:9333'); const p = b.contexts()[0].pages()[0];
const cdp = await p.context().newCDPSession(p); const [w,h]=size.split('x').map(Number);
await cdp.send('Emulation.setDeviceMetricsOverride',{width:w,height:h,deviceScaleFactor:1,mobile:false});
const orig = await p.evaluate(()=>localStorage.getItem('nioh3-ui-locale'));
await p.evaluate(l=>localStorage.setItem('nioh3-ui-locale',l),loc); await p.reload(); await p.waitForTimeout(2500);
await p.evaluate(c=>{for(const s of document.querySelectorAll('style'))s.textContent='';const e=document.createElement('style');e.textContent=c;document.head.append(e)},css);
if (steps) await eval(`(async()=>{${steps}})()`);
await p.waitForTimeout(800); await p.screenshot({path: out});
console.log(JSON.stringify(await p.evaluate(detect)), JSON.stringify(await p.evaluate(()=>{const r=document.querySelector('.window-close')?.getBoundingClientRect();return {close:r&&[Math.round(r.left),Math.round(r.right)],iw:innerWidth}})));
await p.evaluate(o=>o?localStorage.setItem('nioh3-ui-locale',o):localStorage.removeItem('nioh3-ui-locale'),orig);
await cdp.send('Emulation.clearDeviceMetricsOverride'); process.exit(0);
