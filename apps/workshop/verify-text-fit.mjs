// Text-fit acceptance: every page and populated state of the web build, in each
// locale, at the minimum and a common window size. Fails when text is clipped,
// escapes its box, overlaps other text, or a select hides its chosen option.
// Fixture-only: no game, save or native host.
import {chromium} from 'playwright';
import {pathToFileURL} from 'node:url';
import path from 'node:path';
import {mkdir, readFile, writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {detectTextFit as detect} from './text-fit-detect.mjs';
import {selectedLocales, workshopBundle} from '../../tools/ci/workshop-bundle.mjs';

const root = path.resolve(import.meta.dirname, '../..');
const bundle = workshopBundle(root);
const url = pathToFileURL(bundle).href;
const locales = selectedLocales(['zh-CN', 'en-US', 'ja-JP']);
const output = path.resolve(process.env.NIOH3_CI_TASK_OUTPUT || 'deliverables/frontend-v2/text-fit');
await mkdir(output, {recursive: true});

const states = [
  ['search', async () => {}],
  ['search-conditions', async p => {
    await p.locator('.search-page button',{hasText:/^.?(敌人|Enemy|敵)/}).first().click(); await p.waitForTimeout(300);
    const en=p.locator('.enemy-list .catalog-row button'); for(let i=0;i<3;i++){await en.nth(i).click().catch(()=>{});}
    await p.locator('.search-page button',{hasText:/特殊规则|Special rules|特殊ルール/}).first().click(); await p.waitForTimeout(300);
    for (let i=0;i<4;i++){ const c=p.locator('.rule-category').nth(i); await c.click(); await p.waitForTimeout(150); await p.locator('.rule-picker button').first().click().catch(()=>{}); await c.click(); }
    const cat=p.locator('.rule-category',{hasText:/属性伤害与异常|Elemental Damage|属性ダメージ/}); if(await cat.count()){ await cat.first().click(); await p.waitForTimeout(150); await p.locator('.rule-picker button').first().click().catch(()=>{}); }
    await p.waitForTimeout(300); await p.locator('.selected-body').first().evaluate(e=>e.scrollTop=e.scrollHeight);
  }],
  ...[0, 1, 2, 3, 4].map(i => [`editor-section-${i}`, async p => {
    await p.locator('nav.nav > button').nth(1).click(); await p.waitForTimeout(500);
    const s = p.locator('.editor-work details:not(details details) > summary');
    if (i >= await s.count()) return false;
    await s.nth(i).click(); await p.waitForTimeout(300);
  }]),
  ['favorites', async p => { await p.locator('nav.nav > button').nth(3).click(); }],
];
const browser = await chromium.launch();
const failures = [];
const cases = [];
let completed = false;
try {
for (const [w, h] of [[1100, 740], [1600, 1000]]) {
  for (const locale of locales) {
    for (const [name, run] of states) {
      const page = await browser.newPage({viewport: {width: w, height: h}});
      await page.goto(url);
      await page.evaluate(l => localStorage.setItem('nioh3-ui-locale', l), locale);
      await page.reload(); await page.waitForTimeout(900);
      const applicable = (await run(page)) !== false;
      if (applicable) {
        await page.waitForTimeout(500);
        for (const f of await page.evaluate(detect)) failures.push(`${w}x${h} ${locale} ${name}: ${f.k} "${f.txt ?? ''}" ${f.at ?? ''}`);
      }
      cases.push({viewport: {width:w,height:h}, locale, state:name, status:applicable?'Checked':'NotApplicable'});
      await page.close();
    }
  }
}
completed = true;
} finally {
  await browser.close();
  await writeFile(path.join(output, 'text-fit.json'), JSON.stringify({pass:completed&&failures.length===0,boundary:'Production web fixture only; no native/game/save access',locales,cases,failures,source:{bundleSha256:createHash('sha256').update(await readFile(bundle)).digest('hex')}}, null, 2)+'\n');
}
if (failures.length) { console.error(failures.join(String.fromCharCode(10))); process.exit(1); }
console.log('Text fits in every checked page, state, locale and size');
