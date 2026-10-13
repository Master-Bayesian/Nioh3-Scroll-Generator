// Stress audit of the web preview : every page and populated state,
// in each locale plus a pseudo locale that lengthens every text by half.
//   node tools/ui-audit/web-stress.mjs <outdir> [WxH,...]
import {chromium} from 'playwright';
import fs from 'node:fs';
import {detectTextFit as detect} from '../../apps/workshop/text-fit-detect.mjs';
const [out, sizes = '1100x740,1600x1000'] = process.argv.slice(2);
fs.mkdirSync(out, {recursive: true});
const url = new URL('../../deliverables/frontend-v2/search-ui-demo-v2/index.html', import.meta.url).href;
const searchSteps = fs.readFileSync(new URL('./search-steps.js', import.meta.url), 'utf8');
const lengthen = () => {
  const walk = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
  for (let n; (n = walk.nextNode());) {
    const t = n.textContent;
    if (t.trim().length > 1 && !n.parentElement.closest('script,style,option')) n.textContent = t + t.slice(0, Math.ceil(t.length / 2));
  }
  for (const o of document.querySelectorAll('option')) o.textContent += o.textContent.slice(0, Math.ceil(o.textContent.length / 2));
};
const states = [
  ['search', async p => { await eval(`(async()=>{${searchSteps}})()`); }],
  ...[0, 1, 2, 3, 4, 5].map(i => [`editor${i}`, async p => {
    await p.locator('nav.nav > button').nth(1).click(); await p.waitForTimeout(500);
    const s = p.locator('.editor-work details:not(details details) > summary');
    if (i >= await s.count()) return false;
    await s.nth(i).click(); await p.waitForTimeout(300);
  }]),
  ['favorites', async p => { await p.locator('nav.nav > button').nth(3).click(); }],
  ['settings', async p => { await p.locator('.topbar .icon-button').nth(1).click(); }],
];
const b = await chromium.launch();
const report = {};
for (const size of sizes.split(',')) {
  const [w, h] = size.split('x').map(Number);
  for (const loc of ['zh-CN', 'en-US', 'ja-JP', 'pseudo']) {
    for (const [name, run] of states) {
      const p = await b.newPage({viewport: {width: w, height: h}});
      await p.goto(url);
      await p.evaluate(l => localStorage.setItem('nioh3-ui-locale', l), loc === 'pseudo' ? 'en-US' : loc);
      await p.reload(); await p.waitForTimeout(900);
      try { if ((await run(p)) === false) { await p.close(); continue; } } catch (e) { report[`${size} ${loc} ${name}`] = [{k: 'step-failed', txt: String(e).slice(0, 80)}]; await p.close(); continue; }
      await p.waitForTimeout(500);
      if (loc === 'pseudo') { await p.evaluate(lengthen); await p.waitForTimeout(200); }
      await p.screenshot({path: `${out}/${size}_${loc}_${name}.png`});
      const found = await p.evaluate(detect);
      if (found.length) report[`${size} ${loc} ${name}`] = found;
      await p.close();
    }
  }
}
await b.close();
fs.writeFileSync(`${out}/report.json`, JSON.stringify(report, null, 1));
let total = 0;
for (const [k, v] of Object.entries(report)) { total += v.length; console.log('##', k); for (const r of v) console.log('  ', r.k, JSON.stringify(r.txt ?? ''), r.at ?? '', r.d ?? ''); }
console.log('TOTAL', total);
