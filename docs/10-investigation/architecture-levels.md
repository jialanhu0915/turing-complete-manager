---
title: 架构关卡（Architecture Levels）
last_updated: 2026-08-10
scope: investigation
status: 初稿（基于 wiki 校对，未逆向游戏本体）
---

# 架构关卡（Architecture Levels）

> Wiki 来源：`turingcomplete.wiki/wiki/Custom_level_creation/`、`Spec.isa`（CC BY-SA 4.0）。
> 本文是 wiki 校对归档，**不是**逆向结论——架构关卡的字节级协议未实测。

---

## 一、两类关卡对比

| 类别 | 玩家交付物 | DSL 测试接口 |
|---|---|---|
| **组件关卡** | 门级电路图（用 AND/OR/Mux/Register 等基础组件搭建） | `check_output(tick, inputs, outputs)` |
| **架构关卡** | 三件套：自定义 ISA + 汇编程序 + CPU 电路 | `arch_check_output(test, input, output)` |

架构关卡让玩家**先定义一个处理器架构**（ISA），再用这套 ISA **写汇编程序**，最后**用基础组件搭出该 ISA 的 CPU 实现**。比组件关卡高一个抽象层。

例架构关卡：Overture / binary_search / Saving Gracefully / Counter。

---

## 二、文件结构

```
schematics/architecture/<level>/<scheme>/
├── *.asm     # 汇编程序（用 *.isa 定义的 ISA 写成的程序）
├── *.isa     # ISA 声明文件（架构名、寄存器、指令字段、操作码）
└── circuit.data   # CPU 电路（门级实现）
```

| 文件 | 内容 | 类比 |
|---|---|---|
| `*.asm` | 用自定义 ISA 写的可执行程序 | 源代码 |
| `*.isa` | ISA 声明：架构名、变体、字节序、寄存器、指令编码 | 编译器/解码器规范 |
| `circuit.data` | CPU 电路（用基础组件搭出的微架构实现） | 硬件 |

**三者缺一不可**：缺 `*.isa` 则 `*.asm` 无法编译；缺 `*.asm` 则关卡无东西可跑；缺 `circuit.data` 则 ISA 未被实例化。备份/恢复逻辑必须包含 `.isa`，详见 [`level-data.md`](level-data.md) §1.3。

---

## 三、API 差异

### 组件关卡

```simplex
fn check_output(tick: Int, inputs: Input, outputs: Output) TestResult
fn get_input(tick: Int) Input
```

`Input` / `Output` 是带类型的结构体，`Output` 每引脚含 `_is_z` Z 状态字段。详见 [`compile-signature.md`](compile-signature.md) §test.si API 校对。

### 架构关卡（盲区）

```simplex
fn arch_check_output(test: Int, input: Int, output: Int) TestResult
fn arch_get_input(test: Int) Int
```

签名形态**完全不同**：

- 用 `Int`（标量）而非结构体（dict of fields）—— 输入/输出如何编码成单整数未逆向
- 多一个 `test` 参数——架构关卡按测试编号索引输入

**这是文档盲区**：我们目前的逆向结论只覆盖组件关卡的 `check_output` / `get_input`。架构关卡的字节级协议（标量到位的拆分、高低位序、跨字输出如何打包）**未分析**。

---

## 四、Spec.isa 文件结构

`.isa` 是**声明式 ISA 规范**（声明式 DSL，不是汇编）。三个主要部分：

| 部分 | 内容 |
|---|---|
| **Settings** | 架构名、变体、字节序（endianness）、行注释符、块注释符 |
| **Fields** | 指令字段定义（字段名、取值范围、字面量语法；可重复值；空字符串表示可选字段） |
| **Instructions** | 指令定义：汇编格式、虚拟操作数、断言、输出位模式 |

支持的特性：

- 自定义汇编语法与操作数
- 表达式运算符：`+`, `-`, `*`, `/`, `&`, `|`, `^`, `<<`, `>>`
- 内建函数：`asr`, `log2`, `popcount`, `trailing_zeros`
- 位切片（bit slicing）
- 指令地址变量：`$start`, `$end`
- 编译时断言（compile-time assertions）

**未逆向**：`.isa` 文件的具体语法（BNF）、`.asm` 文件的指令编码与操作数序列化。

### Spec.isa 语法片段（wiki 实测）

具体语法要素（抓取 `Components` 与 `Spec.isa` 页交叉确认）：

- 操作数前缀 `%`（如 `%r0`）
- 编译时断言：`!assert ...`
- 字段定义 `name value` 模式（如 `r0 000`、`r1 001`——name 是字段名，value 是二进制位模式）
- 表达式：`+`, `-`, `*`, `/`, `&`, `|`, `^`, `<<`, `>>`
- 内建函数：`asr`, `log2`, `popcount`, `trailing_zeros`
- 位切片（bit slicing）：`field[start:end]` 形式
- 指令地址变量：`$start`, `$end`

指令定义支持：

- 字面量匹配（指令编码直接给二进制位）
- 字段引用（通过字段名引用 Fields 段定义的位模式）
- 两者可混合

**位提取常用掩码**：通过 AND + 移位从指令字中提取子字段。具体掩码值由 ISA 设计者在 Fields 段定义。

完整的指令格式定义需抓取 `?action=raw` 原始 wikitext 进一步解析（本次抓取被模型拒绝原样返回，仅提取要素）。

---

## 五、与现有 docs 的关系

### `circuit-data-format.md` 中的 `selected_programs` 字段

```text
| `selected_programs` | u16 count + (string, string) | 架构关卡程序引用 |
```

二元组 `(string, string)` 推测为 **`(asm_path, isa_path)`**——架构关卡的 (程序, ISA) 配对引用。具体是绝对路径/相对路径/basename **未确认**。

### 组件 vs 架构的编译路径

| 关卡类型 | 编译路径 |
|---|---|
| 组件关卡 | `circuit.data` → 运行时 → `compile.dll`（Nim DSL → 机器码） |
| 架构关卡 | `circuit.data` + `.asm` + `.isa` → 运行时 → ??? |

架构关卡的编译路径**未确认**——可能复用 `compile.dll`（额外加载 `.isa` 后处理 `.asm`），也可能有独立汇编器。需要抓游戏可执行文件中的字符串与符号才能确定。

---

## 六、对项目的影响

| 影响点 | 状态 |
|---|---|
| 电路优化 | 本项目不优化架构关卡（成本不划算，玩家也少） |
| 备份/恢复 | `.isa` 必须包含（已记入 `level-data.md` §1.3） |
| 逆向工程 | 留待未来（见 §七） |

---

## 七、待续工作

- **W-1.** 逆向 `*.isa` 文件的完整格式（BNF + Settings/Fields/Instructions 段落的字节编码）
- **W-2.** 逆向 `*.asm` 文件的指令编码与操作数序列化
- **W-3.** 抓 `arch_check_output` 在游戏可执行文件中的实现，确认输入/输出 `Int` 的字节级编码
- **W-4.** 确认 `circuit.data` 中 `selected_programs` 二元组的实际指向（绝对路径？相对路径？basename？）
- **W-5.** 抓取 `Component/Architecture` 系列 wiki 页（约 5-10 页）扩充架构组件目录