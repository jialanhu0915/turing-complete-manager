---
title: ComponentType 组件目录
last_updated: 2026-08-08
scope: investigation
status: 已审
---

# ComponentType 组件目录

Turing Complete 仿真器支持 **101 种电路组件**（`com_*`）。

数据来源：从 `E:\SteamLibrary\steamapps\common\Turing Complete\replay.nim` 中 grep `com_[a-z_0-9]+`，统计去重得到。

> ComponentType 是 `replay.nim` 嵌入代码里的枚举，**枚举顺序固定**（与 `#COUNTS` 数组下标一一对应）。

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