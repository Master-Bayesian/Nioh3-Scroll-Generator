// Records every element's box in the zh-CN states :
//   node tools/ui-audit/web-measure.mjs <out.json>
import {chromium} from 'playwright';
import fs from 'node:fs';
const [out] = process.argv.slice(2);
const url = new URL('../../deliverables/frontend-v2/search-ui-demo-v2/index.html', import.meta.url).href;
const searchSteps = fs.readFileSync(new URL('./search-steps.js', import.meta.url), 'utf8');
const states = [
  ['search0', async () => {}],
  ['search', async p => { await eval(`(async()=>{${searchSteps}})()`); }],
  ...[0, 1, 2, 3, 4].map(i => [`editor${i}`, async p => {
    await p.locator('nav.nav > button').nth(1).click(); await p.waitForTimeout(500);
    const s = p.locator('.editor-work details:not(details details) > summary');
    if (i < await s.count()) { await s.nth(i).click(); await p.waitForTimeout(300); }
  }]),
  ['favorites', async p => { await p.locator('nav.nav > button').nth(3).click(); }],
];
const b = await chromium.launch();
const result = {};
for (const size of ['1100x740', '1600x1000']) {
  const [w, h] = size.split('x').map(Number);
  for (const [name, run] of states) {
    const p = await b.newPage({viewport: {width: w, height: h}});
    await p.goto(url); await p.waitForTimeout(900);
    await run(p); await p.waitForTimeout(600);
    result[`${size} ${name}`] = await p.evaluate(() => {
      const key = el => { const a = []; for (let e = el; e && e !== document.body; e = e.parentElement) { const i = [...e.parentElement.children].indexOf(e); a.unshift(e.tagName.toLowerCase() + (typeof e.className === 'string' && e.className ? '.' + e.className.trim().split(/\s+/).join('.') : '') + ':' + i); } return a.join('>'); };
      const o = {};
      for (const el of document.querySelectorAll('body *')) { const r = el.getBoundingClientRect(); if (r.width || r.height) o[key(el)] = [Math.round(r.width), Math.round(r.height)]; }
      return o;
    });
    await p.close();
  }
}
await b.close();
fs.writeFileSync(out, JSON.stringify(result));
console.log('measured', Object.keys(result).length, 'states');
