---
title: Hint 系统（多步提示）
last_updated: 2026-08-11
scope: investigation
status: 已审（2026-08-11 实测 `E:\SteamLibrary\steamapps\common\Turing Complete\campaign\` 全部关卡）
---

# Hint 系统（多步提示）

> 教学关卡自带的**多步提示机制**：`.data`（电路）+ `.txt`（教学文字）配套，玩家在游戏内点 Hint 按钮逐级展开。
>
> ⚠️ **核心警告**：`hint_solution.data` **是官方示例解，不是最优解**。详见[§示例解与最优解的区别](#示例解与最优解的区别)。

## 文件结构

```
campaign/<level>/
├── hint_0.data          ← 第 1 步电路（v13/v14，依赖关卡格式）
├── hint_0.txt           ← 第 1 步教学文字（i18n 元组）
├── hint_1.data          ← 第 2 步电路（可选）
├── hint_1.txt
├── ...
├── hint_N.data          ← 第 N 步电路（可选）
├── hint_N.txt
├── hint_solution.data   ← 官方示例解（可选）
└── hint_solution.txt    ← 示例解说明文字
```

每个 `*.data` 都有同名 `*.txt`，**`.data` 和 `.txt` 一一对应**。

## 覆盖率（2026-08-11 实测）

| 类别 | 关卡数 | 占比 |
|---|---|---|
| 总关卡 | 98 | 100% |
| 有 `hint_solution.data` | 66 | 67% |
| 有任意 `hint_*.data`（含 `hint_solution`） | 67 | 68% |
| 有多步提示（`hint_0` 到 `hint_N`） | 37 | 38% |
| 无任何 hint | 31 | 32% |

> 1 个差值：某个关卡有 `hint_0..N.data` 但无 `hint_solution.data`（玩家做完提示步骤后自行完成最终电路）。

## 格式版本

`.data` 文件的格式版本 **= 同关卡 `circuit.data` 版本**：

| 关卡 | `circuit.data` | `hint_solution.data` |
|---|---|---|
| `and_gate` | v13 (`0x0D`) | v13 (`0x0D`) |
| `byte_adder` | v13 (`0x0D`) | **v14 (`0x0E`)** |

**关键**：hint 文件**不走 schematics 的 v15 格式**——它们是 campaign 关卡定义的一部分，遵循 campaign 编码规范。

读 hint 文件需要 **v13/v14 codec**（test/verify-cli 的 `circuit/legacy.rs` 已有 v13/v14 只读解码器）。

## `.txt` 内容格式

抽样 `and_gate/hint_0.txt`：

```
[text text=(31337_68158507707802, `The AND gate can be interpreted as a NOT-NOT-AND gate, or a NOT-NAND gate`) size=37]
```

- `(31337_..., text)` —— i18n key + 英文 fallback 元组
- `size=37` —— UI 字号
- 配套的 `ui.txt` 定义了**文字显示在哪、字号多大、何时显示**

这是**给玩家看的教学指导**，不是电路说明——`.data` 才是电路本身。

## 实测样本路径

| 关卡 | hint 文件 | 说明 |
|---|---|---|
| `and_gate` | `hint_0.data` (116B) / `hint_solution.data` (143B) | 2 步（1 提示 + 示例） |
| `always_on` | `hint_0.data` (61B) / `hint_solution.data` (89B) | 2 步 |
| `byte_adder` | `hint_solution.data` (1015B, **v14**) | 仅示例，无多步 |
| `bit_inverter` | `hint_0.data` / `hint_1.data` / `hint_solution.data` | 3 步 |
| `any_doubles` | `hint_0.data` / `hint_1.data` / `hint_solution.data` | 3 步 |

> 注意：hint_solution.data 文件**不一定比 circuit.data 小**——`byte_adder/hint_solution.data` 1015B，但 `byte_adder/circuit.data` 仅 205B。**示例解可能包含完整电路 + 大量可移除组件**，大小不反映"门数优化程度"。

## 示例解与最优解的区别

**这是本调研最重要的发现**——之前讨论中一度把"hint_solution"当作"最优解"是不准确的。

### 真实语义

| 维度 | `hint_solution.data` | 真正最优 |
|---|---|---|
| 来源 | Stuffe 手工写 | 穷举 / LLM 生成 / 玩家创新 |
| 目的 | 教学思路 | 达到 S 级（最少门 / 最低延迟） |
| 风格 | "标准写法"，让新手看懂每根线 | 极简，可能非常 clever |
| 门数 | 通常不是理论最优 | 全局最优（数学上保证） |
| 评分 | 通常 B/A 级 | S 级 |

### 为什么 Stuffe 不写最优

Stuffe 是游戏开发者，不是关卡设计者会追求"难度梯度"——hint_solution 必须：

1. **学生能看懂**——门摆放有规律，导线清晰
2. **展示思路**——故意用某种特定方法（如 NOT-NOT-AND），让玩家理解原理
3. **不能太短**——1 门的 trivial 解没有教学价值

### 对工具设计的启示

如果工具 UI 显示"hint_solution"，必须：

1. **明确标注 "Hint, not optimal"**——避免误导玩家停止优化
2. **提供对比维度**：玩家方案 vs hint vs 历史最佳
3. **不要把 hint_solution 当作"目标"**——它是"起点"

```
玩家方案:   5 gates, 4 delay  → B 级
Hint:       6 gates, 5 delay  → B 级 (官方示例解, 非最优)
```

## 玩家何时用 hint

游戏内机制（**未实测，仅根据 wiki / 常识推断**）：

- 玩家在关卡内点击 "Hint" 按钮
- 第 1 次点击：显示 `hint_0`（最小子电路 + 教学文字）
- 第 2 次点击：显示 `hint_1`（更接近完整）
- ...
- 最后一次点击：显示 `hint_solution`（完整示例）
- 每次点击 hint 可能消耗"提示次数"或降低最终评分（**待实测确认**）

## 与其他系统的关系

| 关联项 | 关系 |
|---|---|
| `circuit.data` | 同关卡同格式（v13/v14），但**不是关卡定义**——关卡定义是 `circuit.data`（含红色不可删组件） |
| `meta.txt` | 包含 `kind`（关卡类型），决定是否需要 hint |
| `ui.txt` | hint_*.txt 引用的 `id` 在 ui.txt 里定义显示位置 |
| `test.si` | hint 解必须能通过 test.si 验证（否则就不是合法解） |
| `schematics/`（玩家存档） | 玩家方案是 v15；hint 是 v13/v14——**两个不同 codec 体系** |

## 待续 / 盲区

- ❌ **hint 的游戏内触发机制**未实测（按钮在 UI 哪里？消耗什么？影响评分？）——只看了文件
- ❌ **多步提示的内部逻辑**未确认——是固定顺序还是玩家可选？
- ❌ **`hint_solution` 的 S 级阈值**——Stuffe 的示例解自身通常能到哪个等级？需要解析 hint_solution.data 算门数
- ❌ **写 hint 文件的可行性**——我们能 v13 读，是否能 v13 写？test/verify-cli 的 `circuit/codec.rs` 只支持 v15 写；v13/v14 写是潜在工作

## 相关文档

- [`level-data.md`](level-data.md) §2.2.3 — 摘要
- [`circuit-data-format.md`](circuit-data-format.md) — v13/v14/v15 codec 细节
- [`compile-signature.md`](compile-signature.md) §test.si — hint 解要能通过 test.si 验证