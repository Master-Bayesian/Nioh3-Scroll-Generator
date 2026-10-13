// Live UI audit tool for a running EXE started with WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333.
import {chromium} from 'playwright';import {detectTextFit as detect} from '../../apps/workshop/text-fit-detect.mjs';
const [size='1100x740'] = process.argv.slice(2); const [w,h]=size.split('x').map(Number);
const b = await chromium.connectOverCDP('http://127.0.0.1:9333'); const p = b.contexts()[0].pages()[0];
const cdp = await p.context().newCDPSession(p);
await cdp.send('Emulation.setDeviceMetricsOverride',{width:w,height:h,deviceScaleFactor:1,mobile:false});
await p.evaluate(()=>localStorage.setItem('nioh3-ui-locale','zh-CN')); await p.reload(); await p.waitForTimeout(2500);
await p.locator('nav.nav > button').nth(4).click(); await p.waitForTimeout(1200);
await p.locator('.character-page button',{hasText:'修改存档文件'}).first().click(); await p.waitForTimeout(4000);
await p.locator('.character-page tbody tr').first().click(); await p.waitForTimeout(800);
const fav = p.locator('.character-page label',{hasText:'喜好项目'}).locator('input'); await fav.first().click(); await p.waitForTimeout(300);
const btns = await p.locator('.character-page button').allTextContents(); console.log(btns.filter(t=>/计划|核对|应用/.test(t)).join(' | '));
await p.locator('.character-page button',{hasText:/^写入$/}).first().click(); await p.waitForTimeout(4000);
await p.screenshot({path:`${process.env.UI_AUDIT_OUT || '.'}/plan-${size}.png`});
console.log(JSON.stringify(await p.evaluate(()=>{const e=document.querySelector('.character-save-plan');if(!e)return 'no plan';const r=e.getBoundingClientRect();return {h:Math.round(r.height),top:Math.round(r.top),confirm:!!e.querySelector('[data-action=confirm-character-plan]')}})));
console.log(JSON.stringify(await p.evaluate(detect)));
console.log(JSON.stringify(await p.evaluate(()=>{const out=[];const L=document.querySelector('.character-list');out.push([...L.children].map(c=>c.className.split(' ')[0]+':'+Math.round(c.getBoundingClientRect().top)+'/'+Math.round(c.getBoundingClientRect().height)+' flex='+getComputedStyle(c).flex+' minh='+getComputedStyle(c).minHeight).join(' | '));let e=L;while(e&&e!==document.body){const cs=getComputedStyle(e);out.push(e.tagName+'.'+(e.className||'').toString().split(' ')[0]+' ov='+cs.overflow+' st='+e.scrollTop+' h='+Math.round(e.getBoundingClientRect().height)+' sh='+e.scrollHeight);e=e.parentElement}return out})));
await cdp.send('Emulation.clearDeviceMetricsOverride'); process.exit(0);
