# 仁王3绘卷生成器 v0.6.10

本版修复稀有度 4 搜索仍可能把待揭露中间态当作候选的问题，并开放一、二周目稀有度 4 自定义搜索与添加。

- 所有游戏原生稀有度 4 搜索路径现在都会完整执行揭露最终化，再用最终记录检查主词条、副词条与恩宠；写档时仍保留对应的原生待揭露记录，避免重复最终化。
- 修复 Seed `43723117` 的中间词条误命中：槽位 3 在最终化后由 `0xD411` 变为 `0xF9BE`，搜索只按 `0xF9BE` 判断。
- 修复 Seed `36526331` 被误判为“尚未完成最终解析”：游戏尝试全部合资格槽位后若没有接受替换，原记录本身就是有效最终结果。
- 应用候选入口新增稀有度 4 最终态门禁；即使以后某条搜索分支再次泄漏待揭露记录，也不会显示或允许写入。
- 一、二周目现在可以搜索并添加稀有度 4 绘卷及指定恩宠。界面会明确提示这些周目没有合法原生 R4 掉落；程序按所选存档建立独立映射并调用游戏原生最终化，不套用三周目结果。

写档前请让游戏返回标题界面。一、二周目 R4 属于可构造的自定义配置，不代表游戏会原生掉落。敌人、地形和特殊规则的本地覆盖仍是临时内存功能，不会写入存档或传播。

---

# Nioh 3 Scroll Generator v0.6.10

This release prevents rarity-4 searches from exposing pre-reveal intermediate records and enables custom rarity-4 search/install flows for playthroughs one and two.

- Every native rarity-4 search path now runs the complete reveal finalizer before checking primary effects, secondary effects, or Grace. Installation still keeps the corresponding native stage-one payload so the game finalizes it exactly once.
- Fixes Seed `43723117`: slot 3 changes from stage-only `0xD411` to final `0xF9BE`, and only the final effect can satisfy search filters.
- Fixes Seed `36526331`: after every eligible native attempt declines a replacement, the unchanged source record is the valid final result rather than an unresolved candidate.
- Adds an application-boundary gate that rejects any rarity-4 stage-one record before it can enter the candidate list.
- Enables rarity-4 Grace search and installation for playthroughs one and two. The UI explicitly marks these as custom configurations with no legal native R4 drop; mappings are save/context scoped and finalized natively instead of borrowing playthrough-three results.

Return the game to the title screen before writing a save. Playthrough-one/two rarity-4 records are constructible custom configurations, not native drops. Local enemy, terrain, and special-rule overrides remain temporary runtime behavior and do not persist or propagate.
