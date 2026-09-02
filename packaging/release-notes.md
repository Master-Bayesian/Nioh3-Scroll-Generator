# 仁王3绘卷生成器 v0.6.9

本版修复稀有度 4 绘卷揭露后词条变化、RTX 50 系 CUDA 不可用，以及复杂条件令右侧结果不可见的问题，并补齐更灵活的主副词条与地形筛选。

- 修正稀有度 4 新增绘卷的两阶段生命周期。写档时保存游戏原生待揭露记录，候选区仍显示一次正常揭露后的最终结果，避免已经完成的记录被游戏再次补全并改掉另一槽词条或恩宠。种子 `125804734` 已加入精确回归测试。
- 主词条候选新增“未当选主词条时，必须出现在副词条”选项。对 A、B 同时勾选即可搜索“A 为主且 B 为副，或 B 为主且 A 为副”。
- 地形改为多选并按任一命中筛选；“含有地狱”覆盖所有可见结果中带地狱的原生组合，精确地形结果仍不会被错误地自由拼接。
- CUDA DLL 新增 RTX 50 系 `sm_120` 原生映像与 `compute_120` PTX，并用真实内核启动检查代替只检测设备数量。CUDA 不可执行时，支持的普通词条路径会转到 DirectCompute；错误信息会保留具体失败阶段和代码。
- “Seed 计算与结果验证”右侧整列新增独立纵向滚动条，长交集统计不再把候选结果、详情和写档按钮挤出屏幕；候选 Seed 列表也新增纵向滚动条。
- 保留 PC v2.01 与 PC v2.00.02、稀有度 3/4/5、跨厂商 DirectCompute、CUDA 辅助条件加速，以及完整离线精确复核。

写档前请让游戏返回标题界面。稀有度 4 修复已通过离线生命周期回归，仍以实际游戏首次揭露为最终验收。敌人、地形和特殊规则的本地覆盖仍是临时内存功能，不会写入存档或传播。

---

# Nioh 3 Scroll Generator v0.6.9

This release repairs rarity-4 reveal changes, RTX 50-series CUDA compatibility, and inaccessible results under long constraint reports, while adding more expressive primary/secondary and terrain filters.

- Writes the native rarity-4 stage-one acquisition record while previewing the result after exactly one reveal finalization. This prevents the game from completing an already-finalized record a second time and changing another effect or Grace slot. Seed `125804734` is covered by an exact regression test.
- Adds a per-primary option requiring that candidate as a secondary when it is not selected as the primary. Selecting it for both A and B expresses `(A primary + B secondary) OR (B primary + A secondary)`.
- Makes complete terrain results multi-select with OR semantics. The aggregate Hell option covers every native row whose visible result contains Hell; exact results are not treated as freely composable effects.
- Adds native `sm_120` and `compute_120` PTX images for RTX 50-series GPUs and replaces device-count-only detection with a real kernel launch/synchronization health check. Supported ordinary-effect searches route to DirectCompute when CUDA cannot execute, while diagnostics retain the CUDA failure stage and code.
- Adds an independent vertical scrollbar to the complete Seed calculation/results pane so long intersection reports cannot hide candidates, details, or install controls. The candidate Seed list now has its own vertical scrollbar as well.
- Retains PC v2.01 and PC v2.00.02 support, rarities 3/4/5, cross-vendor DirectCompute, CUDA auxiliary filtering, and exact offline replay.

Return the game to the title screen before writing a save. The rarity-4 repair has exact offline lifecycle regression coverage; the first real in-game reveal remains the final acceptance check. Local enemy, terrain, and special-rule overrides remain temporary runtime behavior and do not persist or propagate.
