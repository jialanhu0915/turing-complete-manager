---
title: circuit.data 二进制格式
last_updated: 2026-08-08
scope: investigation
status: 部分逆向（结构骨架已识别，完整 schema 未完成）
---

# `circuit.data` 二进制格式

> ⚠️ **当前状态**：已识别文件骨架、固定区段、ASCII 标签，但完整 schema 未完成。**未达到可读写**的水平。
> 完整逆向需要进一步工作（详见末尾"待续工作"）。

## 概要

- **位置**：`%APPDATA%\Turing Complete\schematics\<level_id>\<scheme>\circuit.data`
- **类型**：二进制，无外部 schema 文档
- **大小范围**：50 B（空模板）— 5439 B（复杂架构关卡）
- **分布**：112 个主文件 + ~225 个 backup

样本盘点和 size 分布见 `level-data.md` § 1.2。

---

## 一、固定结构骨架

通过对 28 个 100–300 B 文件的逐字节统计分析，识别出文件骨架：

```
┌──────────────────────────────────────────────────────────────┐
│ [0x00–0x0F] 16 字节文件头                                    │
│   0x00   = 0x0F  (magic byte, 100% 常量)                     │
│   0x01–0x02     文件 ID（多数文件在此变化）                   │
│   0x03   = 0x20 (空) / 0x30 (有组件)                          │
│   0x04–0x0B     8 字节 UUID-like 随机数（每文件不同）        │
│   0x0C–0x0F     00 00 00 00 (padding, 100% 常量)             │
├──────────────────────────────────────────────────────────────┤
│ [0x10–0x2F] 32 字节结构区 + I/O 模板                         │
│   0x10  XX  ← 候选"首块类型字节"（强候选）                  │
│   0x11  01 (100% 常量)                                       │
│   0x12  05 (75% 常量)                                        │
│   0x13–0x1F  块数据                                          │
│   0x20–0x2F  fe 01 00 重复 6 次（I/O 表，详见 § 三）        │
├──────────────────────────────────────────────────────────────┤
│ [0x30–end] 关卡特有数据                                       │
│   ASCII 标签、组件定义、连接关系、位置坐标等                 │
└──────────────────────────────────────────────────────────────┘
```

**关键常量**（跨 28 个文件验证）：

| 偏移 | 值 | 性质 |
|---|---|---|
| `0x00` | `0x0F` | magic byte，文件格式标识 |
| `0x0F` | `0x00` | padding |
| `0x11` | `0x01` | 结构区第二字节恒为 01 |
| `0x12` | `0x05` | 75% 文件恒为 05 |

**高熵随机区**：`0x04–0x0B`（8 字节），28 个文件几乎全不同。类似 UUID 或 per-file nonce。

---

## 二、byte 0x10 — 候选"首块类型字节"

这是最有价值的发现。byte 0x10 在不同关卡间的取值：

| 关卡 | byte 0x10 | 解读 |
|---|---|---|
| `binary_racer`（空） | `0x08` | 空模板标志 |
| `introduction`（空） | `0x08` | 空模板标志 |
| `and_gate` | `0x02` | 首块 = AND 门 |
| `not_gate` | `0x01` | 首块 = NOT 门 |
| `or_gate` | `0x03` | 首块 = OR 门 |
| `full_adder` | `0x09` | 首块 = 复合组件？ |

**重要观察**：`0x10` 与 `replay.nim` 中的 `ComponentType` 枚举顺序**不对应**（AND=2 ≠ com_and_bit=0，OR=3 = com_or_bit ✓，NOT=1 ≠ com_not_bit=8）。

可能解释：
- 游戏内部对组件有自己的 block ID 编号，与 `replay.nim` 的 ComponentType 枚举不一致
- 或者 `0x10` 不是"组件类型"而是"块类型"（包含位置/连接信息）

**仍需验证**：对 byte 0x10 的语义最终确认需要：
- 找一个含多种门类型的关卡（如 `full_adder`）做穷举
- 或者反编译 `compile.dll` 查看内部结构体定义

---

## 三、ASCII 标签（level I/O names）

跨 112 个文件统计的**真实**标签（非随机字节巧合）：

| 标签 | 出现次数 | 用途 |
|---|---|---|
| `Input` | 33 | 关卡输入（数字关卡常用） |
| `Out` | 27 | 关卡输出（短标签） |
| `Output` | 14 | 关卡输出（长标签） |
| `Result` | 15 | ALU 输出 |
| `flags` | 15 | ALU 标志位 |
| `Instruction` | 15 | 架构关卡输入 |
| `Main Memory` | 13 | 内存信号 |
| `Reg 0` | 13 | 寄存器编号 |
| `Register File` | 11 | 寄存器堆 |
| `Program` | 9 | 程序 ROM |
| `Count` | 9 | 计数器输出 |
| `new_program.asm` | 11 | 架构关卡的汇编文件名引用 |

**观察**：
- "Input" / "Out" / "Output" 几乎在所有关卡出现（基础 I/O 标签）
- ALU/架构关卡有丰富的额外标签（Result, flags, Reg 0, Main Memory）
- `new_program.asm` 出现 11 次 → 架构关卡的电路存档同时包含对 `.asm` 文件名的引用

---

## 四、文件结构示意（基于已知片段）

```
[16-byte 文件头]
  ↓
[结构区]
  XX 01 05 YY YY YY 00 00 00  ← "块"结构（每块 8 字节？）
  ↓ (可能多块)
[重复结构 / 组件定义]
  ↓
[ASCII 标签区]                  ← "Input\0", "Output\0", "Result\0" 等
  ↓
[位置/连接数据]                 ← 坐标、wire IDs
  ↓
[可能的尾部]
  0d 0b 10 fe ff ff ff 06 09 16 48 fe ff 01 00 06 80 00 00 00 00 00
  ← OR/full_adder 共有的尾部片段，AND/NOT 无此段（意义未明）
```

---

## 五、与 `replay.nim` 的关系

`circuit.data` 与 `replay.nim` 编码的是**完全不同的数据**：

| 数据 | `circuit.data` | `replay.nim` |
|---|---|---|
| 电路拓扑 | ✅（本文件） | ❌ |
| 组件类型 | ✅ | ✅ `#COUNTS` |
| 组件连接 | ✅ | ❌ |
| UI 状态 | ❌ | ✅ `ui_set_*` |
| 测试输入/输出历史 | ❌ | ✅ `output_history_pins` |
| 仿真驱动代码 | ❌ | ✅ |

**结论**：要修改电路（添加门、连线、改布局），必须编辑 `circuit.data`；`replay.nim` 只是运行时重放脚本。

---

## 六、对 CLI 目标的影响

CLI 想做"用游戏本体验证 LLM 生成的电路"，关键路径：

```
1. LLM 生成候选电路 → 需要写电路.data
2. 写到新 scheme 文件夹 ← 已知结构（每关一个子目录）
3. 让游戏加载并跑测试 ← 需 D-7（注入机制）
4. 读 sim_test_result    ← 已知 memory layout（command-state.md）
```

**当前阻断点**：
- ❌ 第 1 步：**写**电路.data 仍不可行（schema 不完整）
- ❌ 第 3 步：注入机制未解（D-7）
- ✅ 第 2 步：基础设施已有
- ✅ 第 4 步：原理已知（需要 D-1 打通 DLL 调用通道）

---

## 七、待续工作

按优先级：

### W-1. 完成 schema 逆向
- **方法 A**：找含多门类型的关卡（如 `full_adder`）穷举
- **方法 B**：用 IDA/Ghidra 反编译 `compile.dll`，找到 `circuit.data` 的反序列化函数
- **方法 C**：用 Godot 资源工具（Godot 自带 `--export-debug`）尝试解析

**推荐 B**：dump 出来的字符串表 + 类型签名比纯字节分析快 10 倍。

### W-2. 找到门类型字节的最终语义
- 在多个含异构门的关卡上验证 byte 0x10 的取值范围
- 建立 `byte 0x10 → 组件名` 的查表

### W-3. 写回合法性测试（W-3）
- 拿一个关卡备份，把 `circuit.data` 复制回原位
- 启动游戏，加载关卡，验证无崩溃 + 门数相同
- 改成"加一个门"的版本，再测一次

### W-4. 文档化完整 schema
- 完成 W-1 + W-2 后，更新本文档为"完整版"
- 提供一个 Python 读写函数库（`circuit_io.py`）

---

## 八、关键样本数据（备忘）

### byte 0x10 取值分布（28 文件 100–300B）

| byte 0x10 | 出现次数 | 含义推测 |
|---|---|---|
| 0x01 | 6 | NOT 门 / 1-输入块 |
| 0x02 | 6 | AND 门 / 2-输入块 |
| 0x03 | 5 | OR 门 / 多-输入块 |
| 0x08 | 4 | 空模板 |
| 0x09 | 2 | 复合组件首块 |

### 文件尾共有的片段

```
0d 0b 10 fe ff ff ff 06 09 16 48 fe ff 01 00 06 80 00 00 00 00 00
```

出现在 OR 和 full_adder 中。AND/NOT 中没有——可能与"输入/输出引脚数"或"块数量"有关。

### magic byte 0x0F 的含义

`0x0F` 作为文件首字节，可能是：
- Godot 的 `ResourceLoader` 内部格式标识
- 或游戏自定的存档格式魔数

无法仅凭字节判断，需要查 Godot 源码或 dump 字符串。

---

## 九、参考样本路径

| 关卡 | 路径 | 用途 |
|---|---|---|
| 空模板 | `schematics/binary_racer/缺省/circuit.data` (50B) | 基线对照 |
| 单门（NOT） | `schematics/not_gate/缺省/circuit.data` (189B) | 最小非空样本 |
| 单门（AND） | `schematics/and_gate/缺省/circuit.data` (220B) | 2-输入最小 |
| 单门（OR） | `schematics/or_gate/缺省/circuit.data` (254B) | 2-输入 OR |
| 复合 | `schematics/full_adder/标准/circuit.data` (489B) | 多类型门 |
| 最大 | `schematics/byte_adder/Ling 8bit/circuit.data` (5439B) | 复杂电路 |
| ALU | `schematics/symphony_alu/...` | 含 Result/flags/Instruction 标签 |
| 架构 | `schematics/architecture/.../circuit.data` | 含 `new_program.asm` 引用 |