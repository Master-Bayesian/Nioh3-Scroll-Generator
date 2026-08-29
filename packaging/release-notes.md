# 仁王3绘卷生成器 Beta v0.5.5

本版本重做了组合合法性说明、R4 求解顺序、数值条件和候选比较体验。已安装 v0.5.4 的用户可以通过应用内签名更新直接升级。

- 搜索前检查普通副词条槽位数量、原生冲突组、类别容量、未知词条和零权重上下文；无解组合立即说明原因。
- 普通词条目录明确标注为“逐项可生成”，不再暗示任意组合都合法。找到 Seed 后才显示“完整离线重放验证”。
- 支持 1–3 个主词条候选（任一命中），也支持完全不限制主词条、只要求副词条。
- 每个普通词条可选择任意数值、抽取百分位 ≥80、≥90 或最高 100；数值条件进入精确 Seed 求解和交集统计。
- R4 联立求解把 CUDA/CPU 主词条批量预筛和便宜的辅助条件放在 finalizer 之前；受控三条件基准约从 15 秒降到 0.88 秒。
- “不限制恩宠”明确允许 R4 finalizer 保留恩宠或将其替换成普通词条；指定恩宠仍要求完成后真实保留。
- 流式结果不再抢走正在查看的候选；候选摘要增加副词条、全部规则、敌人和数值信息，并支持排序及多选对比。
- 词条列表增加键盘可操作的上下移动按钮，窗口尺寸不再强制大于较小显示器的可用区域。
- 自动检测已安装游戏文件版本；明确发现未验证的新版本时拒绝使用旧离线生成数据，并提示先更新应用。
- 已知 Seed 单点生成不再被上方筛选条件或无解组合误拦截。
- 教程和中英文 README 已同步更新。

---

This release adds structural feasibility preflight, primary OR-candidates, per-effect roll thresholds, optimized R4 filtering, stable streaming selection, sortable side-by-side candidate comparison, installed-game version gating, and updated bilingual documentation. Existing v0.5.4 installations can upgrade through the signed in-app updater.
