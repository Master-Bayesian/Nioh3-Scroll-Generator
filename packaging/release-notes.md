# 仁王3绘卷生成器 Beta v0.5.4

这是首次启用签名自动更新的引导版本。安装此版本后，应用可以从官方 GitHub Releases 自动升级到后续版本。

- 修复 Seed `91104224` 等候选中已知词条只显示编号的问题。
- 词条选择、目标组合和预览使用完整原生最终态词条目录。
- 修复三周目稀有度 4 候选写入时初始化记录与预览不一致的问题。
- 特殊规则改为普通点击即可多选；每条规则保留完整名称与精确数值变体，并可逐项删除或一键清空。
- 根据游戏开发方即将修复神宝绘卷传播问题的公告，正式入口暂时移除稀有度 5 搜索、生成和写入。
- 简化教程，并将标题界面确认框移到“添加到存档”按钮旁。
- 启用 Ed25519 签名、SHA-256 和精确文件大小校验的自动更新。

首次使用自动更新前，需要手动下载并运行这份重新发布的 v0.5.4。此前没有内置发布公钥的旧 EXE 无法自动获得本版本。

---

This is the bootstrap release for the signed desktop update channel. Install this rebuilt v0.5.4 once; future releases can then be installed automatically after Ed25519 signature, SHA-256, and exact-size verification.
