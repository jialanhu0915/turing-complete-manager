---
title: byte_adder 网络连通解析 bug（SDK 已知限制，不阻塞主线）
date: 2026-08-15
status: known-limitation
---

# byte_adder（Ling 8bit）网络连通解析 bug

## 现象

`test` CLI 验证 `byte_adder` 关卡时，第一个用例（0+0=0）就 `test_result=2`（fail）。
简单关卡（`and_gate` / `or_gate` / `not_gate` / `xor_gate` / `full_adder` / `bit_adder`）全部 pass。

## 已排除（均确认正确）

- **输入字段映射**：按位置排序（Carry in / A / B），对应 `a / b / carry_in` 字段序
- **输出字段映射**：按 word_size 排序（carry_out U1 ↔ 1 位、output U8 ↔ 8 位）
- **字 I/O 位级建模**：kind 61（input_word）/ 69（output_word）拆成 N 个 bit 引脚
- **输出位合并**：多引脚输出 `terms.join(" | ")`，`(({ftype} {src}) << {i})`
- **门仿真表达式**：AND=`&`、OR=`|`、NOR=`~(...|...)` 均正确

## 剩余 bug

**某个门的输入 `vidX` 映射到了错误的源** —— 即 `pins.rs` 的 `resolve()` 把 wire 端点
和 pin 归到同一个 net 的那一步，在 byte_adder 的 carry-lookahead 交叉连线下有误。
门仿真表达式是对的，所以问题在**连通性 / net 解析**，不在表达式生成。

## 下一步

- 写一个全电路 trace：把每条 wire 从起点追到终点，对照门的 label 和 bit 下标，
  逐门验证每个 `vidX` 输入的 driver 是否正确。
- `tc-save-lab` 的 `pins.py` 也把 kind 61/69 建模成单「value」pin，同样不处理
  位级电路，不能照抄。

## 相关

- `docs/20-design/M8-mod-sdk.md`
- `tc-mod-sdk/README.md`（Known limitations）
- memory: [[dsl-generator-test-si]] [[game-runtime-architecture]]
