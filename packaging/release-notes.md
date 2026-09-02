# 仁王3绘卷生成器 v0.6.7

本正式版适配《仁王3》PC v2.01，同时保留 PC v2.00.02 支持，并收录此前测试版中的搜索加速与敌人形态修正。

- 新增 PC v2.01 精确版本配置。程序会根据游戏 EXE 版本选择独立的原生函数地址与特征签名；未知后续版本仍会拒绝运行时调用，不会误用旧地址。
- 对 PC v2.01 完成原生回归验证：稀有度 3、4 各 10,000 个 Seed 完整一致；稀有度 5 的 10,000 个 Seed 词条逐槽一致；敌人、地形、特殊规则与参数表和旧版一致；当前加密存档仍使用原有 400 栏位结构。
- PC v2.01 新增“功能标志 9”稀有度上限。标志不可用时，游戏原生组装器会把稀有度 5 记录头限制为 4，但词条算法没有变化。工具继续保留用户明确选择的原始稀有度 5，并在写入前进行完整词条验证。
- 补齐稀有度 4 的 DirectCompute 最终化筛选，以及“主词条不限”和部分词条条件的 GPU 快速路径。大范围搜索不再静默回退到可能运行几十分钟的 CPU/Python 路径；缺少必要 GPU 后端时会直接说明原因。
- 将同名但不同原生 ID 的高手敌人，以及金井半兵卫的人形／妖怪形态拆成独立选项，搜索和临时修改均只匹配所选形态。
- 地形搜索只提供游戏实际存在的完整结果组合；临时修改器明确区分原始结果与临时替换结果。
- 新增可重复使用的游戏更新迁移管线：截取只读代码段、重定位签名、对比运行时参数表、执行原生生成回归，再由版本门禁决定是否允许产品启用。

写档前请让游戏返回标题界面；不需要断开网络。敌人、地形和特殊规则覆盖仍是临时内存功能，不会写入存档；停止覆盖、关闭软件、重启游戏或游戏重新生成描述符后都会恢复。

---

# Nioh 3 Scroll Generator v0.6.7

This stable release adds Nioh 3 PC v2.01 compatibility while retaining PC v2.00.02 support, and promotes the search acceleration and enemy-variant fixes from the previous test build.

- Adds an exact PC v2.01 runtime profile. The app selects independent native addresses and signatures from the detected executable version. Unknown future versions remain fail-closed.
- Completes native PC v2.01 regression gates: 10,000 full-record Seed checks each for rarities 3 and 4; 10,000 slot-exact rarity-5 checks; unchanged enemy, terrain, special-rule, and parameter resources; and the unchanged encrypted-save 400-slot layout.
- Models the new PC v2.01 feature-flag-9 rarity cap. When that flag is unavailable, the native assembler caps a rarity-5 record header to 4 while leaving its generated effects unchanged. The tool preserves an explicitly requested raw rarity 5 and still performs complete effect validation before installation.
- Completes DirectCompute finalizer-aware rarity-4 filtering and GPU fast paths for unrestricted-primary and partial-effect requests. Broad searches no longer silently fall back to impractically slow CPU/Python scans; unavailable required GPU backends now produce an actionable error.
- Splits same-name bosses with different native IDs, plus human/yokai Kanai Hanbei, into exact independent choices for search and temporary editing.
- Limits terrain search to complete results that the game can actually generate and clarifies original versus temporary terrain results in the runtime editor.
- Adds a repeatable game-update migration pipeline for read-only section capture, signature relocation, runtime-resource comparison, native parity, and fail-closed product approval.

Return the game to the title screen before writing a save; disconnecting from the network is not required. Enemy, terrain, and special-rule overrides remain temporary runtime behavior and revert when the override stops, the app or game closes, or the game regenerates the descriptor.
