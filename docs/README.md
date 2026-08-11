---
title: Wiki 索引
last_updated: 2026-08-11
scope: index
status: 已审（2026-08-11 补 hint-system.md；level-data.md §2.2.3 补 hint 摘要；custom-level-packaging.md 初稿归档至 90-appendix，待写替代版）
---

# Turing Complete Manager · 调研 Wiki

本目录沉淀对 **Turing Complete** 游戏本体与存档系统的调研结果。

> 本 wiki **仅做调研**，未实现任何新功能。
> 详见 `20-design/index.md`。

## 阅读顺序

1. **`00-overview.md`** — 项目总览、范围、术语
2. **`10-investigation/`** — 七篇调研文档，建议按下面顺序读：
   1. `dll-analysis.md` — 了解游戏本体有哪些可调用的 DLL
   2. `replay-format.md` — 了解 `replay.nim` 是干什么的、怎么生成
   3. `component-catalog.md` — 完整的 101 个电路组件目录
   4. `command-state.md` — 仿真命令与状态字段
   5. `level-data.md` — 存档目录、关卡定义、玩家电路存档、**hint 系统摘要**
   6. `circuit-data-format.md` — 玩家电路存档的二进制格式骨架
   7. `hint-system.md` — **多步提示系统详解**（hint_0..N + hint_solution 的语义、覆盖率、与最优解的区别）
   8. `custom-level-packaging.md` — **自制关卡打包/分享规范**（M7 设计前置；zip 结构 + manifest.json + 合法性检查 + 降级路径；2026-08-11 待写——见附录归档版）
3. **`20-design/index.md`** — 后续设计方向占位（CLI / LLM 优化器等）
4. **`90-appendix/archived-investigations/`** — 归档的初稿（已被替代但保留作为反面教材）：
   1. `custom-level-packaging-2026-08-11-pre-correction.md` — 自制关卡打包/分享初稿（基于推断、未实测；2026-08-11 实地核对后指出 5 处关键错误；存档目的：避免以后重蹈覆辙）

## 目录约定

- `00-` 项目总览
- `10-` 调研（Investigation）：事实性内容，已审
- `20-` 设计（Design）：方案、待开始
- `30-` 用法（Usage）：用户/开发者操作指南（暂无）
- `90-` 附录（Appendix）：归档的初稿、被替代的旧版本、参考表（2026-08-11 起启用）

每篇文档头部包含 frontmatter：`title`、`last_updated`、`scope`、`status`。