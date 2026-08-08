---
title: compile.dll::compile 函数完整签名（2026-08-08 反汇编）
last_updated: 2026-08-08
scope: investigation
status: 已完成 Phase A
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

| Offset | Size | 来源寄存器 | 推测语义 |
|---|---|---|---|
| `0x00` | 8 | `r14` | 可能是 `machine_code` Nim seq 的 `data` 指针 |
| `0x08` | 8 | `r12`（=入参 mode） | mode 透传；或 `machine_code` 长度 |
| `0x10` | 4 (高 4 字节 padding) | `r13d` | 错误码 / 状态枚举 |
| `0x18` | 8 | `r15` | 错误信息 / 上下文 |
| `0x20` | 8 | `rbp` | **JIT 编译出的函数指针**（最关键的字段） |

⚠️ 字段语义**未完全确认**——需要与 `Turing Complete.exe` 的 `handle_request_compile_and_run` 调用点交叉验证。**当前足够用于 shim 第一版：shim 只透传输出结构体给 Rust**。

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
4. **JIT 函数指针的调用约定**——成功编译后生成的机器码如何 invoke（最关键的下一步）
5. ~~当前 DSL 的完整作用域规则~~ ——**已解决**（2026-08-08）：跨函数嵌套 def 需移顶层；`mode_run` 内不能有 `#if in_scope`；preamble 必须紧凑；**不能有空行**；电路 defs 先于 helpers。见上文"DSL dialect 转换要点"。

---

## 下一步建议

### 短期（已验证可行）

shim + `run_circuit_test` 骨架已跑通编译。**真实 and_gate 电路已能编译**（`and_gate_current.dsl`，279586 字节机器码）。

下一步：
1. **破解 JIT 机器码的调用约定**（见下方"中期"）——这是"运行电路拿结果"的最后一块
2. 或**写 DSL 生成器**：从 circuit.data 生成当前-dialect DSL（按 `and_gate_current.dsl` 的紧凑格式，遵守上文的 5 个修复点）

### 中期

1. 破解 JIT 机器码的调用约定（IDA 跟 `passes/jit/jit.nim` 的 emit 模式，或试调）
2. 在 Rust 里 `VirtualAlloc` + 拷贝机器码 + `VirtualProtect(PAGE_EXECUTE)` 执行
- 导入 compile.dll::compile
- 直接把 4 个参数透传
- 输出结构体的 40 字节原样回传给 Rust
- Rust 端持有这 40 字节，不尝试调用 JIT 函数指针
- 验证：调用能成功（结构体非空），但不实际运行仿真

### 中期（解决"如何运行"）

写一个 Nim 测试程序，调 `compile.dll::compile`，打印输出结构体的每个字段（用 `$` 重载 / `repr`）：
- 对比"已知正确"的 DSL 输入（比如 replay.nim 第一段，已知 sim_state_length=267）
- 推断每个字段是什么（machine_code 指针？长度？jit 函数指针？）
- 之后才能决定 Rust 端怎么 invoke 仿真

### 长期（如果必要）

如果 JIT 函数指针的签名没法从反汇编稳定推断：
- 上 IDA / Ghidra 看 `passes/jit/jit.nim` 编译后的 `emit_function_entry` 代码生成模式
- 或者试调 + 调试器观察调用约定

---

## 风险与回退

| 风险 | 触发 | 回退 |
|---|---|---|
| JIT 函数指针推断错（segfault） | Phase D demo 调 fn_ptr 崩 | 改用 Nim 测试程序间接验证；或上 IDA/Ghidra |
| 输出结构体内存对齐错（读到 garbage） | 后续读 machine_code 失败 | 加调试 log 打印每个 offset 的值 |
| `NimMain` 调用多次线程问题 | 后续多线程场景崩溃 | 加 `OnceCell` / `Mutex` |