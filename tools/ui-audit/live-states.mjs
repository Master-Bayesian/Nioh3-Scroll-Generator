// Audit inner states of the running app : node tools/ui-audit/live-states.mjs <outdir> <locale>
import {chromium} from 'playwright';
import fs from 'node:fs';
import {detectTextFit as detect} from '../../apps/workshop/text-fit-detect.mjs';
const [out, loc = 'en-US'] = process.argv.slice(2);
fs.mkdirSync(out, {recursive: true});
const b = await chromium.connectOverCDP('http://127.0.0.1:9333');
const p = b.contexts()[0].pages()[0];
const cdp = await p.context().newCDPSession(p);
const orig = await p.evaluate(() => localStorage.getItem('nioh3-ui-locale'));
await p.evaluate(l => localStorage.setItem('nioh3-ui-locale', l), loc);
const report = {};
for (const size of ['1100x740', '1600x1000']) {
  const [w, h] = size.split('x').map(Number);
  await cdp.send('Emulation.setDeviceMetricsOverride', {width: w, height: h, deviceScaleFactor: 1, mobile: false});
  await p.reload(); await p.waitForTimeout(2500);
  const shot = async name => { await p.waitForTimeout(900); await p.screenshot({path: `${out}/${loc}_${size}_${name}.png`}); const f = await p.evaluate(detect); if (f.length) report[`${size} ${name}`] = f; };
  const nav = i => p.locator('nav.nav > button').nth(i).click();
  // editor: open every section
  await nav(1); await p.waitForTimeout(1500);
  const sections = p.locator('.editor-work details:not(details details) > summary');
  for (let i = 0; i < await sections.count(); i++) { await sections.nth(i).click(); await p.waitForTimeout(400); await shot(`editor-section${i}`); await sections.nth(i).click(); }
  // equipment: save-file mode, first entry
  await nav(4); await p.waitForTimeout(1200);
  const modes = p.locator('.character-page .segmented button, .character-page [role=tablist] button');
  if (await modes.count() > 1) await modes.nth(1).click();
  await p.waitForTimeout(2500); await shot('equipment-save');
  const row = p.locator('.character-page tbody tr').first();
  if (await row.count()) { await row.click(); await shot('equipment-row'); }
  const tabs = p.locator('.character-page button', {hasText: /Items|道具|アイテム/});
  if (await tabs.count()) { await tabs.first().click(); await shot('items'); }
  // add-equipment pane
  await p.locator('.character-page button', {hasText: /Add new equipment|新しい装備を追加|添加新装备/}).first().click().catch(() => {});
  await shot('equipment-add');
}
await cdp.send('Emulation.clearDeviceMetricsOverride');
await p.evaluate(o => o ? localStorage.setItem('nioh3-ui-locale', o) : localStorage.removeItem('nioh3-ui-locale'), orig);
await p.reload();
for (const [k, v] of Object.entries(report)) { console.log('##', k); for (const r of v) console.log('  ', r.k, JSON.stringify(r.txt ?? ''), r.at ?? '', r.box ? '| in ' + r.box : '', r.d ?? ''); }
process.exit(0);
