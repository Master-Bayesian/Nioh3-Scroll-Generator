// Live UI audit tools for a running preview EXE; see docs/product/ARC_UI_DEBUG_HANDOFF_20261012.md.
// node tools/ui-audit/live-eval.mjs <WxH|-> <js-expression>
import {chromium} from 'playwright';
const [size, expr] = process.argv.slice(2);
const b = await chromium.connectOverCDP('http://127.0.0.1:9333');
const p = b.contexts()[0].pages()[0];
const cdp = await p.context().newCDPSession(p);
if (size && size !== '-') { const [w,h]=size.split('x').map(Number); await cdp.send('Emulation.setDeviceMetricsOverride',{width:w,height:h,deviceScaleFactor:1,mobile:false}); await p.waitForTimeout(500);} 
console.log(JSON.stringify(await p.evaluate(expr), null, 1));
process.exit(0);
