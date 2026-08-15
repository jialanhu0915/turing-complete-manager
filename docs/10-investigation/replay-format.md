---
title: replay.nim 格式分析
last_updated: 2026-08-15
scope: investigation
status: 已审（2026-08-15 校准 replay.nim 描述：运行时生成的仿真驱动器）
---

# `replay.nim` 格式分析

## 概要

`E:\SteamLibrary\steamapps\common\Turing Complete\replay.nim`

| 指标 | 值 |
|---|---|
| 大小 | 79,481,820 B (~79 MB) |
| 行数 | 2,271,818 |
| 编码 | UTF-8 文本（含 CRLF + LF 混合换行） |
| 类型 | Nim 源代码（**可被 `compile.dll` 编译执行**） |

> **重要认知纠正**：`replay.nim` **不是** 玩家电路的纯数据序列化文件。
> 它是**游戏内 Nim 仿真器的源代码**，每次玩家改动电路后由游戏重新生成。
> 玩家电路的实际拓扑（门的位置、连线、布局）存储在存档目录的 `circuit.data` 二进制文件中，**不在这里**。
> `replay.nim` 编码的是：**仿真驱动 + UI 状态 + 组件计数**，而不是电路连线本身。

---

## 文件结构

整个文件只有 3 种顶层语句：

1. **`import`** —— 1 行
2. **`var`/`let`/`const`** —— 类型别名 + 一段 `actions` 序列
3. **`actions.add(SimulatorRequest(...))`** —— 顶层语句，大量重复

按出现顺序：

```
行 1:    import simulator_types, native_alloc/alloc
行 2:    var actions*: seq[SimulatorRequest]
行 3..:  actions.add(SimulatorRequest(...))  ← 2,271,816 行主要是这个
```

### `actions` 序列的两类成员

通过 grep `kind: \w+`，整个文件只有 2 种 `SimulatorRequest.kind`：

| kind | 出现次数 | 作用 |
|---|---|---|
| `sim_do` | 2,617 | 发简单控制命令（mode_reset / refresh / run） |
| `compile_and_run` | 1,125 | **嵌入一段 Nim DSL 代码**，描述一次完整仿真 |

`sim_do` 是薄包装：

```nim
actions.add(SimulatorRequest(kind: sim_do, command: mode_reset, target_tick: -1))
actions.add(SimulatorRequest(kind: sim_do, command: refresh, target_tick: -1))
actions.add(SimulatorRequest(kind: sim_do, command: run, target_tick: 2))
```

`SimCommand` 命令分布（grep 统计）：

| command | 次数 | 含义 |
|---|---|---|
| `refresh` | 1,058 | 重绘 UI |
| `mode_reset` | 918 | 重置仿真器到初始状态 |
| `run` | 641 | 执行若干 tick 仿真 |

---

## `compile_and_run` 块的内部结构

每个 `compile_and_run` 块长这样：

```nim
actions.add(SimulatorRequest(
  kind: compile_and_run,
  simulation_state_length: <整数>,
  code: """
    ... 嵌入代码 ...
  """
))
```

`simulation_state_length` 是当前仿真器分配的全局内存字节数（不固定，**每个块都不同**），统计：

| length | 出现次数 |
|---|---|
| 665 | 63 |
| 541 | 61 |
| 551 | 55 |
| 549 | 38 |
| 545 | 36 |
| 553 | 33 |
| 455 | 33 |
| 543 | 31 |
| 459 | 30 |
| 446 | 29 |

最大 ~1,125 个不同值，最小 ~267。

### 嵌入代码：自定义 Nim DSL

`code: """..."""` 里是一段**自定义 Nim 子集 DSL**，包含关键字：
`type`、`def`、`var`、`let`、`const`、`store`、`load`、`switch`、`while`、`return`、`break`、`if`。

每个 `compile_and_run` 块的嵌入代码结构都是相同的模板，差异在 `#COUNTS` 数组与 `ui_set_*` 调用：

```nim
# 类型别名（每个块都一样）
type SimCommand Enum[run, refresh, mode_reset, quit_simulation]
type CommandIndex Enum[ctl_command, ctl_command_id, ...]
type StateIndex Enum[sim_tick, sim_target_tick, sim_test_result, ...]
type TestResult Enum[pass, win, fail]
type ComponentType Enum[com_off, com_on, com_not_bit, ...]  # 101 项

# 内存映射（每个块都一样）—— 通过 Ptr 拿游戏状态
var commands         = Ptr """ & $simulation_commands & """
var settings         = Ptr """ & $simulation_settings & """
var input_replay     = [U64] """ & $simulation_input_replay & """
var output_history_pins = Ptr """ & $simulation_output_history_pins & """
var error_buffer     = Ptr """ & $simulation_error_buffer & """
var ui_buffer        = Ptr """ & $simulation_ui_buffer & """
const #SIMULATION_STATE = Ptr """ & $simulation_state & """
const #SIMULATION_KEYBOARD_CHARACTER = Ptr """ & $simulation_keyboard_character & """
const #SIMULATION_KEYBOARD_COORDINATE = Ptr """ & $simulation_keyboard_coordinate & """

# 辅助函数（每个块都一样）
def get_command(idx: CommandIndex) U64 { return load(<U64>, .commands + (Int idx) * 8) }
def get_setting(idx: StateIndex) U64 { return load(<U64>, .settings + (Int idx) * 8) }
def set_setting(idx: StateIndex, value: U64) { store(.settings + (Int idx) * 8, value) }
def set_text(text: String, offset: Int) { ... }

# ⭐ 每块差异：
# 1. #COUNTS：当前电路的 101 个组件用量数组（按 ComponentType 枚举顺序）
const #COUNTS = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, ...]   # 长度 = 101

# 2. get_component_count：返回某个组件的数量
def get_component_count() Int { return 4 }
def get_component_count(component_type: ComponentType) Int {
    const #COUNTS = [...]
    return #COUNTS[Int component_type]
}

# 3. UI 状态：哪些表格行列隐藏、位置、宽度
def ui_set_position(id: String, x: Int, y: Int) { ... }
def ui_set_width(id: String, value: Int) { ... }
def ui_set_hidden(id: String, value: Bool) {
    switch id
        "table[0][0]" { store(.ui_buffer + 88, U64 value) }
        "table[0][1]" { store(.ui_buffer + 96, U64 value) }
        ...
}

# 4. 主仿真循环（每个块都一样）
def run_sim() {
    while true
        switch get_command(ctl_command)
            run { ... }
            refresh { ... }
            mode_reset { ... }
            quit_simulation { break run_sim }
}
```

### 关键发现：每次 `compile_and_run` 都重新生成

```
★ 每次玩家改动电路（拖一个门、改一条线、跑一次仿真），
  游戏就重新生成整个文件，写盘。
```

这就是为什么这个文件能长到 79 MB / 227 万行：
- 1,125 次 `compile_and_run` 块 × 每个块 ~2,000 行 = 227 万行
- 每次玩家操作都新增一个块
- 整个文件是**一长串游戏操作的可重放脚本**

---

## 嵌入字符串 `Ptr """..."""` 是什么？

这些看起来很神秘的 `Ptr """ & $simulation_commands & """` 实际是 **Nim 字符串字面量 + 字符串内插**，生成的代码类似：

```nim
var commands = Ptr """abc123_xyz"""
```

Nim 解析后，`Ptr "..."` 实际变成一个指向字符串字面量的指针。

`$simulation_commands` 这种 `$` 表达式是 **Nim 的字符串内插运算符**——把变量值转成字符串嵌入字面量。

### 生成器推测

这是 Nim 模板（template）在运行时构造另一个 Nim 源文件，再用 `compile.dll` 编译执行。

伪代码：

```nim
# 游戏内 Nim 模板（推测）
template generateReplay(commands, settings, inputReplay, ...) =
  result.add &"""
actions.add(SimulatorRequest(
  kind: compile_and_run,
  simulation_state_length: ${commands.len},
  code: """
var commands = Ptr "${commands}"
var settings = Ptr "${settings}"
...
""")
"""
```

也就是说：**`replay.nim` 是游戏在每次玩家操作后，由一个 Nim template 字符串拼接生成的**。整个 79 MB 的文件本质上是「player action log as Nim source」。

---

## 结论与对后续工作的影响

### 结论

- `replay.nim` 是 **可重放的 Nim 源**，不是纯数据
- **电路拓扑不在这**——在 `circuit.data`
- 文件快速膨胀（玩家每操作一次都新增一个 `compile_and_run` 块）
- 每个 `compile_and_run` 块内部 90% 是固定模板，差异在 `#COUNTS` 与 `ui_*` 调用

### 对「LLM 优化电路」的影响

如果未来要做 LLM 电路优化，正确的路径不是去解析 `replay.nim`，而是：

1. **从 `circuit.data` 提取电路拓扑** —— 需要先逆向二进制格式
2. **让 LLM 生成新的 `circuit.data`** —— 或在游戏 UI 自动化操作生成
3. **驱动游戏跑仿真并读 `sim_test_result`** —— 可以通过 `compile.dll` 模拟、或操作游戏 UI

`replay.nim` 本身只是「运行时生成的仿真驱动器」式的中间产物，不直接服务优化目标。

### 仍待回答

- `circuit.data` 二进制格式（与 `replay.nim` 不同）
- `campaign/<level_id>/` 关卡定义的格式
- `#COUNTS` 数组与 `ui_*` 调用的精确语义（哪些 ui_buffer offset 对应哪些 UI 元素）

这些都需要后续工作。