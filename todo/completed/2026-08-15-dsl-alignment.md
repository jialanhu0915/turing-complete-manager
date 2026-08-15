---
title: DSL 生成器与当前游戏版本对齐（让 test 端到端 pass）
date: 2026-08-15
status: done
---

# DSL 生成器与当前游戏版本对齐

## 现象

2026-08-15 实测 `verify` CLI 端到端跑通（SDK 抽取后），但 `compile.dll` 拒绝编译生成的 DSL：

```
COMPILER ERROR
Expected the line to end here. (1803)  run {
{"ok":true,"test_result":0,"cycles_run":0,"error":null}
```

`test_result=0` / `cycles_run=0` = 编译失败、测试没真正跑。

## 判断

- **SDK 抽取本身正确**：管线「读 circuit → decode → parse test.si → gen DSL → compile.dll」全通，无 import/路径断裂。
- `gen.rs` / `test_si.rs` 是 `test/verify-cli`（0.1→0.2 时代）写的，当时 `and_gate` 实测 pass。
- 游戏已更新：`compile.dll` 2026-08-11 改过，主程序 Nim 2.2.6。DSL 方言 / `test.si` 格式可能随更新漂移。

## 排查步骤

1. 抓取 `gen.rs` 生成的实际 DSL（用 exec.rs 的 dump 测试或临时写盘），定位第 1803 行 `run {` 语法错在哪。
2. 对比当前游戏自己生成的 DSL 样板（`replay.nim` 的 `compile_and_run` 块，或 `sim-shim/prefix.dsl`）。
3. 核对 `test_si.rs` 解析当前 `campaign/*/test.si` 的字段是否对齐。
4. 修正 `gen.rs` / `test_si.rs` 到当前 dialect。

## 验收

`verify --game <game> --save <save> --level and_gate --scheme 缺省` 返回 `test_result=pass` 且 `cycles_run>0`。

## 结果（2026-08-15 完成）

根因不是命名/空行，而是**当前 compile.dll（2026-08-11）方言的 switch case 需要 `case` 关键字前缀**：`case run {` / `case refresh {}` / `case mode_reset {` / `case quit_simulation {`（旧语法 `run {` 缺 `case` 报 "Expected the line to end here"）。

- 修复：`gen.rs` 4 处 switch case 加 `case` 前缀（commit `dc49f55`）
- 验证：and_gate / or_gate / not_gate / xor_gate 全部 `test_result=0`（pass）
- 关键坑：磁盘 `replay.nim`（7 月）是旧方言，别当参考；权威源是当前 exe（Nim 2.2.6）字符串里嵌的主循环模板
- 另：CLI 从 `verify` 更名为 `test`（commit `8abb43a`）

## 相关

- 搬运 `17bd042` / 抽取 `deab5c0` / M8 设计 `9cd53f5`
- `docs/20-design/M8-mod-sdk.md`
- memory: [[dsl-generator-test-si]] [[compile-dll-dsl-restrictions]] [[jit-calling-convention]]
