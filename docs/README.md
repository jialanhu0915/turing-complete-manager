---
title: Wiki 索引
last_updated: 2026-08-08
scope: index
status: 已审
---

# Turing Complete Manager · 调研 Wiki

本目录沉淀对 **Turing Complete** 游戏本体与存档系统的调研结果。

> 本 wiki **仅做调研**，未实现任何新功能。
> 详见 `20-design/index.md`。

## 阅读顺序

1. **`00-overview.md`** — 项目总览、范围、术语
2. **`10-investigation/`** — 六篇调研文档，建议按下面顺序读：
   1. `dll-analysis.md` — 了解游戏本体有哪些可调用的 DLL
   2. `replay-format.md` — 了解 `replay.nim` 是干什么的、怎么生成
   3. `component-catalog.md` — 完整的 101 个电路组件目录
   4. `command-state.md` — 仿真命令与状态字段
   5. `level-data.md` — 存档目录、关卡定义、玩家电路存档
   6. `circuit-data-format.md` — 玩家电路存档的二进制格式骨架
3. **`20-design/index.md`** — 后续设计方向占位（CLI / LLM 优化器等）

## 目录约定

- `00-` 项目总览
- `10-` 调研（Investigation）：事实性内容，已审
- `20-` 设计（Design）：方案、待开始
- `30-` 用法（Usage）：用户/开发者操作指南（暂无）
- `90-` 附录（Appendix）：表格、清单（暂无）

每篇文档头部包含 frontmatter：`title`、`last_updated`、`scope`、`status`。