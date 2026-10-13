// Ad-hoc visual audit of the running app over CDP .
// node tools/ui-audit/live-pages.mjs <outdir> [locales] [themes] [sizes]
import {chromium} from 'playwright';
import fs from 'node:fs';
import {detectTextFit as detect} from '../../apps/workshop/text-fit-detect.mjs';
const [out='ui-audit-out', locs='zh-CN,en-US,ja-JP', themes='light,dark', sizes='1100x740,1600x1000'] = process.argv.slice(2);
fs.mkdirSync(out, {recursive: true});
const web = process.env.WEB;
const b = web ? await chromium.launch() : await chromium.connectOverCDP('http://127.0.0.1:9333');
const p = web ? await b.newPage() : b.contexts()[0].pages()[0];
if (web) await p.goto(web);
const cdp = await p.context().newCDPSession(p);
const orig = await p.evaluate(() => ({loc: localStorage.getItem('nioh3-ui-locale'), theme: localStorage.getItem('nioh3-theme')}));

const report = {};
for (const loc of locs.split(',')) {
  for (const theme of themes.split(',')) {
    await p.evaluate(([l, t]) => { localStorage.setItem('nioh3-ui-locale', l); if (t === 'light') localStorage.removeItem('nioh3-theme'); else localStorage.setItem('nioh3-theme', t); }, [loc, theme]);
    for (const size of sizes.split(',')) {
      const [w, h] = size.split('x').map(Number);
      if (web) await p.setViewportSize({width: w, height: h}); else await cdp.send('Emulation.setDeviceMetricsOverride', {width: w, height: h, deviceScaleFactor: 1, mobile: false});
      await p.reload(); await p.waitForTimeout(2500);
      const tabs = await p.locator('nav.nav > button').count();
      await p.evaluate(() => 0);
      for (let i = 0; i < tabs; i++) {
        const btn = p.locator('nav.nav > button').nth(i);
        const name = (await btn.innerText()).trim().replace(/\s+/g, '_') || `tab${i}`;
        if (await btn.isDisabled()) continue;
        await btn.click(); await p.waitForTimeout(900);
        const tag = `${loc}_${theme}_${size}_${i}-${name}`;
        await p.screenshot({path: `${out}/${tag}.png`});
        const found = await p.evaluate(detect);
        if (found.length) report[tag] = found;
      }
    }
  }
}
await cdp.send('Emulation.clearDeviceMetricsOverride');
await p.evaluate(o => { o.loc ? localStorage.setItem('nioh3-ui-locale', o.loc) : localStorage.removeItem('nioh3-ui-locale'); o.theme ? localStorage.setItem('nioh3-theme', o.theme) : localStorage.removeItem('nioh3-theme'); }, orig);
await p.reload();
fs.writeFileSync(`${out}/report.json`, JSON.stringify(report, null, 1));
for (const [k, v] of Object.entries(report)) { console.log('##', k); for (const r of v) console.log('  ', r.k, JSON.stringify(r.txt ?? ''), r.at ?? '', r.box ? '| in ' + r.box : '', r.d ?? ''); }
process.exit(0);
