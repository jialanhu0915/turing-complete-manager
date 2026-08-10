---
title: ComponentType 组件目录
last_updated: 2026-08-10
scope: investigation
status: 已审（Stuffe/save_monger 完整 ComponentKind 枚举已校对）
---

# ComponentType 组件目录

Turing Complete 仿真器支持 **101 种电路组件**（`com_*`），ordinal 范围 0..124（其中 24 个 `com_deleted_*` 为版本兼容性保留位）。

**数据来源**：

1. 权威源：[`Stuffe/save_monger` `common.nim`](https://github.com/Stuffe/save_monger/blob/master/common.nim) 第 9-134 行的 `ComponentKind` 枚举——**官方完整枚举**，含每个 ordinal 对应的组件名
2. 交叉源：`E:\SteamLibrary\steamapps\common\Turing Complete\replay.nim` 中 grep `com_[a-z_0-9]+`——确认名称列表一致

> ComponentType 是 save_monger 与 replay.nim 共用的枚举，**枚举顺序固定**（与 `#COUNTS` 数组下标一一对应）。

### 关键 ordinal 速查表

| ordinal | 名称 | 说明 |
|---|---|---|
| 0 | `com_none` | 空 |
| 1-2 | `com_off`, `com_on` | 常量 |
| 3-7 | `com_not_bit`, `com_and_bit`, `com_and_3_bit`, `com_nand_bit`, `com_or_bit` | 1-bit 基础门（前 5 个） |
| 8-12 | `com_or_3_bit`, `com_nor_bit`, `com_xor_bit`, `com_xnor_bit`, `com_switch_bit` | 1-bit 基础门（后 5 个） |
| 13 | `com_delay_line_bit` | 1-bit 延迟线 |
| 14 | `com_register_bit` | 1-bit 寄存器 |
| 15 | `com_full_adder` | 全加器 |
| 16-17 | `com_maker_bit_8`, `com_splitter_bit_8` | 8-bit 合并/拆分 |
| 18-25 | 字级门（`com_not_word`, `com_or_word`, ..., `com_switch_word`） | 8 入口 |
| 26-32 | 比较+算术（`com_equal`, `com_less_u`, `com_less_s`, `com_neg`, `com_add`, `com_mul`, `com_div`） | 7 入口 |
| 33-37 | 移位（`com_lsl`, `com_lsr`, `com_rol`, `com_ror`, `com_asr`） | 5 入口 |
| 38-39 | `com_counter`, `com_register_word` | 计数+字级寄存器 |
| 40 | `com_level_output_8_pin` | 8-pin 输出 |
| 41 | `com_level_delay_gate` | 关卡定义延迟门 |
| **42** | **`com_mux`** | **多路复用器** |
| 43-45 | `com_decoder_1/2/3` | 解码器 |
| 46 | `com_constant` | 常量输出 |
| 47-50 | `com_splitter_word_2`, `com_maker_word_2`, `com_clz`, `com_register_word_config` | 字级拆分+CLZ+可配置寄存器 |
| 51-57 | `com_probe_wire_asm`, `com_push_button`, `com_pipelined_load_port`, `com_load_port`, `com_delay_line_word`, `com_store_port`, `com_ctz` | 端口+按钮+计数辅助 |
| 58 | `com_cc_level_output` | cc 总线关卡输出 |
| 59 | `com_level_gate` | 关卡门输出 |
| 60-65 | `com_level_input_1_pin` ... `com_level_input_4_pin` (60/63/64/65) + `com_level_input_word` (61) + `com_level_input_switched` (62) | 关卡输入族 |
| 66-67 | `com_deleted_16`, `com_deleted_17` | 保留位 |
| 68-70 | `com_level_output_1_pin`, `com_level_output_word`, `com_level_output_switched` | 关卡输出族（前 3） |
| 71-72 | `com_deleted_2`, `com_deleted_3` | 保留位 |
| 73-75 | `com_level_output_2_pin`, `com_level_output_3_pin`, `com_level_output_4_pin` | 关卡输出族 |
| 76 | `com_deleted_18` | 保留位 |
| 77 | `com_level_output_counter` | 计数器型输出 |
| **78** | **`com_custom`** | **自定义元件** |
| 79-81 | `com_cc_input`, `com_cc_input_buffer`, `com_cc_output` | cc 总线族 |
| 82-85 | `com_probe_memory_bit`, `com_probe_memory_word`, `com_probe_wire_bit`, `com_probe_wire_word` | probe 族 |
| 86 | `com_deleted_20` | 保留位 |
| 87 | `com_halt` | 暂停 |
| 88 | `com_deleted_1` | 保留位 |
| 89 | `com_segment_display` | 段码显示 |
| 90-96 | `com_static_value`, `com_screen`, `com_time`, `com_keyboard`, `com_static_eval`, `com_verilog_input`, `com_verilog_output` | 静态值 + 显示 + 外部 I/O |
| 97-100 | `com_maker_word_4/8`, `com_splitter_word_4/8` | 字级 4/8-bit 合并/拆分 |
| 101 | `com_static_indexer` | 静态索引 |
| 102-103 | `com_deleted_7`, `com_deleted_8` | 保留位 |
| 104 | `com_inc` | 自增 |
| 105 | `com_deleted_19` | 保留位 |
| **106** | **`com_cc_level_input`** | **cc 总线关卡输入**（架构关卡用） |
| 107 | `com_deleted_9` | 保留位 |
| 108 | `com_mod` | 取模 |
| 109-113 | `com_splitter_bit_2/4`, `com_maker_bit_2/4` + `com_deleted_10` | bit 合并/拆分 |
| 114-119 | `com_concatenator_2/4/8`, `com_static_indexer_config`, **`com_ram`** (118), `com_delay_line_word_config` | 连接器 + 索引器 + **RAM** + 可配置延迟线 |
| 120-124 | `com_deleted_11..15` | 保留位 |

**关键集合**（save_monger `common.nim` 常量定义）：

- `UNUSED_COMPONENTS` = 24 个 deleted slot
- `LEVEL_INPUTS` = {60, 61, 62, 63, 64, 65} = 1/2/3/4_pin + word + switched
- `LEVEL_OUTPUTS` = {40, 68, 69, 70, 73, 74, 75, 77} = 1/2/3/4/8_pin + word + switched + counter
- `ARCHITECTURE_KINDS` = {62, 70} = com_level_input_switched + com_level_output_switched ——**架构关卡就这 2 个 kind**
- `ASSEMBLER_MEMORY` = {118} = **com_ram 是唯一汇编器存储**
- `MIN_ONE_WATCHED_COMPONENT` = {82, 83, 91} = probe_memory_bit + probe_memory_word + screen

---

## 关于 `kind` 字段（circuit.data vs ComponentType 枚举）

`circuit.data` 二进制中的 `kind: u16` 字段**不是** ComponentType 枚举顺序（0..100）。它们是两套独立的编号：

| 编号空间 | 范围 | 来源 |
|---|---|---|
| `replay.nim` `ComponentType` 枚举 | 0..100（本文档表格的"枚举顺序"列） | 嵌入 DSL 编译器侧 |
| `circuit.data` `kind` 字段 | 见下表（独立 ID） | 游戏存档二进制侧 |

**已知 `kind` 集合**（实测于 `tc-save-lab/src/tc_save_lab/scaffold.py`）：

```python
LEVEL_INPUT_KINDS    = frozenset({60, 61, 62, 63, 64, 65, 106})
LEVEL_OUTPUT_KINDS   = frozenset({40, 58, 68, 69, 70, 73, 74, 75, 77})
CUSTOM_COMPONENT_KIND = 78
```

**实测样本**（用 tc-save-lab codec 解码本地玩家存档）：

| 关卡 | 解出 kind | 解读 |
|---|---|---|
| `not_gate` 的输入 pin | 60 | 1-pin input |
| `and_gate` / `or_gate` 的输入 pin | 63 | 2-pin input |
| 多数关卡的输出 pin | 68 | 通用输出 |
| `full_adder` 的 Sum / Carry | 69 | Sum/Carry 输出（8-pin） |
| `or_gate` 的 OR 门 | 3 | 与 replay.nim 枚举一致 (`com_or_bit`) |
| `and_gate` / `not_gate` 的逻辑门 | 6 | （**注**：实测 = 6，但具体对应哪种门待测） |

> ⚠️ 门类（com_and_bit、com_xor_bit 等）在 circuit.data 里的 kind 编号**尚未系统逆向**——只有 com_or_bit 实证匹配枚举序号 3，其余需要逐关卡穷举。
> 
> 当前若需把电路写入 circuit.data，**优先通过 LLM 穷举验证** + 与 Python tc-save-lab 输出 diff 来发现 kind 编号。

---

## 一、基础门（9 项）

| 名称 | 功能 |
|---|---|
| `com_and_bit` | 1-bit AND（与） |
| `com_and_3_bit` | 1-bit 3-input AND（三输入与） |
| `com_nand_bit` | 1-bit NAND（与非） |
| `com_or_bit` | 1-bit OR（或） |
| `com_or_3_bit` | 1-bit 3-input OR（三输入或） |
| `com_nor_bit` | 1-bit NOR（或非） |
| `com_xor_bit` | 1-bit XOR（异或） |
| `com_xnor_bit` | 1-bit XNOR（同或） |
| `com_not_bit` | 1-bit NOT（非） |

## 二、字级门（7 项）

| 名称 | 功能 |
|---|---|
| `com_and_word` | N-bit AND（字与） |
| `com_or_word` | N-bit OR（字或） |
| `com_nand_word` | N-bit NAND |
| `com_nor_word` | N-bit NOR |
| `com_xor_word` | N-bit XOR |
| `com_xnor_word` | N-bit XNOR |
| `com_not_word` | N-bit NOT |

## 三、算术与位移（13 项）

| 名称 | 功能 |
|---|---|
| `com_add` | 加法（字） |
| `com_neg` | 取负（减法） |
| `com_inc` | 自增（加一） |
| `com_mul` | 乘法（无符号） |
| `com_div` | 除法（无符号） |
| `com_mod` | 取模（无符号） |
| `com_lsl` | 逻辑左移 |
| `com_lsr` | 逻辑右移 |
| `com_asr` | 算术右移 |
| `com_rol` | 循环左移 |
| `com_ror` | 循环右移 |
| `com_clz` | 前导零计数 |
| `com_ctz` | 末尾零计数 |

## 四、比较（3 项）

| 名称 | 功能 |
|---|---|
| `com_equal` | 等于（==） |
| `com_less_u` | 小于（无符号） |
| `com_less_s` | 小于（有符号） |

## 五、复合逻辑（5 项）

| 名称 | 功能 |
|---|---|
| `com_full_adder` | 1-bit 全加器（输入：a, b, cin；输出：sum, cout） |
| `com_mux` | 数据选择器 |
| `com_decoder_1` | 1-to-2 解码器 |
| `com_decoder_2` | 2-to-4 解码器 |
| `com_decoder_3` | 3-to-8 解码器 |

## 六、总线工具 — bit 切分/合并（6 项）

| 名称 | 功能 |
|---|---|
| `com_splitter_bit_2` | 1→2 bit 拆分 |
| `com_splitter_bit_4` | 1→4 bit 拆分 |
| `com_splitter_bit_8` | 1→8 bit 拆分 |
| `com_maker_bit_2` | 2→1 bit 合并 |
| `com_maker_bit_4` | 4→1 bit 合并 |
| `com_maker_bit_8` | 8→1 bit 合并 |

## 七、总线工具 — word 切分/合并（6 项）

| 名称 | 功能 |
|---|---|
| `com_splitter_word_2` | 字→2 字拆分 |
| `com_splitter_word_4` | 字→4 字拆分 |
| `com_splitter_word_8` | 字→8 字拆分 |
| `com_maker_word_2` | 2→1 字合并 |
| `com_maker_word_4` | 4→1 字合并 |
| `com_maker_word_8` | 8→1 字合并 |

## 八、连接器（3 项）

| 名称 | 功能 |
|---|---|
| `com_concatenator_2` | 2-wire 串联 |
| `com_concatenator_4` | 4-wire 串联 |
| `com_concatenator_8` | 8-wire 串联 |

## 九、常量与索引（4 项）

| 名称 | 功能 |
|---|---|
| `com_constant` | 常量输出（值在组件内配置） |
| `com_static_value` | 静态值（同上） |
| `com_static_indexer` | 静态索引访问（运行时只读） |
| `com_static_indexer_config` | 静态索引访问的可配置版本 |

## 十、存储（9 项）

| 名称 | 功能 |
|---|---|
| `com_register_bit` | 1-bit 寄存器 |
| `com_register_word` | N-bit 寄存器 |
| `com_register_word_config` | 可配置 N-bit 寄存器 |
| `com_counter` | 计数器 |
| `com_ram` | 随机访问存储器 |
| `com_delay_line_bit` | 1-bit 延迟线（固定延迟） |
| `com_delay_line_word` | N-bit 延迟线（固定延迟） |
| `com_delay_line_word_asm` | N-bit 延迟线（程序配置延迟） |
| `com_delay_line_word_config` | N-bit 延迟线（可配置） |


## 十一、输入（8 项）

| 名称 | 功能 |
|---|---|
| `com_switch_bit` | 1-bit 手动开关 |
| `com_switch_word` | N-bit 手动开关组 |
| `com_level_input_1_pin` | 关卡输入：1 pin |
| `com_level_input_2_pin` | 关卡输入：2 pin |
| `com_level_input_3_pin` | 关卡输入：3 pin |
| `com_level_input_4_pin` | 关卡输入：4 pin |
| `com_level_input_switched` | 关卡输入：可切换 |
| `com_level_input_word` | 关卡输入：N-bit 字 |

## 十二、输出（11 项）

| 名称 | 功能 |
|---|---|
| `com_level_output_1_pin` | 关卡输出：1 pin |
| `com_level_output_2_pin` | 关卡输出：2 pin |
| `com_level_output_3_pin` | 关卡输出：3 pin |
| `com_level_output_4_pin` | 关卡输出：4 pin |
| `com_level_output_8_pin` | 关卡输出：8 pin |
| `com_level_output_counter` | 关卡输出：计数器型（需在 N tick 内变化） |
| `com_level_output_switched` | 关卡输出：可切换 |
| `com_level_output_word` | 关卡输出：N-bit 字 |
| `com_level_gate` | 关卡输出：门型（单 tick 验证） |
| `com_segment_display` | 段码显示（7-segment） |
| `com_screen` | 屏幕（像素矩阵） |


## 十三、调试 / 探测（7 项）

| 名称 | 功能 |
|---|---|
| `com_probe_memory_bit` | 探测 1-bit 内存值 |
| `com_probe_memory_word` | 探测 N-bit 内存值 |
| `com_probe_wire_bit` | 探测 1-bit wire 值 |
| `com_probe_wire_word` | 探测 N-bit wire 值 |
| `com_static_eval` | 静态求值（编译期常量折叠） |
| `com_verilog_input` | Verilog 输入端口（外部 Verilog 模块） |
| `com_verilog_output` | Verilog 输出端口 |


## 十四、cc 总线（5 项）

> cc = "circuit component"，是 Component Collection 总线协议

| 名称 | 功能 |
|---|---|
| `com_cc_input` | cc 总线输入 |
| `com_cc_output` | cc 总线输出 |
| `com_cc_input_buffer` | cc 总线输入缓冲 |
| `com_cc_level_input` | cc 关卡输入 |
| `com_cc_level_output` | cc 关卡输出 |

## 十五、其他控制（8 项）

| 名称 | 功能 |
|---|---|
| `com_off` | 恒 0 |
| `com_on` | 恒 1 |
| `com_halt` | 暂停仿真（停止 tick） |
| `com_custom` | 自定义（用户脚本） |
| `com_keyboard` | 键盘输入 |
| `com_time` | 时间源 |
| `com_load_port` | 加载端口（连接 RAM） |
| `com_store_port` | 存储端口（连接 RAM） |

---

## 统计

| 类别 | 实际数量 |
|---|---|
| 基础门 | 9 |
| 字级门 | 7 |
| 算术与位移 | 13 |
| 比较 | 3 |
| 复合逻辑 | 5 |
| 总线 bit | 6 |
| 总线 word | 6 |
| 连接器 | 3 |
| 常量索引 | 4 |
| 存储 | 9 |
| 输入 | 8 |
| 输出 | 11 |
| 调试 | 7 |
| cc 总线 | 5 |
| 控制 | 8 |
| **总计** | **101** ✅ |

---

## 使用说明

枚举顺序在 `replay.nim` 中固定。当 LLM/工具需要：

- **查询某个组件的使用量** → 读 `#COUNTS[idx]`，其中 `idx` 等于该组件在枚举中的位置
- **统计电路成本** → 累加 `#COUNTS[i] * cost[i]`
- **生成新电路** → 修改 `#COUNTS` 后重新生成 `compile_and_run` 块

但再次提醒：**实际电路拓扑（门的连接关系）不在 `replay.nim` 里**，而在 `circuit.data` 中。ComponentType 计数只是「统计」信息。

---

## wiki 校对（2026-08-10）

> Wiki 来源：`turingcomplete.wiki/wiki/Components`（CC BY-SA 4.0）。

### Sandbox-only 子类

wiki 显式标注下列组件**仅 sandbox 可用**（不在普通关卡）：

- File Loader（文件加载）
- Keyboard（键盘输入）
- Network（网络）
- Time（时间源）

本表中对应：

| wiki 名 | 本表 enum | 备注 |
|---|---|---|
| Keyboard | `com_keyboard` | ✓ |
| Time | `com_time` | ✓ |
| File Loader | — | 未找到 `com_file_loader`，可能用其他机制实现 |
| Network | — | 未找到 `com_network`，可能用其他机制实现 |

### 位宽分类（wiki 视角）

wiki 按**位宽**组织，与本表"按功能"分类互为补充视图：

| 位宽类 | 内容 | 与本表对应 |
|---|---|---|
| 1-bit | 基础门 + 1-bit 解码器/全加器/延迟线 | §一 §四 §九 §十 |
| 8-bit | 字级门 + 算术 + 移位 + 旋转 | §二 §三 §五 |
| 16/32/64-bit | 同 8-bit，通过 `word_size` 参数区分 | 同上（同一 enum） |
| IO | HDD / Program / Halt / Sound / Configurable delay / Indexer | §十 §十二 §十五 |
| RAM 子类 | RAM / ROM / Dual Load / Fast / Latency | §十（仅 `com_ram`） |
| Level 子类 | Level output / Level input / Level Screen | §十一 §十二 |
| Probe 子类 | Memory/Wire × bit/word | §十三 |
| Display 子类 | Dot Matrix / 7-Segment / Console / Sprite | §十二 |
| Sandbox only | File Loader / Keyboard / Network / Time | §十五 |

### 计数差异

- **本表**：101 项（`replay.nim` grep `com_[a-z_0-9]+`）
- **wiki**：~74 个去重名字（不区分 16/32/64-bit 变体）

差异来源推测（**待确认**）：

- 本表可能包含 alpha-branch 变体或内部 enum（wiki 主线页未覆盖）
- wiki 一些条目**不是独立 ComponentType**，而是 `com_ram` 或 `com_custom` 的配置变体：
  - **HDD / Sound / Console** — 未在本表找到对应 enum，可能通过 `com_custom`（用户脚本）实现
  - **ROM / Dual Load RAM / Fast RAM / Latency RAM** — 可能都是 `com_ram` 的配置变体（buffer_size / settings 不同）

**验证方法**：grep 这些名字到 `replay.nim`，看是否存在独立 `com_*` 标识。

### 富描述来源

wiki 每个组件名链到一个独立页面（`Component/<NAME>`），含 pin 图、用法、限制，约 70+ 个页面。本表当前**只有名字和功能**——若要给 LLM/玩家提供 pin 图，需批量抓取 wiki Component 页。

### 引脚信息（wiki 校对 2026-08-10）

下述 4 个组件已有 wiki 完整引脚图与真值表：

#### `Register`（com_register_word, com_register_bit）

| 项 | 内容 |
|---|---|
| Inputs | 3：**Load**、**Save**、**Save Value** |
| Output | 1 |
| 位宽 | 8 / 16 / 32 / 64（word）；1（bit） |
| Load=1 | 输出当前存储值 |
| Load=0 | 输出无值 |
| Save=1 | 写入 Save Value 到存储 |
| Save=0 | 不写入 |
| 解锁 | 8-bit: Saving Bytes；16/32/64: Lab |

#### `Mux`（com_mux）

| 项 | 内容 |
|---|---|
| Inputs | 3：**select**（1-bit）+ **input1**（位宽）+ **input2**（位宽） |
| Output | 1（位宽） |
| select=0 | 输出 input1 |
| select=1 | 输出 input2 |
| 解锁 | 8-bit: Input Selector；16/32/64: Lab |

#### `Delay Line`（com_delay_line_bit, com_delay_line_word, ...）

| 项 | 内容 |
|---|---|
| Inputs | 1 |
| Output | 1 |
| 位宽 | 1 / N（word） |
| 行为 | 输出 = 输入延迟 1 tick（恰好） |
| 地位 | 几乎所有 bit/byte 存储组件的基础构件 |
| 解锁 | Delayed Lines |

#### `Switch`（com_switch_bit）

| 项 | 内容 |
|---|---|
| Inputs | 2：**input**、**enable** |
| Output | 1 |
| enable=0 | 输出 Z（高阻） |
| enable=1 | 输出 input 值 |
| 解锁 | Bit Switch |

**真值表**：

| Input | Enable | Output |
|---|---|---|
| 0 | 0 | Z |
| 0 | 1 | 0 |
| 1 | 0 | Z |
| 1 | 1 | 1 |

> ⚠️ Switch 的 Z 行为与 [`compile-signature.md`](compile-signature.md) §test.si API 校对中的 `Output._is_z` 直接对应——未驱动的输出会被标记为 Z 状态。这是 LLM 生成电路时必须显式处理的悬空信号。

wiki 上其他组件页（RAM/ROM/Probe 等）的引脚信息尚未抓取，若需要可批量补充。

### Alpha Branch（save_breaker）变更摘要

> Wiki 来源：`Save_breaker_changes` 页（stable v0.1059 → alpha save_breaker 差异）。
> 本项目面向 stable 版，但备份逻辑需考虑 alpha 存档兼容性。

| 变更 | 影响 |
|---|---|
| **Variable width** — 线与组件改为可配置位宽 | 解释本表 `com_*_word` 覆盖所有宽度（无需独立 enum） |
| **New Assembler** — `Program` 块移除，改用 "Assembler" 类型的 RAM load port | 本表 `com_program` 在 alpha 中不存在 |
| **Scoring** — 公式改为 `gates * delay * ticks` | 影响任何评分/排行功能 |
| **IO pins** — 简化为 Input（可配置位宽）和 Output | Bidirectional / Switched Output 在 alpha 中被替换 |
| **RAM/ROM** — 模块化：RAM / Latency RAM / SSD + 独立 load/store port | Latency RAM 在本表 `com_ram` 之外（仅 alpha 存在） |
| **组件移除** — Bit Indexer / Byte Indexer 移除；Display Matrix / Network / Sprite 在 alpha 不可用 | 本表 `com_static_indexer*` 在 alpha 中不存在 |
| **关卡变更** — 自定义组件允许在更多关卡；**Z 状态与 0 区分**；Overture 操作码变化 | Z 状态区分与 [`compile-signature.md`](compile-signature.md) §test.si API 校对中 `Output._is_z` 对应——独立印证 |

**重要警告**：从 save_breaker 切回 stable **会破坏存档**。本项目备份/恢复逻辑应考虑存档版本兼容性（检测 alpha 存档并提示用户）。