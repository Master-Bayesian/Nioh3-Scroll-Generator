# 仁王3绘卷生成器 v0.6.6

本正式版修复新增绘卷偶尔被游戏显示为装备的问题，恢复大范围搜索的显卡路径，并加入原生组合地形筛选和临时覆盖。

- 修复“添加绘卷后显示为装备”：新增绘卷不再只依据绘卷栏位分配 `+0x28` 生成序号，而是避开整个存档中已被原生装备记录使用的序号；写入时也会修复已经与装备冲突的绘卷序号。该根因和修复状态转换已经过真实问题存档的游戏内验证。
- 不再改写未经证实的 `+0x1C` 字段来掩盖问题。现有绘卷的该字段保持原样，新绘卷继续采用保守分配。
- “主词条不限”的完整稀有度4词条组合现在会枚举可能的主词条位置，并进入 Pro 完整组合逆向路径；所有主词条布局都无解时会在搜索前立即说明不存在。
- 通用固定抽取 Seed 候选构造现在优先使用 Direct3D 11 Compute，支持 AMD、NVIDIA 和 Intel 显卡；NVIDIA 专用 CUDA 路径仍保留。只有对应 GPU 后端不可用时才回退到原生 CPU，最终结果仍由完整 CPU 生成器精确重放验证。
- 生成器的地形条件改为“必须包含”多选。原生 `0x08` 会显示并筛选为“地狱＋瘴血”，原生 `0x2D` 为“地狱＋火”；没有共同原生地形行的组合会立即报无解。
- 临时运行时修改器增加“地狱”“地狱＋火”“地狱＋瘴血”快捷项，并明确说明一个原生地形参数可以同时产生多个可见效果。候选预览会显示全部地形效果，不再只显示第一项。
- 敌人、地形和特殊规则覆盖仍是 PC v2.00.02 的临时内存功能，不会写入存档；停止覆盖、关闭软件、重启游戏或游戏重新生成描述符后都会恢复。

本版本仍仅认证《仁王3》PC v2.00.02。写档前请让游戏返回标题界面；不需要断开网络。

---

# Nioh 3 Scroll Generator v0.6.6

This stable release fixes newly installed scrolls occasionally appearing as equipment, restores GPU routing for broad searches, and adds native combined-terrain filtering and temporary overrides.

- Fixes “installed scroll appears as equipment.” New records now allocate their `+0x28` generation serial against native equipment records across the save, and installation repairs existing scroll serials that collide with equipment. The root cause and repair transition were validated in game against an affected save.
- Stops rewriting the unproven `+0x1C` field as a workaround. Existing values are preserved, while new scrolls retain conservative allocation.
- Complete rarity-4 requests with an unrestricted primary now enumerate every possible primary assignment and use the Pro complete-composition inverse. When every primary layout is exhausted, the request is rejected before scanning.
- Generic fixed-draw Seed construction now prefers Direct3D 11 Compute on AMD, NVIDIA, and Intel adapters. The NVIDIA-specific CUDA path remains available. Native CPU is used only when a suitable GPU backend is unavailable, and every result still passes exact CPU replay.
- Terrain requirements are now multi-select. Native terrain `0x08` is exposed as The Crucible + Foulblooded and `0x2D` as The Crucible + Fire; selections with no shared native terrain row fail immediately.
- The temporary runtime editor adds direct presets for The Crucible, The Crucible + Fire, and The Crucible + Foulblooded. Candidate previews show every visible terrain effect rather than only the first.
- Enemy, terrain, and special-rule overrides remain temporary PC v2.00.02 runtime behavior. They are not written to the save and revert when the override stops, the app or game closes, or the native descriptor is regenerated.

This release remains certified only for Nioh 3 PC v2.00.02. Return the game to the title screen before writing a save; disconnecting from the network is not required.
