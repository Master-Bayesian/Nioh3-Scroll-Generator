import {chromium} from 'playwright';
import assert from 'node:assert/strict';
import {writeFile} from 'node:fs/promises';
const browser=await chromium.launch();const page=await browser.newPage({viewport:{width:1600,height:1000}});const checks=[];
const check=(name,ok)=>{assert.ok(ok,name);checks.push(name)};
async function move(source,target,fraction=.5){const a=await source.boundingBox(),b=await target.boundingBox();await page.mouse.move(a.x+5,a.y+5);await page.mouse.down();await page.mouse.move(b.x+b.width*fraction,b.y+b.height*.5,{steps:6});await page.waitForTimeout(150)}
try{
 await page.goto('http://127.0.0.1:4178');await page.getByRole('button',{name:'清空全部',exact:true}).click();await page.getByRole('button',{name:'敌人',exact:true}).click();
 await page.locator('.enemy-list button').nth(0).click();await page.locator('.enemy-list button').nth(1).click();await page.locator('.selection>header h2').hover();
 const items=page.locator('.enemy-chip');const before=await items.evaluateAll(nodes=>nodes.map(e=>e.dataset.conditionId));
 await move(items.first().locator('.drag-grip'),items.last(),.95);await page.mouse.up();
 check('Enemy edge drops never reorder conditions',JSON.stringify(before)===JSON.stringify(await items.evaluateAll(nodes=>nodes.map(e=>e.dataset.conditionId))));
 await move(items.first().locator('.drag-grip'),items.last());
 check('Center hover shows a group bubble',await page.locator('[data-group-target=true]').evaluate(e=>getComputedStyle(e,'::after').content.includes('松开加入分组')));
 check('Dragged chip remains solid',await page.locator('.drag-preview').evaluate(e=>getComputedStyle(e).opacity==='1'));
 await page.screenshot({path:'deliverables/frontend-v2/search-ui-demo-v2/group-hover-preview.png',fullPage:true});await page.mouse.up();
 check('Drop creates a persistent group bubble',await page.locator('.condition-group.enemy-group .enemy-chip').count()===2);
 const grip=await items.first().locator('.drag-grip').boundingBox(),body=await page.locator('.selected-body').boundingBox();
 await page.mouse.move(grip.x+5,grip.y+5);await page.mouse.down();await page.mouse.move(body.x+body.width-20,body.y+body.height-25,{steps:8});await page.mouse.up();
 check('Dropping outside dissolves a two-item group',await page.locator('.condition-group.enemy-group').count()===0);
 check('Ungrouped enemy conditions are mandatory again',(await items.locator('select').evaluateAll(nodes=>nodes.map(e=>e.value))).every(v=>v==='0'));
 await page.getByRole('button',{name:'收起已选条件'}).click();
 for(const title of ['主副词条','恩宠','敌人','特殊规则']){
  const panelButton=page.getByRole('button',{name:title,exact:true});if(await panelButton.getAttribute('aria-expanded')!=='true')await panelButton.click();
  const module=panelButton.locator('xpath=ancestor::section[1]');const list=module.locator('.catalog-list,.rule-tree').first();
  await list.evaluate(e=>{e.scrollTop=200;e.dispatchEvent(new Event('scroll',{bubbles:true}))});const top=page.getByRole('button',{name:title+'返回顶部',exact:true});await top.waitFor();
  check(title+' back-to-top matches its section background',await top.evaluate(e=>getComputedStyle(e).backgroundColor===getComputedStyle(e.closest('.module')).backgroundColor));
  await top.click();check(title+' back-to-top scrolls to the beginning',await list.evaluate(e=>e.scrollTop)===0);
 }
 await writeFile('deliverables/frontend-v2/search-ui-demo-v2/group-verification.json',JSON.stringify({checks},null,2));console.log(`${checks.length} group and section-color checks passed`);
}finally{await browser.close()}
