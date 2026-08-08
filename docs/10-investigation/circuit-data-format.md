---
title: circuit.data 二进制格式
last_updated: 2026-08-08
scope: investigation
status: 已审（外部参考实现确认）
---

# `circuit.data` 二进制格式

> ✅ **schema 已确认**：通过外部参考实现 `tc-save-lab` 验证完整读写。
> 本文档记录关键发现和外部参考来源。

## 概要

`circuit.data` 在 **两种地方出现**，**两种版本格式**：

| 位置 | 用途 | 版本 |
|---|---|---|
| `E:\SteamLibrary\steamapps\common\Turing Complete\campaign\<level>\circuit.data` | 游戏关卡定义 | **v13**（旧格式） |
| `%APPDATA%\Turing Complete\schematics\<level>\<scheme>\circuit.data` | 玩家存档 | **v15**（当前格式） |

- **第 1 字节 = 格式版本号**
- **其余 = Snappy 压缩的二进制数据**
- 解压后：含 Circuit 元数据 + 组件列表 + 连线列表

---

## 文件布局

```
┌──────────────────────────────────────────────┐
│ [0x00] 1 字节：格式版本                      │
│         0x0F = v15（玩家存档）               │
│         0x0D = v13（游戏本体 campaign）      │
├──────────────────────────────────────────────┤
│ [0x01..end] Snappy 压缩的 Circuit body       │
└──────────────────────────────────────────────┘
```

**重要**：之前我们看到的"mysterious UUID-like 字节"和"fe 01 00 重复模式"，**不是**文件结构——是 Snappy 压缩流的内部表示。byte-level 分析无法逆向出 schema。

---

## 解压后的 Circuit 结构（v15）

解压后按 `binary.py` 的小端序 reader 解析：

### 顶层 Circuit

| 字段 | 类型 | 说明 |
|---|---|---|
| `custom_id` | i64 | 自定义元件 ID（0 = 普通电路） |
| `hub_id` | u32 | hub 标识 |
| `gate` | i64 | 总门数（cost） |
| `delay` | i64 | 总延迟 |
| `menu_visible` | bool | UI 中是否显示 |
| `clock_speed` | u64 | 时钟频率 |
| `dependencies` | u16 + i64[] | 依赖的自定义元件 ID 列表 |
| `description` | u16-len + UTF-8 | 玩家描述 |
| `sync_state` | u8 | 同步状态 |
| `score` | u16 | 当前得分 |
| `player_data` | u16-len + bytes | 玩家私有数据 |
| `hub_description` | u16-len + UTF-8 | hub 描述 |
| `design` | 512 bytes | 仅当 `custom_id != 0` |
| `components` | i64 count + ... | 见下 |
| `wires` | i64 count + ... | 见下 |

### Component

| 字段 | 类型 | 说明 |
|---|---|---|
| `kind` | u16 | ComponentType 枚举值（见下） |
| `position` | (i16, i16) | (x, y) 坐标 |
| `rotation` | u8 | 旋转（0-3） |
| `permanent_id` | i64 | 唯一 ID（玩家拖动不会变） |
| `user_label` | u16-len + UTF-8 | 玩家给的标签（"Input"等） |
| `custom_string` | u16-len + UTF-8 | 自定义字符串 |
| `settings` | u16 count + u64[] | 组件配置参数 |
| `buffer_size` | i64 | RAM 缓冲大小 |
| `ui_order` | i16 | UI 显示顺序 |
| `word_size` | i64 | 字长（位） |
| `immutable` | bool | 不可变（关卡定义用） |
| `cost_gate` | i64 | 此组件门数 |
| `cost_delay` | i64 | 此组件延迟 |
| `little_endian` | bool | 字节序 |
| `init_data` | u8 | 初始数据 |
| `linked_components` | u16 count + 5-tuples | 关联组件（多态端口） |
| `selected_programs` | u16 count + (string, string) | 架构关卡程序引用 |
| **条件字段**（kind == 78）： |
| `custom_id` | i64 | 自定义元件 ID |
| `custom_word_sizes` | u16 count + (i64, i64) | 自定义字长 |

### Wire

| 字段 | 类型 | 说明 |
|---|---|---|
| `color` | u8 | 颜色 ID |
| `comment` | u16-len + UTF-8 | 玩家注释 |
| `start` | (i16, i16) | 起点 |
| `segments` | u16 循环（length=0 结束） | 路径段，bits 13-15=direction, bits 0-12=length |

---

## ComponentType 枚举（实测样本）

实测几个简单关卡，验证 `replay.nim` 里的枚举顺序：

| 关卡 | kind | 推测 |
|---|---|---|
| `and_gate` 的 XOR 门 | 6 | com_xor_bit（实际枚举第 6）|
| `not_gate` 的 XOR 门 | 6 | com_xor_bit |
| `or_gate` 的 OR 门 | 3 | **com_or_bit** ✓ 与 ComponentType 枚举一致 |
| `full_adder` Sum/Carry 输出 | 69 | 特殊输出类型 |

**已知 kind 集合**（按 `tc-save-lab/scaffold.py`）：

```python
LEVEL_INPUT_KINDS  = frozenset({60, 61, 62, 63, 64, 65, 106})
LEVEL_OUTPUT_KINDS = frozenset({40, 58, 68, 69, 70, 73, 74, 75, 77})
CUSTOM_COMPONENT_KIND = 78
```

---

## 关键 ASCII 标签

实际解码后（不再是随机字节巧合）：

| 标签 | 出现次数 | 用途 |
|---|---|---|
| `Input` | 33+ | 关卡输入 |
| `Input 0/1/2` | 多 | 多输入关卡 |
| `Out` / `Output` | 27/14+ | 关卡输出 |
| `Result` | 15 | ALU 输出 |
| `flags` | 15 | ALU 标志位 |
| `Instruction` | 15 | 架构关卡输入 |
| `Main Memory` | 13 | 内存信号 |
| `Reg 0` | 13 | 寄存器编号 |
| `Register File` | 11 | 寄存器堆 |
| `Program` | 9 | 程序 ROM |

---

## 与 `replay.nim` 的关系（再确认）

`circuit.data` 与 `replay.nim` 编码**完全不同的数据**：

| 数据 | `circuit.data` | `replay.nim` |
|---|---|---|
| 电路拓扑 | ✅ | ❌ |
| 组件类型 | ✅ | ✅ `#COUNTS` |
| 组件连接（wire） | ✅ | ❌ |
| UI 状态 | ❌ | ✅ |
| 测试输入/输出历史 | ❌ | ✅ |

---

## 外部参考实现

### `tc-save-lab`（已验证可工作）

`B:\VS_Code_Project\turing-complete-optimizer\` 是另一个项目，提供：

- `src/tc_save_lab/codec.py` — 完整的 v15 编解码器（含 `decode_v15` / `encode_v15`）
- `src/tc_save_lab/legacy_codec.py` — v7/v13/v14 只读解码器
- `src/tc_save_lab/binary.py` — 小端二进制 reader/writer 原语
- `src/tc_save_lab/snappy.py` — 纯 Python Snappy 编解码
- `src/tc_save_lab/model.py` — 完整的数据模型（`Circuit`、`Component`、`Wire` dataclasses）
- `src/tc_save_lab/scaffold.py` — 关卡脚手架提取（输入/输出 pin 配置）

**实测结果**（用该项目的 codec 解码我们的样本）：
- `and_gate/缺省` v15 → 4 components, gate=2, delay=2 ✓
- `not_gate/缺省` v15 → 3 components, gate=1, delay=1 ✓
- `or_gate/缺省` v15 → 5 components, gate=3, delay=2 ✓
- `full_adder/标准` v15 → 10 components, gate=9, delay=4 ✓
- `campaign/and_gate` v13 → 2 components (Input/Output, immutable) ✓

**`tc-save-lab` 的工作流（已实现）：**
- 严格 v15 读写 + round-trip 校验
- 92 个主线关卡独立 examples 目录
- 离线穷举验证组合逻辑
- 原子写回（`apply`/`install-reviewed` 子命令）
- 安全检查：游戏运行时拒绝写

**`tc-save-lab` 不提供的（我们要补的）：**
- ❌ 调用 `compile.dll` / 不启动游戏直接驱动仿真
- ❌ 把候选电路"喂给游戏本体"做端到端验证
- ❌ LLM 集成（生成候选）
- ❌ Campaign v13/v14 的写（只读）

---

## 我们 CLI 要做的事（基于参考实现）

```
tc-save-lab 提供的         我们 CLI 需要补的
─────────────────         ─────────────────
v15 codec (读写)    ──┐
v13/v14 codec (读)  ──┤
关卡脚手架提取       ──┼──→  游戏本体调用 (compile.dll)  ← 新增
纯 Python 仿真      ──┤    LLM 集成                  ← 新增
原子写回机制        ──┘    per-scheme 隔离工作流      ← 新增
```

我们的 manager CLI **不只是 tc-save-lab 的复制**——核心增量是"用游戏本体（不是离线模拟）验证候选电路"。

---

## 待续工作

### ~~W-1 完整 schema 逆向~~ ✅ 已由 tc-save-lab 完成
直接采用 `tc-save-lab/codec.py` 的实现（或移植到 Rust）。

### W-2. 移植到 Rust / 集成进 Tauri app
- `binary.py` → 用 `byteorder` crate 重写（小端序 reader/writer）
- `snappy.py` → 用 `snap` crate（pure Rust Snappy）
- `model.py` → `serde` 派生 `Circuit`/`Component`/`Wire`
- 入口：`src-tauri/src/circuit/` 模块
- 共享给 CLI（`src-tauri/src/bin/tcc.rs`）

### W-3. 注入机制（D-7）—— 让游戏加载我们的电路
- ~~改 `levels.txt`~~ 已排除
- 选项：通过 `compile.dll` 直接调用
- 选项：通过游戏 UI 自动化
- 待选

### W-4. ~~写回合法性测试~~（仅在 v15 codec 移植后）
- 拿一份备份，写回原位，启动游戏加载关卡看是否接受

---

## 关键样本路径

| 关卡 | 路径 | 用途 |
|---|---|---|
| `and_gate` | `schematics/and_gate/缺省/circuit.data` (220B) | v15 最小非空 |
| `not_gate` | `schematics/not_gate/缺省/circuit.data` (189B) | 单门 1 输入 |
| `or_gate` | `schematics/or_gate/缺省/circuit.data` (254B) | 单门 2 输入 |
| `full_adder` | `schematics/full_adder/标准/circuit.data` (489B) | 多类型组件 |
| `byte_adder` | `schematics/byte_adder/Ling 8bit/circuit.data` (5439B) | 最大样本 |
| `campaign/and_gate` | `E:\...\campaign\and_gate\circuit.data` | **v13** 关卡定义 |