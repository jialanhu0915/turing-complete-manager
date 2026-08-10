---
title: 命令与状态枚举
last_updated: 2026-08-10
scope: investigation
status: 已审（2026-08-10 补充 Simplex 端 API wiki 校对）
---

# 命令与状态枚举

`replay.nim` 嵌入代码中用到的所有枚举类型（除 ComponentType 已在 component-catalog.md 列出）。

---

## SimulatorRequest

顶层 `actions` 序列的元素类型。

```nim
type SimulatorRequest = object
    case kind: SimulatorRequestKind
    of sim_do:
        command: SimCommand
        target_tick: int
    of compile_and_run:
        simulation_state_length: int
        code: string
```

| 字段 | `kind` | 类型 | 说明 |
|---|---|---|---|
| `kind` | 通用 | `SimulatorRequestKind` | `sim_do` 或 `compile_and_run` |
| `command` | `sim_do` | `SimCommand` | 简单控制命令 |
| `target_tick` | `sim_do` | `int` | 目标 tick（`-1` 表示无限） |
| `simulation_state_length` | `compile_and_run` | `int` | 本次仿真分配的全局内存字节数 |
| `code` | `compile_and_run` | `string` | 嵌入的 Nim DSL 源码 |

> 实际 Nim 代码是用 variant object（case）写的；以上是语义化描述。

### 出现次数（grep 统计）

| kind | 次数 | 占比 |
|---|---|---|
| `sim_do` | 2,617 | 70% |
| `compile_and_run` | 1,125 | 30% |

---

## SimCommand

`sim_do` 类型请求的命令字段。

```nim
type SimCommand Enum[run, refresh, mode_reset, quit_simulation]
```

| 名称 | 含义 |
|---|---|
| `run` | 启动仿真 tick 到 `target_tick` |
| `refresh` | 仅重绘 UI，不推进仿真 |
| `mode_reset` | 重置仿真器到初始状态（清 input_replay、tick 归零） |
| `quit_simulation` | 退出仿真循环 |

### 出现次数

| command | 次数 |
|---|---|
| `refresh` | 1,058 |
| `mode_reset` | 918 |
| `run` | 641 |

> `quit_simulation` 仅在嵌入代码的 `run_sim` 函数体内出现，控制流用，未出现在顶层 actions。

---

## CommandIndex

仿真器控制命令内存区的索引。`get_command(idx)` 用此访问控制字。

```nim
type CommandIndex Enum[
    ctl_command,
    ctl_command_id,
    ctl_tick_speed_ms,
    ctl_exit,
    ctl_level_manual_input,
    ctl_level_manual_input_id,
    ctl_test,
]
```

| 名称 | 含义 |
|---|---|
| `ctl_command` | 当前命令槽（`SimCommand` 编码值） |
| `ctl_command_id` | 命令 ID（自增计数器） |
| `ctl_tick_speed_ms` | tick 间隔（毫秒） |
| `ctl_exit` | 退出标志 |
| `ctl_level_manual_input` | 关卡手动输入值（uint64） |
| `ctl_level_manual_input_id` | 关卡手动输入 ID |
| `ctl_test` | 测试触发（写 1 触发一次验证） |

每个槽占 8 字节（U64），通过 `load(<U64>, .commands + (Int idx) * 8)` 访问。

---

## StateIndex

仿真器状态内存区的索引。`get_setting(idx)` / `set_setting(idx, value)` 用此访问。

```nim
type StateIndex Enum[
    sim_tick,
    sim_target_tick,
    sim_test_result,
    sim_last_command_id,
    sim_error_component,
    sim_short_circuit_component_id_1,
    sim_short_circuit_component_id_2,
    sim_short_circuit_pin_1,
    sim_short_circuit_pin_2,
    sim_short_circuit_top_level_permanent_id_1,
    sim_short_circuit_top_level_permanent_id_2,
    sim_short_circuit_value_1,
    sim_short_circuit_value_2,
    sim_short_circuit_any_top_level_wire_id,
    sim_running,
]
```

### 一般状态（5 项）

| 名称 | 含义 |
|---|---|
| `sim_tick` | 当前已执行 tick 数 |
| `sim_target_tick` | 目标 tick（`sim_do.run` 时设置） |
| `sim_test_result` | `TestResult` 编码值（pass/win/fail） |
| `sim_last_command_id` | 最后处理的命令 ID |
| `sim_error_component` | 错误发生的组件 ID（运行时报错时填充） |
| `sim_running` | 仿真运行中标志 |

### 短路诊断（9 项）

当电路发生短路（输出 pin 被多个驱动源同时拉高/低）时填充：

| 名称 | 含义 |
|---|---|
| `sim_short_circuit_component_id_1` | 短路组件 1 ID |
| `sim_short_circuit_component_id_2` | 短路组件 2 ID |
| `sim_short_circuit_pin_1` | 短路 pin 1 |
| `sim_short_circuit_pin_2` | 短路 pin 2 |
| `sim_short_circuit_top_level_permanent_id_1` | 短路顶层组件永久 ID 1 |
| `sim_short_circuit_top_level_permanent_id_2` | 短路顶层组件永久 ID 2 |
| `sim_short_circuit_value_1` | 短路时的值 1 |
| `sim_short_circuit_value_2` | 短路时的值 2 |
| `sim_short_circuit_any_top_level_wire_id` | 任一顶层 wire ID（短路导线 ID） |

> 这 9 个槽是短路诊断的关键字段，对调试 LLM 生成的电路很有价值。

---

## TestResult

仿真完成后判定测试是否通过。

```nim
type TestResult Enum[pass, win, fail]
```

| 名称 | 含义 |
|---|---|
| `pass` | 测试通过（输出与期望一致） |
| `win` | 关卡通关（隐含 pass，通常带额外要求如 tick 数 < 阈值） |
| `fail` | 测试未通过 |

> `sim_test_result` 槽写入此枚举的编码值。

---

## 内存布局总结

```
┌─────────────────────────────────────────┐
│  commands (Ctl 区)                      │  U64 × 7（按 CommandIndex 顺序）
│  每槽 8 字节，总 56 字节                │
├─────────────────────────────────────────┤
│  settings (State 区)                    │  U64 × 15（按 StateIndex 顺序）
│  每槽 8 字节，总 120 字节               │
├─────────────────────────────────────────┤
│  input_replay (测试输入数组)            │  U64 × 1024
│  总 8,192 字节                          │
├─────────────────────────────────────────┤
│  output_history_pins (输出引脚历史)     │  由 simulation_state_length 决定
├─────────────────────────────────────────┤
│  error_buffer (错误消息缓冲)            │  同上
├─────────────────────────────────────────┤
│  ui_buffer (UI 状态缓冲)                │  同上
└─────────────────────────────────────────┘
         ↑
   simulation_state_length = commands + settings + input_replay + output + error + ui
   （每个 compile_and_run 块独立分配，块间数值不同）
```

---

## 应用场景

### 读取测试结果（LLM 验证电路用）

```text
1. 触发 `ctl_test` = 1
2. 等若干 tick
3. 读 `sim_test_result` → 0=pass / 1=win / 2=fail
4. 若 fail → 读 `sim_error_component` + `sim_short_circuit_*` 定位错误
```

### 触发单步仿真

```text
1. 写 `ctl_command` = `run` 的枚举值
2. 写 `ctl_command_id` = 当前 ID
3. 写 `sim_target_tick` = N
4. 游戏主循环读到 `ctl_command` 后推进 N tick
5. 完成后回写 `sim_test_result`
```

---

## Simplex 端 API（wiki 校对 2026-08-10）

> Wiki 来源：`turingcomplete.wiki/wiki/Custom_level_creation/test.si`（CC BY-SA 4.0）。
> 以下是 `test.si` 暴露给关卡作者的 Simplex 语言 API 表。
> 仅记录我们现有 docs 未覆盖或与现有结论交叉的项。

### 钩子（可选函数，组件关卡用）

| 函数 | 触发时机 | 用途 |
|---|---|---|
| `on_ui_update(input, output)` | sim 运行时约 30 次/秒 | UI 实时刷新（玩家可见控件） |
| `on_manual_input(input: Int)` | 玩家操作键盘时 | 键盘输入处理；配合 `add_keyboard_value` |

### 评分与计数

| 函数 | 返回 |
|---|---|
| `get_delay_score() Int` | 关卡 delay 总分 |
| `get_gate_score() Int` | 关卡 gate 总分 |
| `get_component_count() Int` | 组件总数 |
| `get_component_count(component_type: ComponentType) Int` | 按类型计数（类型用 `com_*` 内部名） |

### 状态查询

| 函数 | 对应内存 | 说明 |
|---|---|---|
| `get_tick() Int` | `settings[sim_tick]` | 当前 tick |
| `get_test() Int` | — | 当前测试编号 |
| `get_last_time() Int` | — | 最后一次执行时间 |

### 内存访问（RAM）

| 函数 | 用途 |
|---|---|
| `get_memory(label: String) Int` | 按 label 取 RAM 基址 |
| `get_ram_value(label: String, address: Int, size: @Size) @Size` | 读 RAM 值（按位宽 `@Size`） |
| `set_ram_value(label: String, offset: Int, value: @Type)` | 写 RAM 值 |

### 键盘事件注入

| 函数 | 用途 |
|---|---|
| `add_keyboard_value(key_down: Bool, value: U8)` | 推键盘事件进队列 |

### UI 与错误

| 函数 | 用途 |
|---|---|
| `set_error(input: String)` | 显示黄色错误消息 |
| `ui_set_*`（多个） | UI 自定义（具体名称 wiki 未列全） |

### 架构关卡独立 API（盲区）

| 函数 | 替代 |
|---|---|
| `arch_check_output(test: Int, input: Int, output: Int) TestResult` | 替代 `check_output`（组件关卡） |
| `arch_get_input(test: Int) Int` | 替代 `get_input`（组件关卡） |

签名形态不同（用 `Int` 而非结构体），多一个 `test` 参数。详见 [`architecture-levels.md`](architecture-levels.md)。

### 类型 → 字段映射规则

- **Input**：每个输入引脚一个字段；**非字母数字字符替换为下划线**（如 `Carry in` → `carry_in`）
- **Output**：每个输出引脚**占两个字段**（值 + `_is_z` Z 状态标记）—— 详见 [`compile-signature.md`](compile-signature.md) §test.si API 校对