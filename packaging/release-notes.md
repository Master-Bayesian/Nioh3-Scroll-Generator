# 仁王3绘卷生成器 v0.6.4

本正式版修复 v0.6.3 收到的三项主要反馈，并加入可选 Beta 更新通道及新的完整词条组合加速路径。

- FB-014：补全“志那都彦的恩宠”（`0x4192`），现在可在稀有度 4、5 的恩宠候选中正确显示和选择。
- FB-015：词条类别容量或冲突检查现在会直接列出相关词条名称、ID、原生类别、容量以及至少需要移除的数量。
- FB-016：新增绘卷不再只检查绘卷栏位内的实例键，而是分配在完整存档中未使用的物品实例键；写入事务也会修复可严格确认的绘卷/装备实例键冲突，避免合法绘卷被游戏索引或显示成装备。
- 更新通道默认只接收正式版。用户可在标题栏勾选“接收 Beta”，同时比较最新正式版和已签名的 GitHub prerelease；取消后立即恢复正式通道。
- 带 `-beta.N` 或 `-rc.N` 的版本会作为 GitHub prerelease 发布，稳定版用户不会自动收到；两个通道继续强制校验 Ed25519 签名、文件大小和 SHA-256。
- 恢复并保留三周目稀有度 3、4、5 的搜索、已知 Seed 生成和写入入口。
- 加入完整词条组合预像加速路径，在满足完整槽位约束时可通过 Direct3D 11 Compute 在 AMD、NVIDIA 或 Intel 显卡上构造候选，再由 CPU 精确重放完整记录。
- 保留 CUDA/原生 CPU 回退、流式候选显示、结构合法性预检和完整记录 fail-closed 门禁。
- 应用标题和教程不再把正式版整体标成 Beta，并同步修正稀有度与更新通道说明。

本版本仍仅认证《仁王3》PC v2.00.02。写档前请让游戏返回标题界面；不需要断开网络。

---

# Nioh 3 Scroll Generator v0.6.4

This stable release fixes the three principal v0.6.3 reports and adds an opt-in Beta update channel plus a new complete-effect preimage accelerator.

- FB-014: adds the missing Shinatsuhiko's Grace name (`0x4192`) to the rarity-4 and rarity-5 Grace selectors.
- FB-015: category-capacity and conflict errors now name every involved effect, its ID and native category, the capacity, and the minimum number of selections to remove.
- FB-016: newly installed scrolls now receive an item-instance key unused across the complete save, not only the scroll array. Save transactions also repair strictly recognized scroll/equipment key collisions that can make a valid scroll render or index as equipment.
- Stable updates remain the default. Users may opt into Beta in the header to compare the newest stable Release with signed GitHub prereleases, and can return to stable-only updates at any time.
- Versions ending in `-beta.N` or `-rc.N` are published as GitHub prereleases and are ignored by stable clients. Both channels still require the Ed25519 signature, exact asset size, and SHA-256.
- Rarity 3, 4, and 5 search, known-Seed generation, and installation remain available for the third playthrough.
- A complete-effect preimage path can construct candidates through Direct3D 11 Compute on AMD, NVIDIA, or Intel GPUs when every ordinary effect slot is constrained, followed by exact CPU replay of the complete record.
- CUDA/native-CPU fallbacks, streamed results, structural legality preflight, and fail-closed final-record gates remain active.
- Stable builds no longer label the entire application as Beta, and the tutorial now reflects the actual rarity and update-channel behavior.

This release remains certified only for Nioh 3 PC v2.00.02. Return the game to the title screen before a save write; disconnecting from the network is not required.
