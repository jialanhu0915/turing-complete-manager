---
title: 架构关卡（Architecture Levels）
last_updated: 2026-08-11
scope: investigation
status: 已审（2026-08-11 重写——实测 11 个架构 kind 关卡的 test.si / meta.txt / 完整文件清单；纠正 §一/§二/§三 全部错误；保留 §四 Spec.isa 细节并补 Overture 实测样本）
---

# 架构关卡（Architecture Levels）

> **实测范围**：11 个 `kind=architecture` 关卡（binary_search / capitalize / circumference / conditional_jumps / maze / mod_4 / nim / rng / sort / tower / sandbox）+ 1 个 `kind=sequential` 特例 `assembly_programming`。
>
> **纠正要点**：2026-08-10 初稿含 4 处关键错误——
>
> 1. §一 API 名称错：写的 `arch_check_output` / `arch_get_input` **不存在**；实际是 `check_output(_switched)` / `get_input(_switched)`
> 2. §二 文件路径错：`schematics/architecture/<level>/<scheme>/*` 是**关卡玩家存档**，不是 CPU 架构；CPU 在 `schematics/architecture/<arch>/{circuit.data, spec.isa}`
> 3. §三 字节级协议误解：`Int` 是 API 风格差异，**不是**字节级编码——CPU 的 IO pin 数 = 数据宽度（通常 8 位）
> 4. §一 `Overture` 描述错：是 **Stuffe 提供的预填玩家存档模板**（首次启动复制到 `%APPDATA%`），不是玩家搭的 CPU

---

## 一、关键分离：关卡定义 ≠ 玩家 CPU ≠ 玩家存档

三种东西**完全不同**——路径、内容、谁创建都不同：

| 概念 | 谁创建 | 内容 | 路径 |
|---|---|---|---|
| **关卡定义** | **Stuffe** | 题目契约：默认电路布局 + meta + ui + test.si + （可选）hint | `campaign/<level_id>/*` |
| **玩家 CPU 架构** | **Stuffe 提供模板 / 玩家自建** | CPU 电路 + ISA 声明 + 各关卡的程序 | `schematics/architecture/<arch_name>/*` |
| **玩家关卡存档** | **玩家** | 玩家搭的方案（用基础组件或自搭 CPU） | `schematics/architecture/<level_id>/<scheme>/circuit.data` |

> ⚠️ **常见混淆**：`schematics/architecture/` 下既有 `<arch_name>/`（CPU），也有 `<level_id>/<scheme>/`（关卡存档）——**两者共用同一父目录，但语义完全不同**。判断方式：`<X>/circuit.data` + `<X>/spec.isa` + `<level>/new_program.asm` → CPU；`<X>/<scheme>/circuit.data` → 关卡存档。

### 1.1 关卡定义实测清单（`campaign/<level>/*`，2026-08-11）

| 关卡 | circuit.data | meta.txt | test.si | ui.txt | label.txt | *.png | default.isa | new_program.asm | hint | 默认 CPU |
|---|---|---|---|---|---|---|---|---|---|---|
| `binary_search` | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | Overture |
| `capitalize` | ✅ | ✅ | ✅ | ✅ | ❌ | pointer.png | ❌ | ❌ | ❌ | Symphony |
| `circumference` | ✅ | ✅ | ✅ | ✅ | ? | ❌ | ❌ | ❌ | ❌ | Overture |
| `conditional_jumps` | ✅ | ✅ | ✅ | ✅ | ? | ❌ | ❌ | ✅ | ❌ | Overture |
| `maze` | ✅ | ✅ | ✅ | ✅ | ? | 多个（robot / looking_at / maze_0..7） | ❌ | ❌ | hint_0.txt | Overture |
| `mod_4` | ✅ | ✅ | ✅ | ❌ | ? | ❌ | ❌ | ❌ | hint_0.txt | Overture |
| `nim` | ✅ | ✅ | ✅ | ✅ | ? | 0..12.png | ❌ | ❌ | ❌ | Symphony |
| `rng` | ✅ | ✅ | ✅ | ✅ | ? | ❌ | ❌ | ❌ | ❌ | Symphony |
| `sort` | ✅ | ✅ | ✅ | ✅ | ? | 0..48.png | ❌ | ❌ | ❌ | Symphony |
| `tower` | ✅ | ✅ | ✅ | ✅ | ? | 0..4.png + magnet/peg/min | ❌ | ❌ | ❌ | Symphony |
| `sandbox` | ❌ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | (无，自由) |
| **`assembly_programming`** | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ✅ | ✅ | ❌ | Overture（immutable_isa） |

> **关键观察**：
>
> - **所有 11 个架构 kind 关卡**都有 `circuit.data` + `meta.txt` + `test.si`（`sandbox` 例外：无 `circuit.data`）
> - **`default_architecture = "<X>"`** 字段存在于 10 个关卡（`sandbox` 无）；`assembly_programming` 是 sequential kind，**没有**该字段（但有 `copy_solution_to_architecture = "Overture"` + `immutable_isa = true`）
> - **`new_program.asm` 是关卡可选的"起始程序"**——只有 `conditional_jumps` 和 `assembly_programming` 两个关卡有；其他 9 个架构关需要玩家**从零写**程序
> - **`default.isa` 仅 `assembly_programming` 有**——它是 ISA 教学关，需要先有 ISA 才能写程序
> - `mod_4` 和 `maze` 有 `hint_0.txt`（文字提示，无配套 `.data`）——非标准 hint 形态

### 1.2 Stuffe 提供的 CPU 模板（玩家存档预填）

| CPU 模板 | 引用方 | 路径 |
|---|---|---|
| **Overture** | binary_search / circumference / conditional_jumps / maze / mod_4 + assembly_programming 的 `copy_solution_to_architecture` | `%APPDATA%\Turing Complete\schematics\architecture\Overture\{circuit.data, spec.isa, <level>/new_program.asm}` |
| **Symphony** | capitalize / nim / rng / sort / tower（更复杂的关卡） | `%APPDATA%\Turing Complete\schematics\architecture\Symphony\{circuit.data, spec.isa, <level>/new_program.asm}` |

> 这两个 CPU 由 **Stuffe 写好**，**首次启动游戏时复制到玩家存档目录**（不在 game install 下）。玩家可以直接用、改、或者从零搭自己的。**实测 `Symphony/` 下还有 `5281115256352676631.bin`——未知文件，待逆。**

### 1.3 `assembly_programming` 的特殊性

`assembly_programming` 是 `kind=sequential`（不是 architecture），但承担"教玩家搭 ISA"的入门角色：

```ini
kind = sequential
tests = 251
size = 128
copy_solution_to_architecture = "Overture"   # 玩家完成后 ISA 复制到 Overture
unlocks_pages = ["Assembly/Language creation", "Assembly/Assembly programming"]
immutable_isa = true                           # 不能编辑已有 Overture ISA
```

+ `default.isa`（ISA 教学起点）+ `new_program.asm`（教学程序）

它是关卡 → CPU 的"承上启下"：玩家在此搭出自己的 ISA + 电路 + 程序后，ISA 通过 `copy_solution_to_architecture` 复制到 `schematics/architecture/Overture/spec.isa`，后续架构关就用这个 ISA。

---

## 二、API 实测（test.si 全 11 关读取）

### 2.1 两种 API 风格

| 模式 | 函数签名 | 使用关卡（11 个架构 kind 中） |
|---|---|---|
| **`_switched` 变体**（架构关默认） | `def get_input_switched() Int` + `def check_output_switched(output: Int) TestResult` | binary_search / circumference / conditional_jumps / maze / mod_4 / nim / rng / sort / tower / sandbox（**10/11**） |
| **标准组件 API** | `def get_input(cycle: Int) Input` + `def check_output(cycle: Int, input: Input, output: Output) TestResult` | **capitalize**（1/11） |

> ⚠️ **命名纠正**：之前文档写的 `arch_check_output` / `arch_get_input` **不存在**。真实函数名是 `get_input` / `check_output`（标准）或 `get_input_switched` / `check_output_switched`（架构）。

### 2.2 `_switched` 模式的语义（实测 10 关）

从 `sandbox` / `binary_search` / `circumference` / `mod_4` / `rng` / `sort` / `maze` / `nim` / `tower` / `conditional_jumps` 10 关实测：

- **`get_input_switched() Int`**：返回下一个输入值（`Int` 类型，CPU 的输入位宽由 IO pin 数决定，通常 8 位）。**不带 `cycle` 参数**——由游戏驱动器内部维护状态机
- **`check_output_switched(output: Int) TestResult`**：检查玩家的 CPU 输出（一个 `Int`）。返回 `pass` / `win` / `fail`
- **每次调用对应 CPU 的一个 IO 周期**：CPU 读输入（→ `get_input_switched`）、计算、写输出（→ `check_output_switched`）
- **`Int` 是 Simplex DSL 的整数类型**——不限 8 位。CPU 的 IO 接口宽度由 CPU 的 IO pin 数决定（玩家自搭 CPU 可任意，但主流 ISA 用 8 位）

### 2.3 标准 API 实测（capitalize）

```simplex
var input = U8 ((get_test() * 207 + 168) % 251)

def get_input(cycle: Int) Input {
    return Input {input: .input}
}

def check_output(cycle: Int, input: Input, output: Output) TestResult {
    let actual_counter = get_output("Count", 0)
    let instruction = get_ram_value("Program", actual_counter, <U8>)
    ui_set_instruction(0, Int instruction, cycle)

    if !output.output_enabled { return pass }
    if input.input + 5 != output.output {
        ...
        return fail
    }
    return win
}
```

- `Input` / `Output` 是带类型的结构体；`Output.output_enabled`（输出是否有效）+ `Output.output`（数值）
- `cycle: Int` 是 tick 计数（架构关通常不用，但 capitalize 是混合形态）
- **capitalize 的 API 与组件关完全一致**——它的 `kind=architecture` 是因为默认用 Overture 解，但 IO 模型是组件式

### 2.4 **没有"字节级协议"**

之前文档写"`arch_check_output` 用 `Int` 而非结构体，标量到位的拆分方式未实测"——**这是误解**。

- `_switched` 模式的"标量 vs 结构体"差异**就是 API 风格差异**，不是字节级编码
- `Int` 是 Simplex DSL 的普通整数类型，宽度由 CPU 的 IO pin 数决定
- **玩家自搭 CPU 可以用任意位数的 IO**——这是 ISA 设计的一部分（Overture 8 位，Symphony 可能更宽）
- `_switched` 不是"位流打包"，只是"用整数传 IO 而不是用结构体"

---

## 三、Spec.isa 文件结构

`*.isa` 是**声明式 ISA 规范**（声明式 DSL，描述指令格式与编码）。

### 3.1 实测样本：`Overture/spec.isa`

```
[settings]
name = "Overture"

[fields]

register
r0 000
r1 001
r2 010
r3 011
r4 100
r5 101

in_register
in 110

out_register
out 110

[instructions]

mov %a(register | out_register), %b(register | in_register)
10aaabbb
# Moves a value from %b to %a.

imm %a:U8(immediate | label)
00aaaaaa
# 将固定值 %a 写入寄存器 r0。

nand
01000000
# 对寄存器 r1 和 r2 的值执行按位与非运算，并将结果存入寄存器 r3。

...

jmp
11000001
# 无条件跳转至寄存器 r0 的值对应的地址。

jz
11000010
# 当寄存器 r3 的值等于 0 时，跳转至寄存器 r0 的值对应的地址。
```

**观察**：

- `[settings]` 段极简——只有 `name`
- `[fields]` 段定义**操作数类型**（不是字段）：
  - `register` 段：`r0`..`r5` 6 个通用寄存器（3 位编码）
  - `in_register` 段：`in` 特殊寄存器（CPU 输入端口，3 位编码）
  - `out_register` 段：`out` 特殊寄存器（CPU 输出端口，3 位编码）
- `[instructions]` 段每条指令 3 行：
  1. 汇编格式（`mov %a(...), %b(...)`）
  2. 8 位编码（`10aaabbb`，`a` 和 `b` 是字段引用）
  3. 注释（以 `#` 开头，**简体中文**）
- 操作数通过 `|` 列举可接受的类型（`register | out_register` 表示 `%a` 可以是通用寄存器或输出寄存器）
- 指令格式：操作码（`10` / `00` / `01` / `11`）前缀 + 字段位

### 3.2 isa_spec 校对（Stuffe/isa_spec，MIT）

[github.com/Stuffe/isa_spec](https://github.com/Stuffe/isa_spec)（Nim + Assembly，**MIT**）—— 配套 ISA 规范处理器实现。本地 clone 在 `reference/isa_spec/`。

**比 Overture 实测多出的字段**（isa_spec 完整支持，Overture 用不到）：

| 字段 | 说明 | Overture 是否用 |
|---|---|---|
| `line_comments` / `block_comments` | 注释符号 | 用 `#` 单行（Overture 未声明，默认 `#`） |
| `endianness` (`end_big` / `end_little`) | 字节序 | 未声明（Overture 是位级 ISA，无字节序） |
| `code_alignment` | 指令对齐字节数 | 未声明 |
| `patterns` / `instruction_decoders` | 解码器声明 | 未声明 |

**完整操作数类型范围**（isa_spec 全集，Overture 用其中一小部分）：

- **`FieldKind`**: 126 个 var 槽（`fk_var_0`..`fk_var_125`）+ `fk_label` + `fk_imm_0` + 1..64 位有/无符号立即数
- **`BitFieldKind`**: 252 个 var 槽 + `bfk_zero` / `bfk_one` / `bfk_wildcard` / `bfk_invalid`
- **`SyntaxKind`**: `sk_fixed` / `sk_field` / `sk_pattern` / **`sk_any_number_of_spaces`** / **`sk_at_least_one_space`**——空格是通配
- **`InstructionUnbranched`** 支持**分块条件指令**（`chunks: seq[InstructionBranch]`，每个 chunk 有自己的位模式 + 条件表达式）—— `debranch()` 生成所有组合
- **`OperandType`**: `otk_normal`（普通字段引用）/ `otk_virtual`（运行时计算的虚拟操作数）/ `otk_pattern`（参数化模式）

权威参考：
- `reference/isa_spec/README.md`（89 行）—— BNF
- `reference/isa_spec/types.nim`（502 行）—— 完整数据模型

---

## 四、编译路径

| 关卡类型 | 编译路径 |
|---|---|
| **组件关** | `circuit.data`（v15 玩家存档 / v13/v14 关卡定义） → 运行时 → `compile.dll`（Nim DSL → 机器码） |
| **架构关** | `circuit.data`（CPU 电路，v15）+ `spec.isa`（ISA 声明）+ `new_program.asm`（程序） → 运行时 → `compile.dll`（汇编 + 编译） |

**架构关的编译路径未完全逆向**——可能复用 `compile.dll`（额外加载 `.isa` 后处理 `.asm`），也可能有独立汇编器。**实测**：Overture CPU 有 16 条指令（mov / imm / nand / or / and / nor / add / sub / nop / jmp / jz / jnz / jl / jge / jle / jg），复杂到需要独立汇编器（不是简单把 ASM 文本替换成 8 位 bit 串）。

---

## 五、对项目的影响

| 影响点 | 状态 |
|---|---|
| 电路优化 | 本项目不优化架构关卡（成本不划算，玩家也少） |
| 备份/恢复 | `.isa` 必须包含（已记入 `level-data.md` §1.3） |
| **M7 自制关卡工具** | **不需要处理 CPU**——CPU 和 ISA 都在玩家存档，不属于关卡定义。zip 只含 `campaign/<level>/*` |
| **关卡合法性验证** | `test.si` 必须用 compile.dll 编译通过（不论 `_switched` 还是标准 API） |
| `_switched` vs 标准 | 都是合法 API，导入工具不做区分——编译通过即可 |

---

## 六、待续工作（更新）

| 编号 | 内容 | 状态 |
|---|---|---|
| ~~W-3~~ | ~~抓 `arch_check_output` 在游戏可执行文件中的实现~~ | **取消**——函数名错了，正确名称是 `check_output_switched`；且无字节级协议 |
| ~~W-5~~ | ~~抓取 `Component/Architecture` 系列 wiki 页扩充架构组件目录~~ | **取消**——玩家不创建架构组件（玩家 CPU 是基础组件拼出来的） |
| **W-1a** | 解析 Overture `spec.isa` 的 16 条指令（实测已读文件，未结构化） | 待续 |
| **W-1b** | 解析 `new_program.asm` 的汇编格式（从 Overture 已存档程序实测） | 待续 |
| **W-4** | `circuit.data` 中 `selected_programs` 字段——v15 codec 已读写（test/verify-cli），但二元组 `(string, string)` 的具体语义（绝对路径？相对路径？basename？）待 M7 工具实测 | 待续 |
| **W-6** | Symphony 的 `5281115256352676631.bin` 未知文件——可能是 ISA 编译缓存？ | 待续 |
| **W-7** | M7 自制关卡工具：合法性检查 #10（test.si 编译通过）需实测 `_switched` 与标准 API 是否都通过现有 compile.dll | 待续 |