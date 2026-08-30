# 仁王3绘卷生成器 Beta v0.6.2

本版本修复应用新增绘卷可能在游戏内消失，或暂时显示成装备名称的问题。

- 新增绘卷不再继承 donor 模板的 `+0x1C` 库存实例键；写入时会在完整 400 栏绘卷数组中分配新的非零唯一键。
- 写档事务会检测已有的重复实例键，保留第一条记录并为后续冲突记录重新分配唯一键，同时在安装报告中记录修复栏位及新旧值。
- 两份独立用户存档证明了同一根因的两种表现：游戏完全隐藏新增绘卷，或把它暂时关联成装备显示；原生丢弃/拾取流程重新分配实例键后，两种现象都会恢复。
- 受控诊断存档只修改两个冲突实例键及 checksum 后，原本在游戏中不可见的两张绘卷立即恢复显示。
- 新增单张安装、批量安装、已有冲突迁移和加密回读回归；完整测试套件共 324 项，全部通过。

---

# Nioh 3 Scroll Generator Beta v0.6.2

This release fixes newly installed scrolls that could disappear from the game inventory or temporarily render as equipment.

- New records now receive a fresh nonzero `+0x1C` inventory-instance key instead of inheriting the donor template's key.
- Save transactions repair existing duplicate keys by preserving the first record and assigning fresh keys to later collisions, with every repair recorded in the install report.
- Two independent support saves reproduced different symptoms of the same collision. A controlled save changing only the two colliding keys and checksum restored both hidden records in game.
- The complete 324-test suite passes, including single install, batch install, collision migration, backup, checksum, and encrypted roundtrip coverage.
