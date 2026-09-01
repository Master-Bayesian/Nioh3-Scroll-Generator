# 仁王3绘卷生成器 v0.6.5

本正式版补全本地绘卷编辑与敌人选择界面，并保留 v0.6.4 已实装的效果逆向快路径、结构合法性预检及 AMD/NVIDIA/Intel Direct3D 11 后端。

- 敌人候选按“低手／中手／高手”三个生成池档位分栏，每栏有独立搜索框，同时保留横向全敌人搜索下拉框。档位只描述原生生成池，不代表实际战斗强弱。
- 新增“敌人合法组合一览”，明确列出原生十种敌人组结构。选择一名敌人仍表示“成品必须至少包含它”，不会错误承诺成品只有该敌人。
- 142 个合法敌人显示名全部保留；不在原生绘卷候选表中的名称仍不会进入合法搜索。唯一横跨 role 4/5 的金井半兵卫标为“中／高手”，并同时出现在对应两栏。
- 本地编辑器现在可在同一备份事务中修改 Seed、周目、稀有度、绘卷等级、推荐等级、转手次数及七个完整词条槽。词条 ID、数值、prefix、metadata 和 tail 仍允许任意 raw 值及非法组合。
- 本地词条数值旁会显示 uint32 输入范围，并在原生表可解析时显示当前稀有度与等级下的离散 raw 范围；这些提示不会禁止自由输入。
- 加入实验性的临时敌人／地形／特殊规则覆盖。它只支持 PC v2.00.02、按目标 Seed 命中，并允许在同档位原生槽内重复敌人；跨低手／中手／高手档位替换会被游戏丢弃，因此程序会拒绝。覆盖不会写进存档，停止覆盖、关闭软件、重启游戏或游戏重新生成后都会恢复。
- 临时编辑器会直接列出当前 Seed 可复用的低手／中手／高手槽位；未出现的档位不再被误解为候选漏项。地形下拉改用玩家可见名称，补齐“地狱”及“地狱／瘴血（俗称污血）”等选项。
- 临时覆盖会核对精确机器码并限制在原生已分配的敌人组容量内；停止覆盖时恢复原始指令。若游戏版本、签名或运行时结构不匹配，功能会拒绝启用。
- 自动更新改由已签名的新 EXE 以独立更新器模式完成：用户确认一次后，程序会安全退出旧版、重试替换原位置 EXE、自动启动新版并清理下载缓存，不再依赖隐藏 PowerShell 脚本。
- Pro 完整词条组合预像快路径、Direct3D 11 Compute 跨厂商加速、CUDA/原生 CPU 回退、流式候选显示和完整 CPU 精确重放继续保留。

本版本仍仅认证《仁王3》PC v2.00.02。写档前请让游戏返回标题界面；不需要断开网络。临时辅助覆盖属于本机运行时功能，不具备传播性。

---

# Nioh 3 Scroll Generator v0.6.5

This stable release completes the local editor and enemy-selection workflow while retaining the effect-inversion fast path, structural preflight, and cross-vendor Direct3D 11 backend shipped in v0.6.4.

- Enemy candidates are separated into Low, Middle, and High generation-pool tiers. Every column has its own search box, and the horizontal all-enemy search combobox remains available. These tiers describe native generation pools, not combat difficulty.
- A read-only legal-combination guide lists the ten native enemy-group structures and explains that selecting an enemy means “must contain,” not “the finished scroll contains only this enemy.”
- All 142 legal display identities remain available. Localization-only names without a native scroll-candidate row stay excluded. Kanai Hanbei, the sole role-4/role-5 identity, is marked Middle/High and appears in both columns.
- The local editor can now change Seed, playthrough, rarity, scroll level, recommended level, transfer count, and all seven complete effect slots in one backup-gated transaction. Effect ID, value, prefix, metadata, and tail fields remain unrestricted.
- Effect values show the full uint32 input domain and, when the native tables permit it, the discrete raw range for the selected rarity and level. These hints never restrict free editing.
- An experimental temporary enemy/terrain/special-rule override is included for PC v2.00.02. It matches a target Seed and permits repeated enemies within compatible native tiers. Cross-tier replacements are discarded by the game and are therefore rejected. It is not saved: stopping the override, closing the application, restarting the game, or native regeneration restores the Seed-derived data.
- The runtime editor now lists the Low/Middle/High slots actually reusable by the selected Seed. Missing tiers are no longer presented as missing candidates. Terrain choices use player-visible names, including The Crucible and The Crucible/Foulblooded combinations.
- The runtime override verifies exact machine code, stays within native enemy-group capacity, and restores the original instruction when stopped. Unsupported versions, signatures, or runtime layouts fail closed.
- Automatic updates now run the signed new EXE in a dedicated updater mode. One confirmation safely exits the old build, retries replacement in the original location, restarts the new build, and removes the download cache without a hidden PowerShell installer.
- The Pro complete-composition preimage path, AMD/NVIDIA/Intel Direct3D 11 compute, CUDA/native-CPU fallbacks, streamed candidates, and exact CPU replay remain active.

This release remains certified only for Nioh 3 PC v2.00.02. Return the game to the title screen before writing a save; disconnecting from the network is not required. Temporary auxiliary overrides are local runtime behavior and do not propagate.
