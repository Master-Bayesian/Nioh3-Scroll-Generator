# 仁王3绘卷生成器 v0.6.8

本版集中修复纯特殊规则／敌人／地形搜索性能、首次搜索提前停止、Intel 等非 NVIDIA 设备的回退交互，以及 1080p 下本地绘卷编辑器无法访问底部控件的问题。

- 将自然 Seed 构造、地形、敌人和特殊规则筛选合并为一次原生 CUDA 管线，避免每层条件都在 Python 与显卡之间往返。稀有组合会持续扫描完整数学空间，不再按单个条件特判。
- 普通词条 DirectCompute 着色器改为构建时预编译并嵌入 DLL，启动后的首次计算不再现场编译两个大型着色器。
- 修正效果条件未通过时仍继续计算敌人、地形和特殊规则的问题；现在先丢弃不符合词条的 Seed，再运行后续辅助筛选。
- 修正启动软件后的第一次搜索可能只返回 1 个候选便提前结束的问题。界面任务会自动继续底层数学批次和内部预像页，直到收集到用户要求的候选数、整个数学族耗尽或用户取消；默认 GPU 批次也由 100 万游标提高到 1 亿游标。
- 未检测到 CUDA 时不再直接拒绝 Intel、AMD 或无 CUDA 用户。程序会明确说明哪些条件需要改用原生 CPU，并在用户确认后继续；不会静默回退。
- 本地绘卷编辑器右侧增加独立纵向滚动条与鼠标滚轮支持，1080p 窗口可以访问全部字段、敌人、地形和特殊规则控件。
- 保留 PC v2.01 与 PC v2.00.02、稀有度 3/4/5、DirectCompute 跨厂商普通词条加速和完整离线精确复核。

写档前请让游戏返回标题界面。敌人、地形和特殊规则覆盖仍是临时内存功能，不会写入存档；停止覆盖、关闭软件、重启游戏或游戏重新生成描述符后都会恢复。

---

# Nioh 3 Scroll Generator v0.6.8

This release focuses on pure special-rule/enemy/terrain search performance, first-search pagination, explicit fallback behavior for Intel and other non-NVIDIA systems, and editor accessibility at 1080p.

- Fuses natural Seed construction and terrain, enemy, and special-rule filtering into one native CUDA pipeline instead of repeatedly transferring intermediate batches through Python.
- Precompiles and embeds the ordinary-effect DirectCompute shaders at build time so the first search no longer compiles two large shaders at runtime.
- Rejects effect-stage failures before running auxiliary filters.
- Fixes the first search after application startup sometimes stopping after one candidate. A UI search now continues through bounded mathematical batches and internal preimage pages until it fills the requested count, exhausts the mathematical family, or is cancelled. The default GPU batch increases from one million to 100 million cursors.
- Replaces the hard CUDA rejection with an explicit confirmation dialog. DirectCompute still accelerates supported ordinary-effect stages on AMD, NVIDIA, and Intel; unsupported auxiliary stages use exact native CPU code only after user consent.
- Adds an independent vertical scrollbar and mouse-wheel support to the local scroll editor so every field and runtime auxiliary control remains reachable at 1080p.
- Retains PC v2.01 and PC v2.00.02 support, rarities 3/4/5, cross-vendor DirectCompute ordinary-effect acceleration, and complete exact offline verification.

Return the game to the title screen before writing a save. Enemy, terrain, and special-rule overrides remain temporary runtime behavior and revert when the override stops, the app or game closes, or the game regenerates the descriptor.
