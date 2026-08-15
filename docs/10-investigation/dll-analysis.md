---
title: DLL 静态分析（compile.dll & game_engine.dll）
last_updated: 2026-08-15
scope: investigation
status: 已审（compile.dll 已重做签名调研；2026-08-15 纠正 game_engine.dll：非 Godot，C++ ImGui/OpenGL3）
---

# DLL 静态分析

## 概要

| DLL | 大小 | 平台 | 性质 |
|---|---|---|---|
| `compile.dll` | 1,786,818 B (1.78 MB) | x86-64 | Nim 编译器 + LLVM 后端 + 仿真器运行时 |
| `game_engine.dll` | 1,988,608 B (1.99 MB) | x86-64 | C++(MSVC) + Dear ImGui + OpenGL3 自研渲染/UI 引擎 |

文件位置：`E:\SteamLibrary\steamapps\common\Turing Complete\`

---

## compile.dll（2026-08-08 重做）

### 性质

`compile.dll` 是 **Nim 编译器本体 + 仿真器专用运行时**。证据：

- 字符串表中大量 Nim 编译器 pass 符号：`compile_function__OOZpassesZfront95endZfront95end_u2948`、`compile_source__OOZcore_u144`、`passes/front_end`、`passes/back_end`、`passes/middle` 等
- 内嵌 LLVM 后端符号（`X86_TUNE_*`、`ARM_SPECIAL_REGISTERS`、`VectorRegister` 等）
- 静态链入 Win32 API（`VirtualAlloc`、`GetProcAddress`、`LoadLibraryA`）
- 包含 `native_alloc/alloc` 的核心函数（`__allocate_clear`、`__allocate_data`、`__create_arenas` 等）—— 这就是 `replay.nim` 里 `import native_alloc/alloc` 解析的实际实现

**关键解读**：`compile.dll` 不只是 Nim 编译器——它是 **为这个游戏专门打包的"Nim 编译器 + 仿真器运行时 + 仿真器内存分配器"一体包**。`replay.nim` 里的 `simulator_types`、`native_alloc/alloc` 实际指向 DLL 内部已编译的代码。

### 导出表（精确：仅 3 个）

`objdump -p` 实测：

| 导出名 | RVA | 类型 | 用途 |
|---|---|---|---|
| `NimMain` | `0x000f4dd0` | 函数 | Nim 运行时入口，**调用任何 Nim 代码前必须先调** |
| `NimDestroyGlobals` | `0x000f47e0` | 函数 | Nim 运行时反初始化 |
| `compile` | `0x000f4370` | 函数 | **核心入口**：编译并运行一段 Nim 源码 |

> ⚠️ 之前的 wiki 文档说"≥6 个导出"——**实测只有 3 个**。其他"导出"猜测（NimMainInner、NimMainModule、nim_program_result）实际是 DLL 内部函数，未通过 PE 导出表暴露。

### `compile` 函数签名分析

**完整反汇编已迁移到** [`compile-signature.md`](compile-signature.md)。摘要：

```c
void compile(
    void*   out_buf,    // rcx:  40 字节输出结构体
    void*   src_str,    // rdx:  Nim string {int64 length, ptr char_data}
    int32_t mode,       // r8:   仅低 32 位使用
    int32_t flags       // r9:   仅低 32 位使用
);
```

**关键发现**：
- `compile` 内部把 **27361 字节的 Nim DSL 标准库前缀**拼到用户源码前面，再编译成机器码
- 输出 40 字节结构体，5 个字段（详细见 `compile-signature.md`）
- 必须先调 `NimMain()` 初始化；多次调需要锁

**粗略 prologue**（`0x2fdde4370`）：

```asm
00000002fdde4370 <compile>:
   2fdde4370: push   %r15, %r14, %r13, %r12, %rbp, %rdi, %rsi, %rbx  ; 保存 8 个 callee-saved
   2fdde437c: sub    $0x118,%rsp                                      ; 280 字节栈帧
   2fdde4383: movaps %xmm6,0x100(%rsp)                                ; 保存 xmm6（SIMD 用？）
   2fdde438b: pxor   %xmm0,%xmm0
   2fdde438f: mov    (%rdx),%rbp                                       ; rdx 是 arg2 指针；rbp = arg2->field0
   2fdde4392: mov    0x8(%rdx),%r14                                    ; r14 = arg2->field1
   2fdde4396: lea    0x6ae1(%rbp),%rdx                                 ; rdx = arg2->field0 + 0x6AE1
   2fdde439d: mov    %rcx,%rbx                                         ; 保存 arg1
   2fdde43a0: lea    0x40(%rsp),%rcx                                   ; 新建 0x40 字节栈缓冲
   2fdde43a5: mov    %r8d,%r12d                                        ; 保存 arg4 低 32 位
   ... (后续读 simulation_state 等)
```

**推断的 C ABI 签名**：

```c
int32_t compile(
    void*       arg1,   // rcx: Nim string（compile 的 Nim 源码，repr=ptr+len）
    void*       arg2,   // rdx: 指向编译器上下文结构体的指针；至少含两个指针字段
    int32_t     arg3,   // r8:  flags / mode
    int32_t     arg4    // r9:  低 32 位有效，含义待定
);
```

**关键证据**：
- x86-64 Windows 调用约定（rcx/rdx/r8/r9）
- arg2 是结构体指针：`mov (%rdx),%rbp` 读 offset 0，`mov 0x8(%rdx),%r14` 读 offset 8
- arg2->field0 是一个内部数据结构（被以 +0x6AE1 偏移访问，0x6AE1=27361 字节处）
- arg1 (rcx) 完整保留到 rbx，跨多个子调用 → 这是贯穿整个函数的"主输入"（最可能是 Nim 源码字符串）
- 函数尾有多处 `ret`，且 prologue/epilogue 不对称（保存 8 个寄存器，ret 时只还原 5 个）→ **使用 Nim 的 setjmp/longjmp 异常处理**

**未解之谜**：
- arg3/arg4 的精确语义（flags？length？arena？）
- arg2 结构体的完整字段布局
- arg2->field0 的类型（编译器内部 context？还是 simulation state？）
- 返回值是否就是 nim_program_result 的引用

### 从 Rust 调用 `compile.dll` 的潜在风险

1. **必须先 `NimMain()`**——否则 GC/全局状态未初始化，所有 Nim 操作会段错误
2. **`compile` 函数签名只有部分推断**——arg3/arg4 含义未知，结构体字段未完全映射
3. **Nim GC 与 Rust ABI 微妙不同**——特别是 `string`、`seq[T]`、内部 context 跨 DLL 边界
4. **DLL 内部不重定位**——编译期固定 `image base = 0x180000000`，如果 Rust 进程占用该地址会冲突
5. **`compile.dll` 内嵌整个 Nim + LLVM**——1.7 MB 代码全在内部，不可裁剪

### 调用约定验证

- **调用约定**：Microsoft x64（Windows 默认 cdecl 等价）
- **string 编码**：Nim `string` = `(len: int64, data: ptr char)`（长度前缀结构）
- **seq[T]**：`(cap: int, len: int, data: ptr T)`
- **GC**：默认 `refc` 或 `arc`，需要在 LoadLibrary 后立刻调 `NimMain()` 初始化

### 推荐路径：写 Nim C-ABI shim（不在本次范围内）

由于 `compile` 签名不完整，**强烈不推荐** Rust 直接试调 `compile`。推荐路径：

1. **写一个 Nim shim DLL**——内部 import `compile.dll::compile`，外部暴露清晰 C-ABI
2. shim 负责：
   - 构造正确格式的 source `string`
   - 构造 simulator state 结构体（含 arena allocator header）
   - 调用 `compile`
   - 把结果通过简单 out-parameter 返回给 Rust
3. **Rust 调 shim 而不是 compile.dll**——所有 Nim GC 操作留在 Nim 世界内，ABI 风险降到零

实现步骤：

```nim
# shim.nim
{.push dynlib.}

proc compile_inner(src: string, ctx: ptr SomeStruct, flags, opts: cint): cint
  {.importc, dynlib: "compile.dll".}

type
  CompileArgs = object
    field0: pointer
    field1: pointer
    # ... 通过 Nim 编译器内部模块分析后填全

{.pop.}

proc tcc_compile(source_code: string, sim_state: pointer, state_len: cint,
                out_pass: ptr cint, out_result: ptr cint): cint
  {.exportc, dynlib, cdecl.} =
  # 构造 CompileArgs，调用 compile_inner，解析返回值
  ...
```

编译：`nim c --app:lib --out:shim.dll shim.nim`

Rust 端 `libloading::Library::new("shim.dll")?` + 调用 `tcc_compile`。

## 后续建议（更新版）

1. ❌ ~~IDA / Ghidra 静态分析 `compile.dll` 完整签名~~ — 已用 `objdump` 完整推断（4 参数 + 40 字节输出结构体），见 `compile-signature.md`
2. ✅ **写 Nim C-ABI shim DLL**（推荐路径）—— 内部封装 `compile.dll::compile`，把 40 字节输出透传给 Rust
3. ⚠️ **JIT 函数指针调用约定未完全确定** —— 需要写 Nim 测试程序间接验证，或上 IDA 跟 `passes/jit/jit.nim` 的代码生成模式
4. ⚠️ **simulator runtime 在 exe 里静态链接**——`handle_request_compile_and_run` 不可直接调；只能复用 compile.dll 的 JIT 输出
5. **下一步**：shim.nim（Phase B+C），只透传编译结果，不执行仿真；后续再单独解决"如何调用 JIT 函数指针"

| 节 | VA | VSize | 推测用途 |
|---|---|---|---|
| `.text` | 0x2fdcf1000 | 1,026,528 | 可执行代码 |
| `.data` | 0x2fddec000 | 2,144 | 已初始化数据 |
| `.rdata` | 0x2fdded000 | 161,016 | 只读数据、字符串、Nim pass 符号名 |
| `.pdata` | 0x2fde15000 | 25,584 | 异常处理表 |
| `.xdata` | 0x2fde1c000 | 26,752 | 异常处理展开信息 |
| `.bss` | 0x2fde23000 | 89,088 | 未初始化数据（运行时分配） |
| `.edata` | 0x2fde39000 | 116 | 导出表（**实测只有 3 项**） |
| `.idata` | 0x2fde3a000 | 2,636 | 导入表 |
| `.tls` | 0x2fde3b000 | 16 | 线程局部存储 |
| `.reloc` | 0x2fde3c000 | 3,572 | 基址重定位表 |
| `.debug_*` | — | ~64K | DWARF 调试信息 |
| 8 个匿名节 | — | 总计 ~189K | LLVM/Nim 内部数据 |

---

## game_engine.dll

### 性质

C++(MSVC) + Dear ImGui + OpenGL3 自研渲染/UI 引擎层。**只关心窗口/输入事件**，对电路语义无直接价值。

### 完整导出清单（10 个）

| 导出名 | 推测用途 |
|---|---|
| `game_engine_initialize` | 启动引擎 |
| `game_engine_initialize_platform` | 平台相关初始化 |
| `game_engine_destroy` | 关闭引擎 |
| `game_engine_destroy_platform` | 平台相关销毁 |
| `game_engine_change_window_mode` | 切换窗口/全屏 |
| `game_engine_post_render` | 帧渲染后回调注册 |
| `game_engine_pre_render` | 帧渲染前回调注册 |
| `game_engine_get_window_position` | 获取窗口位置 |
| `game_engine_get_key_coordinates_recorded` | 读取键盘坐标记录 |
| `game_engine_clear_key_coordinates_recorded` | 清空键盘坐标记录 |

### 是否需要集成？

**结论：不需要**。这些函数只暴露窗口管理 + 键盘输入事件。电路仿真的核心数据流走 `compile.dll` + `replay.nim`，不走 `game_engine.dll`。

如果未来要注入 hook 拦截玩家操作（用于自动重放），可能要用 `pre_render`/`post_render` 回调。但本次不做。

---

## 后续建议（更新版）

1. ❌ ~~IDA / Ghidra 静态分析 `compile.dll` 完整签名~~ — 已用 `objdump` 部分推断（4 个参数，结构体 context），完整结构体字段还需 IDA Pro
2. ✅ **写 Nim C-ABI shim DLL**（推荐路径）—— 内部封装 `compile.dll::compile`，暴露清晰的 C 接口给 Rust 调用
3. ✅ 在 Nim 端做 `simulator_state` 构造（用 `replay.nim` 已有的 `simulator_types` + `native_alloc/alloc`）
4. 写一个最小 Nim 测试程序（不带 GUI），从 `compile.dll` 加载 `compile`，传入一个 1 行的 Nim 源码，验证返回
5. Rust 端通过 `libloading` 加载 shim，先做"最小 demo"：把 `simulate_combinational` 的输出作为输入，写一个完整的电路文件，让 `compile.dll` 跑通

`D-7`（注入机制）的具体实现路径取决于 `compile.dll` 的实际可调用性——目前看 shim 路线最稳。