# 仁王3绘卷生成器 Beta v0.6.3

本版本完成不受原生生成规则限制的本地绘卷词条修改器。

- 选择一张绘卷后同时显示七个物理词条槽，可连续修改多个槽并一次性保存。
- 每个槽可直接修改词条 ID 与 raw 数值；当前槽还能修改 prefix、metadata、tail 0 和 tail 1。
- 完整原生词条目录会同步所选词条的 ID、group prefix 与类别字段，减少只改 ID 导致显示异常的情况；同步后所有 raw 字段仍可手动覆盖。
- 明确允许重复词条、冲突词条、主副槽混放、未知 ID、任意 uint32 数值，以及与当前 Seed、稀有度或原生生成规则不一致的组合。
- 切换绘卷时会保护未保存草稿；支持逐槽清空和放弃整张绘卷的未保存修改。
- 所有变化在一个写档事务中提交，继续保留自动备份、源记录一致性、校验和、加密回读与源文件哈希门禁。
- 本地自由修改不会改变传播用的 canonical Seed/稀有度，接收方重新生成后通常不会保留这些改动。

---

# Nioh 3 Scroll Generator Beta v0.6.3

This release completes the unrestricted local effect editor.

- Selecting a scroll exposes all seven physical effect slots, allowing several slots to be edited and saved in one transaction.
- Every slot accepts a direct effect ID and raw value; the active slot also exposes prefix, metadata, tail 0, and tail 1.
- The complete native catalog can synchronize the selected effect ID, group prefix, and category field while keeping every raw field manually overridable.
- Duplicate or conflicting effects, primary/secondary role mixing, unknown IDs, arbitrary uint32 values, and combinations unrelated to the current Seed, rarity, or native generation rules are explicitly permitted.
- Unsaved drafts are protected when switching scrolls, with per-slot clearing and whole-draft discard actions.
- Batch saves retain automatic backups, exact-original checks, checksum repair, encrypted readback, and source-file hash gates.
- Local edits do not change the canonical propagation tuple and normally disappear when another player regenerates the scroll.
