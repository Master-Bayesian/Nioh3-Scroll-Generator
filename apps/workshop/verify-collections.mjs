import {_electron as electron} from 'playwright';
import {resolve} from 'node:path';
import {writeFile,mkdir} from 'node:fs/promises';
import assert from 'node:assert/strict';
const output=resolve('deliverables/frontend-v2/collections-acceptance');await mkdir(output,{recursive:true});
const app=await electron.launch({...(process.env.NIOH3_PORTABLE_EXE?{executablePath:resolve(process.env.NIOH3_PORTABLE_EXE),args:[]}:{args:[resolve('apps/desktop/dist/main.cjs')]}),env:{...process.env,NIOH3_REVIEW_UI:'1',NIOH3_ELECTRON_TEST:'1'}});
const checks=[],errors=[];
try{
 const p=await app.firstWindow();p.on('pageerror',e=>errors.push(e.message));
 await p.getByText('后端已连接，请选择筛选条件。',{exact:true}).waitFor({timeout:30000});
 await p.getByRole('textbox',{name:'已知绘卷ID',exact:true}).fill('10030565');
 await p.getByRole('button',{name:'查看',exact:true}).click();await p.locator('.result-detail .scroll').waitFor();
 await p.getByRole('button',{name:'收藏绘卷',exact:true}).click();await p.getByRole('button',{name:'取消收藏',exact:true}).waitFor();
 await p.getByRole('button',{name:'收藏夹',exact:true}).click();await p.locator('.favorites-review .scroll').waitFor();
 await p.locator('.favorites-review').getByRole('button',{name:'复制 ID 和稀有度',exact:true}).click();
 assert.equal(await app.evaluate(({clipboard})=>clipboard.readText()),'10030565 · R4');
 await p.locator('.favorites-review').getByRole('button',{name:'加入购物车',exact:true}).click();
 await p.locator('.favorites-review').getByRole('button',{name:'加入购物车',exact:true}).isDisabled();
 await p.keyboard.press('Escape');await p.getByRole('button',{name:'查看购物车（1）'}).click();
 await p.locator('.cart-review').getByRole('button',{name:'取消收藏',exact:true}).click();await p.locator('.cart-review').getByRole('button',{name:'收藏绘卷',exact:true}).waitFor();
 await p.locator('.cart-review').getByRole('button',{name:'收藏绘卷',exact:true}).click();await p.locator('.cart-review').getByRole('button',{name:'取消收藏',exact:true}).waitFor();
 checks.push('Search → favorites → cart; cart → favorites; copy ID and rarity');
 await p.reload();await p.getByText('后端已连接，请选择筛选条件。',{exact:true}).waitFor({timeout:30000});
 await p.getByRole('button',{name:'收藏夹',exact:true}).click();await p.locator('.favorites-review .scroll').waitFor();
 await p.locator('.favorites-review').getByRole('button',{name:'加入购物车',exact:true}).click();await p.keyboard.press('Escape');
 assert.equal((await p.evaluate(()=>window.review.favorites({action:'list'}))).length,1);
 checks.push('Favorites reload exact broker-owned candidate after renderer references are cleared');
 const snapshots=[];
 for(const [locale,label] of [['en-US','English'],['ja-JP','日本語'],['zh-CN','简体中文']]){
  await p.locator('.language-button').click();await p.locator('.side-popup').getByRole('button',{name:label,exact:true}).click();
  for(const [width,height] of [[1280,800],[1920,1080],[2560,1440]]){
   await app.evaluate(({BrowserWindow},bounds)=>BrowserWindow.getAllWindows()[0].setBounds(bounds),{width,height});
   await p.waitForTimeout(150);
   const state=await p.evaluate(()=>({lang:document.documentElement.lang,width:innerWidth,height:innerHeight,overflow:document.documentElement.scrollWidth>innerWidth,text:document.body.innerText}));
   assert.equal(state.lang,locale);assert.equal(state.overflow,false,`${locale} ${width}: horizontal overflow`);
   if(locale==='en-US'){assert.ok(state.text.includes('Scroll search'));assert.ok(!/[\u3400-\u9fff]/.test(state.text),'English screen contains untranslated Chinese');}
   snapshots.push(state);
   if(width===1920&&process.env.NIOH3_CAPTURE_UI==='1'){
    await app.evaluate(({BrowserWindow})=>BrowserWindow.getAllWindows()[0].showInactive());
    await p.screenshot({path:resolve(output,`${locale}-1920.png`),timeout:10000});
   }
  }
 }
 checks.push('Three UI languages; 1280×800, 1920×1080 and 2560×1440 have no page-width overflow');
 assert.deepEqual(errors,[]);
 await writeFile(resolve(output,'verification.json'),JSON.stringify({checks,errors,snapshots,scope:'Local generated candidate; no save write or game call'},null,2));
 console.log(checks.join('\n'));
}finally{await app.close()}
