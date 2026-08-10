---
title: compile.dll::compile 函数完整签名（2026-08-08 反汇编）
last_updated: 2026-08-10
scope: investigation
status: 已完成 Phase A（2026-08-10 补充 test.si wiki 校对）
---

# compile.dll::compile 函数完整签名

## 概要

通过 `objdump -d --disassemble=compile compile.dll` 完整反汇编 `compile` 导出函数（RVA `0x000f4370`），确认 ABI 形态。

**导出函数清单**（与 dll-analysis.md 一致，仅 3 个）：
- `NimMain` @ `0x000f4dd0`
- `NimDestroyGlobals` @ `0x000f47e0`
- `compile` @ `0x000f4370`

---

## 签名（Microsoft x64 调用约定）

```c
void compile(
    void*   out_buf,    // rcx:  40 字节输出结构体（由 caller 预分配）
    void*   src_str,    // rdx:  Nim string = { int64 length, ptr char[8..] }
    int32_t mode,       // r8:   仅低 32 位使用；语义待定（推测 = 控制位）
    int32_t flags       // r9:   仅低 32 位使用；语义待定
);
```

**关键证据**（prologue 反汇编片段）：

```asm
2fdde4370 <compile>:
   2fdde4370: push   %r15, %r14, %r13, %r12, %rbp, %rdi, %rsi, %rbx
   2fdde437c: sub    $0x118,%rsp                ; 280 字节栈帧
   2fdde4383: movaps %xmm6,0x100(%rsp)
   2fdde438b: pxor   %xmm0,%xmm0
   2fdde438f: mov    (%rdx),%rbp                ; rbp = src_str->length
   2fdde4392: mov    0x8(%rdx),%r14             ; r14 = src_str->ptr
   2fdde4396: lea    0x6ae1(%rbp),%rdx          ; rdx = src_str->length + 0x6AE1
   2fdde439d: mov    %rcx,%rbx                  ; rbx = out_buf（保留到 epilogue）
   2fdde43a0: lea    0x40(%rsp),%rcx
   2fdde43a5: mov    %r8d,%r12d                 ; 保存 mode（低 32 位）
   ...
   2fdde442f: test   %rbp,%rbp                  ; if (length <= 0) skip copy
   2fdde4432: jle    2fdde4451
   2fdde4434: lea    0x6ae9(%r13,%rsi,1),%rcx   ; dst = new_str_data + new_len + 0x6AE9
   2fdde443c: lea    0x8(%r14),%rdx             ; src = src_str->ptr + 8 (skip len header)
   2fdde4440: mov    %rbp,%r8                   ; copy size = src_str->length
   2fdde4443: add    %rbp,%r15                  ; new_str end += length
   2fdde4446: call   2fddeb8b8 <memcpy>
```

---

## 关键发现：标准库前缀

`compile` 内部会**预分配一个长度 = `0x6AE1 + src_str->length + 8` 的新字符串**：
- 先 `memcpy` 写入 **27361 字节（0x6AE1）的标准库前缀**（地址 `TM__AoKvN7TZPdpu9adcOAgv2Lw_2+0x8`，Nim 模板 mangled 名）
- 再 `memcpy` 追加用户源码

**含义**：`compile.dll` 是**自包含的 Nim DSL 编译器**——它把 Nim DSL 标准库 + 用户源码一起编译成机器码。**用户无需提供任何运行时环境**（除 `NimMain()` 初始化）。

---

## 输出结构体（40 字节）

`compile` 在 epilogue 时把 5 个字段写到 `out_buf`：

```asm
2fdde458c: mov    %r14,0xd0(%rsp)
2fdde4594: mov    %rbx,%rax                  ; rax = out_buf
2fdde4597: mov    %r12,0xd8(%rsp)
2fdde459f: movdqa 0xd0(%rsp),%xmm1           ; xmm1 = {r14, r12}
2fdde45a8: mov    %r13d,0xe0(%rsp)
2fdde45b0: mov    %r15,0xe8(%rsp)
2fdde45b8: movdqa 0xe0(%rsp),%xmm2           ; xmm2 = {r13d, r15}
2fdde45c1: mov    %rbp,0x20(%rbx)             ; *(out_buf+0x20) = rbp
2fdde45c5: movups %xmm1,(%rbx)                ; *(out_buf+0x00) = {r14, r12}
2fdde45c8: movups %xmm2,0x10(%rbx)            ; *(out_buf+0x10) = {r13d, r15}
2fdde45cc: ... 恢复寄存器
2fdde45e7: ret
```

| Offset | Field | 来源寄存器 | **实测语义（2026-08-08 and_gate 确认）** |
|---|---|---|---|
| `0x00` | `field_0` | `r14` | **machine_code 长度**（279586） |
| `0x08` | `field_1` | `r12` | **machine_code 数据指针**（compile.dll 堆内；**+8 才是字节**，Nim string payload 的 cap 头） |
| `0x10` | `field_2` | `r13d` | **入口偏移**（entry = 拷贝基址 + 14132） |
| `0x18` | `field_3` | `r15` | **状态**：0 = 成功，13 = 编译器错误 |
| `0x20` | `field_4` | `rbp` | 成功时 0；错误时 = 错误消息指针 |

⚠️ 早期文档写"field_4 = JIT fn ptr"是**错的**（被 epilogue 寄存器 rbp 误导）。真正模式见下文"JIT 调用约定（已破解）"。实测成功输出：`field_0=279586, field_1=堆指针, field_2=14132, field_3=0, field_4=0`。

---

## 内部编译管线

`compile` 内部调用链（从反汇编 + 模块符号推断）：

```
compile (rcx=out, rdx=src_str, r8=mode, r9=flags)
  │
  ├─ 前缀拼接：新字符串 = TM_PREFIX (27361B) + 用户源码
  │
  └─ compile_source__OOZcore_u144(rcx=ctx_stack, rdx=ctx__compile_u54,
                                   r8=新字符串, r9=3)
       │
       ├─ reset_globals__OOZtypes_u34605
       ├─ prepareAdd + memcpy → 写入全局 source_buffer
       ├─ global_ctf_push__OOZtypes_u37282 → 编译期 frame
       ├─ front_end__OOZpassesZfront95endZfront95end_u30105
       │     (tokens → scope → constraint → AST)
       ├─ middle__OOZpassesZmiddleZmiddle_u10
       │     (assembly / common_ancestors / lookahead / lower / ...)
       ├─ (推断) back_end + emit（未在反汇编片段中明显看到，
       │     但模块符号齐全：passes/back_end/{allocate_registers, back_end,
       │     bit_array, emit, lifetime, register_frame, spill_sort, stack}）
       └─ (推断) jit（passes/jit/jit.nim）→ 写入全局 machine_code
```

模块来源证据：
- `compile_source` 调用 `front_end__OOZpassesZfront95endZfront95end_u30105`（已确认）
- `compile_source` 调用 `middle__OOZpassesZmiddleZmiddle_u10`（已确认）
- 模块符号完整覆盖 `passes/back_end/*` + `passes/jit/jit`（strings 提取确认）
- 全局变量 `machine_code__OOZtypes_u34604` @ RVA `0x2fde379b0` 是 seq 头

---

## 参数语义（已实测验证 2026-08-08）

### arg1 (out_buf)

`void*`，caller 分配 ≥40 字节，`compile` 写 5 个字段（见上文输出结构体）。

### arg2 (src_str) — 实测确认

**必须是"指向 NimStringV2 的指针"**，不是裸 char*，也不是 NimStringV2 本身。

本地 Nim 2.2.10 (orc) 生成的类型布局（`strlayout.nim` 编译产物实测）：

```c
struct NimStrPayload { NI cap; char data[]; };   // 字符从 offset 8 开始
struct NimStringV2   { NI len; NimStrPayload* p; }; // 8 字节对齐
```

`compile` 反汇编读取：
- `[arg2+0]` = len（`mov (%rdx),%rbp`）
- `[arg2+8]` = payload ptr（`mov 0x8(%rdx),%r14`）
- 然后 `memcpy(dst, r14+8, len)` —— `r14+8` 跳过 payload 的 `cap` 字段，正好是字符数据

**shim 正确做法**：在 Nim 里构造 `var code = $cstring`，然后传 `addr code`（指向 NimStringV2）。**不能**在 Rust 里手拼 `{len, data}` —— 布局容易错（我们踩过坑：直接传裸 char* 会让 compile 把源码前 8 字节当 len 读成垃圾）。

### arg3 (mode)

低 32 位。分支 `0x2fdde445a: test %r12b,%r12b; jne 0x2fdde45f0 <compile+0x280>` 暗示：
- `mode == 0` → 正常编译路径（直接走 `compile_source`）
- `mode != 0` → 调用 `log__OOZpassesZfront95endZtokens_u258` 后再走相同路径

推测：mode bit 0 = "log tokens to stderr"。实测传 `0` 正常。

### arg4 (flags)

低 32 位。在 `compile_source` 调用时硬编码为 `3`（来自 `mov $0x3, %r9d`），即 arg4 是给调用方的 hint，不直接传给 compile_source。

推测：arg4 = `simulation_state_length`（DSL 的 sim_state 字节数）。

---

## 调用约定清单（实测更新）

| 项 | 值 |
|---|---|
| 调用约定 | Microsoft x64 (rcx/rdx/r8/r9) |
| `NimMain()` | **不能显式调**！`--app:lib` DLL 的 DllMain 在 `LoadLibraryA` 时自动调用。再调一次会触发 `source_buffer.len == 0` "Only call init once" 断言（实测踩坑）。shim 里的 `tccNimMain` 现在是 no-op。 |
| 字符串编码 | UTF-8（无 BOM） |
| 输出结构体内存对齐 | 8 字节（自然对齐） |
| 线程安全 | **非线程安全**——compile.dll 全局状态；需加锁 |
| 返回值 | `rax` = out_buf 指针（不是状态码！低 32 位看起来像乱码 status，实际是 buffer 地址） |

---

## JIT 调用约定（已破解 2026-08-08）

**核心答案：编译出的机器码是一个无参入口，靠绝对地址内存通信。** 反汇编游戏 `jit__modelZsimulationZjitZjit_u1807` + `jit_function__..._u83` 确认：

```text
compile(out, code, 0) → 40 字节 { len, data, entry_off, status, err }

exec  = VirtualAlloc(len, PAGE_EXECUTE_READWRITE) + copy(data+8, len)
arena = VirtualAlloc(0x1000000, ..., PAGE_READWRITE)   # DSL preamble 固定地址
call (exec + entry_off)()                               # 无参调用！
```

**游戏内部流程**（`handle_request_compile_and_run` + `jit`，exe 带完整 Nim 符号，objdump 直接反汇编）：

```
compile() → field_3==0 ? 
  createThread(jit_function, {field_2, field_0, field_1})
    jit(x=machine_code{len,data}, y=entry_off):
      buf = VirtualAlloc(...); copyMem(buf, data+8, len); c_jit()
      call (buf + y)     # 无参数
```

**关键事实：**
1. **入口无参**（`call *%rax`，rcx/rdx/r8/r9 不设置）——机器码通过**绝对地址**读状态：DSL 的 `Ptr 0x1000000`（commands）等 9 个固定地址 + compile.dll 全局（`movabs $0x7ffc...`，**编译时烤进的本进程地址**，必须在编译它的同一进程执行）。
2. **arena 必须映射在 0x1000000**——机器码用 4 字节 immediate（`mov $0x1000000,%esi`）直接寻址。入口自清零 sim_state/keyboard 缓冲。
3. **机器码引用 compile.dll 全局**（字符串字面量等绝对地址）→ **必须在编译它的同一进程运行**（我们本来就如此，shim 持有 compile.dll）。
4. **run_sim 是长驻分发器**，不是一次调用：entry() 会阻塞轮询 `commands[ctl_command_id]`，读到新命令才分发。→ 必须在**后台线程**跑，主线程写命令 + 轮询 + 发 quit（游戏用 `createThread`）。
5. **退出用 `kernel32.ExitThread(0)`**（DSL `thread_exit()`）→ 线程异常终止，Rust `join()` 会 panic `"threads should not terminate unexpectedly"`，需 `catch_unwind` 吞掉。

**驱动协议**（对应 DSL 的 run_sim / mode_run）：
```
commands[ctl_command] = run(0)         # 触发 run 分支
commands[ctl_command_id] = 1           # 命令 id（race 检测）
commands[ctl_test] = test_number       # 测试编号 → seed
commands[ctl_cycle_speed_ms] = 10^13   # 节拍上限 → 自由运行
settings[sim_target_cycle] = target    # 跑多少 cycle
→ 后台线程 call entry()
→ 轮询 sim_running (settings[15]) 1→0 或 sim_cycle==target 或 test_result==2
→ 写 quit_simulation + ctl_command_id=2 → 线程退出
```

**⚠️ DSL 固定地址别名坑**：commands=0x1000000、settings=0x1000010、input_replay=0x1000020 在 8 字节对齐下**物理别名**：
- `input_replay[0]` == `settings[2]`(sim_target_cycle) == `commands[4]`
- `input_replay[1]` == `settings[3]`(sim_test_result) == `commands[5]`

后果：mode_run 每个 cycle 写 `.input_replay[1] = 输入值`，会覆盖 sim_test_result。DSL 的 `set_setting(sim_test_result, max(get_setting(...), res))` 里 `get_setting` 读到的是**输入值**（如 96），`max(96, fail=2)=96` → fail 被吞掉。**生成器必须注意**：失败时直接 `set_setting(sim_test_result, U64 res)`，不要用 max(get_setting(...))。

**执行验证**（2026-08-08，`dll::exec::tests::run_and_gate_machine_code`）：
- and_gate_current.dsl 编译 → 279586 字节机器码 → 执行
- 电路硬编码 result=0，对 condition=3 期望非零 → **check_output 返回 fail(2)，mode_run halt，test_result=2** ✓
- `sim_cycle=1, running=0`（halt 于 cycle 1）✓
- 完整实现见 `src-tauri/src/dll/exec.rs`

---

## test.si API 校对（wiki 对照 2026-08-10）

> Wiki 来源：`turingcomplete.wiki/wiki/Custom_level_creation/test.si`（CC BY-SA 4.0）。
> 以下只提取与我们逆向结论相关的字段；wiki 原文不复述。

### `Input` / `Output` 类型——`Output._is_z` 是关键

`check_output(tick, inputs, outputs)` 的入参**不是裸字典，而是带类型的结构体**：

- `Input`：每个输入引脚一个字段；**非字母数字字符替换为下划线**（如 `Carry in` → `carry_in`）
- `Output`：每个输出引脚**占两个字段**：
  - `<name>: <位宽>` — 输出值
  - `<name>_is_z: Bool` — `true` 表示引脚悬空 / 高阻态

`Output._is_z` 这条**直接影响电路验证正确性**：玩家电路若有输出引脚未驱动，`_is_z == true`；典型 check 应在此情况判 fail。我们当前的 `exec.rs` **不区分 Z 状态**，存在误判风险——LLM 生成的电路若输出悬空，会被错误判为 pass。

### 架构关卡独立 API（盲区）

架构关卡（玩家用 `.isa` + `.asm` + `circuit.data` 自定义 ISA）**不走** `check_output`，改用：

- `arch_check_output(test: Int, input: Int, output: Int) TestResult`
- `arch_get_input(test: Int) Int`

签名形态不同（用 `Int` 而非结构体）。这是文档盲区——目前逆向结论只覆盖**组件关卡**的 `check_output`，架构关卡协议未单独分析。

---

## 已实测确认（2026-08-08，经 shim + Rust 测试）

1. **shim 成功驱动 compile.dll**：`cargo test dll::runtime` 里传合法 DSL，无断言、无 COMPILER ERROR，输出结构体字段填充。
2. **最小当前 dialect DSL 编译成功**（见 `sim-shim/probe.dsl`）：
   - `switch expr` + 缩进 `case {}` 块（`quit_simulation {}` 不是独立语句）
   - `var x = <value>`（没有 `var x: Type` 无初始化）
   - 顶层前向引用 OK、同函数嵌套前向引用 OK
3. **replay.nim 是旧 dialect**：用 `sim_tick`/`target_tick`，当前编译器用 `sim_cycle`/`sim_target_cycle`。且 replay.nim 依赖**跨函数嵌套 def 调用**（`mode_run` 调 `run_sim` 循环里定义的 `get_input`），当前编译器不支持 → 原始 and_gate DSL 编译失败 `No function matched get_input(Int)`。
4. **标准库前缀**：`compile` 自动拼接 27361 字节（1506 行）DSL 标准库（`prefix.dsl` 已提取）——含 82 个函数（`memory_*`, `big_int_*`, `print_using_stack`, `allocate_raw` 等），**不含** `get_input`/`get_output` 等仿真函数（那些在电路 DSL 里定义）。
5. **✅ 真实 and_gate 电路编译成功**（2026-08-08，`sim-shim/and_gate_current.dsl`）：把 replay.nim 的 and_gate DSL 转成当前 dialect 后，经 shim 编译无错误，输出 279586 字节机器码（`field_3=0`/`field_4=0`）。**真实电路（组件连线 + 测试逻辑）能通过游戏本体编译器生成机器码**——这是"用游戏本体验证 LLM 电路"的核心可行性证明。

### DSL dialect 转换要点（`sim-shim/convert_dialect.py`）

从 replay.nim（旧 dialect）转成当前 compile.dll 可编译的 DSL，需要 5 个修复（都是**实测踩坑**得出的 compile.dll 行为）：

| # | 问题 | 修复 | 证据 |
|---|---|---|---|
| 1 | `sim_tick`/`target_tick` 命名 | 全局改名 `tick→cycle`（`sim_cycle`/`target_cycle`/`ctl_cycle_speed_ms`/`burst_cycles`/`nanos_per_cycle`/`get_target_cycle`...） | exe 字符串 codegen 模板 |
| 2 | 跨函数嵌套 def 调用（`mode_run` 调 run_sim 内定义的 `get_input`/`check_output`） | 移到**顶层 def** | `No function matched get_input(Int)` |
| 3 | `mode_run` 内的 `#if in_scope(on_ui_update){...}` 块 | **删除**——它破坏嵌套 def 注册（halt 等找不到了） | probe 二分：加 #if 就失败，去掉就过 |
| 4 | preamble 对齐空格（`var commands                          = Ptr 0x1000000`） | **紧凑化**（单空格）——对齐空格破坏变量注册（`input_replay not in scope`） | n23 换成对齐空格即失败 |
| 5 | **空行**：replay.nim 的双空行格式 | **全部去除**——双空行破坏嵌套 def 注册 | 无空行编译通过、有双空行 `No function matched halt()` |

⚠️ **定义顺序也很关键**：电路 defs（`get_input`/`check_output`/`mode_run`/`mode_refresh`）必须在 helper 区（`ComponentType`/`ui_*`/`get_*`/`set_error`/`reset_sim`）**之前**定义。转换器按此顺序输出。

> 这些怪癖（空行、对齐空格、#if 位置、定义顺序）都是 compile.dll 前端的实现细节 bug/限制。**后续 DSL 生成器必须按 `and_gate_current.dsl` 的紧凑格式输出**，否则同样的错误会出现。

---

## 已知未解之谜

1. **输出结构体的精确字段语义**——成功时 `field_0`/`field_1`/`field_2` 是 machine_code 相关（field_0≈8607 稳定计数、field_1=堆指针、field_2≈8521）；`field_3=0`/`field_4=0` 表示无错误。错误时 `field_3=13`、`field_4=错误消息指针`。
2. **arg3 mode 的全部 bit 含义**——只确认了 bit 0 = log tokens
3. **arg4 flags 的语义**——可能是 `simulation_state_length`
4. ~~**JIT 函数指针的调用约定**~~ ——**已解决**（2026-08-08）：无参入口 + 可执行拷贝 + 入口偏移；arena 映射 0x1000000；后台线程驱动（见上文"JIT 调用约定（已破解）"）。执行已实测：and_gate 电路跑完、test_result=2（fail）。实现见 `src-tauri/src/dll/exec.rs`。
5. ~~当前 DSL 的完整作用域规则~~ ——**已解决**（2026-08-08）：跨函数嵌套 def 需移顶层；`mode_run` 内不能有 `#if in_scope`；preamble 必须紧凑；**不能有空行**；电路 defs 先于 helpers。见上文"DSL dialect 转换要点"。

---

## 下一步建议

### 短期（已完成）

- **真实 and_gate 电路已能编译**（`and_gate_current.dsl`，279586 字节机器码）
- **JIT 机器码已能执行**（`exec.rs`：可执行拷贝 + arena + 后台线程 + 命令驱动 → 读 test_result）。验证：电路对随机输入正确判 fail。

### 下一步

1. **写 DSL 生成器**：从 circuit.data 生成当前-dialect DSL（按 `and_gate_current.dsl` 的紧凑格式，遵守上文的 5 个修复点 + 别名坑）。这是把"执行"变成"验证 LLM 电路"的关键——生成器产出的电路才能跑出有意义的 pass/fail。
2. **接入 Tauri 命令**：把 `run_circuit_test`（已接 exec）暴露为 `#[tauri::command]`，GUI 调它验证电路。
3. 正确电路验证：用生成器产出一个**真会通过**的电路（如全加器），跑出 pass，端到端闭环。

---

## 风险与回退

| 风险 | 触发 | 回退 |
|---|---|---|
| JIT 函数指针推断错（segfault） | Phase D demo 调 fn_ptr 崩 | 改用 Nim 测试程序间接验证；或上 IDA/Ghidra |
| 输出结构体内存对齐错（读到 garbage） | 后续读 machine_code 失败 | 加调试 log 打印每个 offset 的值 |
| `NimMain` 调用多次线程问题 | 后续多线程场景崩溃 | 加 `OnceCell` / `Mutex` |