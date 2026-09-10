import {chromium} from 'playwright';import assert from 'node:assert/strict';import {readFile,writeFile} from 'node:fs/promises';
const data=JSON.parse(await readFile('apps/workshop/catalog.json','utf8'));
const b=await chromium.launch();const p=await b.newPage({viewport:{width:1600,height:1000}});const checks=[];
try{
 await p.goto('http://127.0.0.1:4178');await p.getByRole('button',{name:'清空全部',exact:true}).click();await p.getByRole('button',{name:'敌人',exact:true}).click();
 const names=[...data.enemies].sort((a,b)=>b.name.length-a.name.length).slice(0,10).map(v=>v.name);
 for(const name of names){await p.getByRole('textbox',{name:'搜索全部敌人名称或 ID',exact:true}).fill(name);await p.getByRole('button',{name:'添加敌人'+name,exact:true}).click()}
 await p.getByRole('button',{name:'展开已选条件'}).click();
 for(const name of names){await p.getByRole('combobox',{name:name+'敌人组合',exact:true}).selectOption('1')}
 const group=p.locator('.condition-group.enemy-group');const items=group.locator('.group-items');
 assert.equal(await group.count(),1);checks.push('Ten long conditions remain in one rectangular group');
 const box=await group.boundingBox();assert.ok(box.width<=350);checks.push('Group width is bounded');
 assert.ok(await items.evaluate(e=>e.scrollHeight>e.clientHeight&&e.clientHeight<=178));checks.push('Large groups scroll within a fixed-height body');
 assert.ok(await group.locator('.enemy-chip').evaluateAll(nodes=>nodes.every(e=>{const child=e.querySelector('span:not(.drag-grip)').getBoundingClientRect();const parent=e.getBoundingClientRect();return child.x>=parent.x&&child.right<=parent.right+1})));checks.push('Long names wrap inside the group');
 assert.ok((await group.locator('small').innerText()).includes('10 项'));checks.push('Group header keeps total count visible');
 await items.evaluate(e=>e.scrollTop=0);await p.screenshot({path:'deliverables/frontend-v2/search-ui-demo-v2/large-group-preview.png',fullPage:true});
 await writeFile('deliverables/frontend-v2/search-ui-demo-v2/large-group-verification.json',JSON.stringify({checks},null,2));console.log(`${checks.length} large-group checks passed`);
}finally{await b.close()}
