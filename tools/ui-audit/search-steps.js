await p.locator('.search-page button',{hasText:/^.?(敌人|Enemy|敵)/}).first().click(); await p.waitForTimeout(300);
const en=p.locator('.enemy-list .catalog-row button'); for(let i=0;i<3;i++){await en.nth(i).click().catch(()=>{});}
await p.locator('.search-page button',{hasText:/特殊规则|Special rules|特殊ルール/}).first().click(); await p.waitForTimeout(300);
for (let i=0;i<4;i++){ const c=p.locator('.rule-category').nth(i); await c.click(); await p.waitForTimeout(150); await p.locator('.rule-picker button').first().click().catch(()=>{}); await c.click(); }
const cat=p.locator('.rule-category',{hasText:/属性伤害与异常|Elemental Damage|属性ダメージ/}); if(await cat.count()){ await cat.first().click(); await p.waitForTimeout(150); await p.locator('.rule-picker button').first().click().catch(()=>{}); }
await p.waitForTimeout(300); await p.locator('.selected-body').first().evaluate(e=>e.scrollTop=e.scrollHeight);
