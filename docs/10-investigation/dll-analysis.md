---
title: DLL 静态分析（compile.dll & game_engine.dll）
last_updated: 2026-08-08
scope: investigation
status: 已审
---

# DLL 静态分析

## 概要

| DLL | 大小 | 平台 | 节数 | 导出数 | 性质 |
|---|---|---|---|---|---|
| `compile.dll` | 1,786,818 B (1.78 MB) | x86-64 | 19 | 少量（≥6） | Nim 编译产物 + LLVM 后端 |
| `game_engine.dll` | 1,988,608 B (1.99 MB) | x86-64 | 6 | 10 | Godot 引擎的 C-ABI 薄包装 |

文件位置：`E:\SteamLibrary\steamapps\common\Turing Complete\`

---

## compile.dll

### 性质

确认是 **Nim 编译产物**。证据：

- 导出 `NimMain`、`NimMainInner`、`NimMainModule`、`NimDestroyGlobals`、`nim_program_result` — Nim 运行时标配符号
- 字符串表中大量 LLVM 后端符号（`X86_TUNE_*`、`ARM_SPECIAL_REGISTERS`、`FloatRegisters`、`VectorRegister` 等）
- 静态链入 Win32 API（`VirtualAlloc`、`VirtualFree`、`GetProcAddress`、`LoadLibraryA`、`TlsGetValue` 等）

这意味着 `compile.dll` **内嵌了整个 Nim 编译器 + LLVM**，可以在运行时把任意 Nim 源（典型如 `replay.nim`）编译为机器码并执行。

### 导出表（精确名单）

| 导出名 | 类型 | 推测用途 |
|---|---|---|
| `NimMain` | 函数 | Nim 运行时入口，调用任何 Nim 代码前必须先调 |
| `NimMainInner` | 函数 | 同上的内部版本 |
| `NimMainModule` | 函数 | 模块初始化（用户在 Nim 中写 `proc foo()` 时注册） |
| `NimDestroyGlobals` | 函数 | 反初始化，释放 Nim 全局状态 |
| `nim_program_result` | 数据 | Nim 程序退出码存放处 |
| `compile` | 函数 | **自定义入口**，签名未确定（详见下文） |

> 共 ≥6 个导出。`compile` 是游戏开发者导出的主入口函数，但其参数列表与返回类型**目前无法静态确定**。需要 IDA / Ghidra / x64dbg 动态分析。

### ABI 推测

- **调用约定**：cdecl（x86-64 上等价于 System V/Microsoft x64）
- **string 编码**：Nim 默认 `string` = `(len: int64, data: ptr char)`（长度前缀结构）
- **seq[T]**：`(cap: int, len: int, data: ptr T)`，GC 头在 GC_ref 上
- **GC**：默认 `refc` 或 `arc`，需要在 LoadLibrary 后立刻调 `NimMain()` 初始化

### 从 Rust 调用 `compile.dll` 的潜在风险

1. **必须先 `NimMain()`**——否则 GC 未初始化，所有 `seq`/`string` 操作会段错误
2. **`compile` 函数签名未知**——需要在调试器里看反汇编才能知道参数类型
3. **Nim ABI 与 Rust ABI 微妙不同**——尤其是 GC 对象跨 DLL 边界
4. **导出少但内部巨大**——1.7 MB 代码全在内部，不可重定位，JIT 行为受限

**推荐路径**：写一个 thin C shim DLL（用 Nim 编译），由 Rust 调 shim，shim 调 `compile`。这样所有 GC 操作都留在 Nim 世界内。

### 节区结构

| 节 | VA | VSize | 推测用途 |
|---|---|---|---|
| `.text` | 0x1000 | 1,026,528 | 可执行代码 |
| `.data` | 0xFC000 | 2,144 | 已初始化数据 |
| `.rdata` | 0xFD000 | 161,016 | 只读数据、字符串 |
| `.pdata` | 0x125000 | 25,584 | 异常处理表 |
| `.xdata` | 0x12C000 | 26,752 | 异常处理展开信息 |
| `.bss` | 0x133000 | 89,088 | 未初始化数据（运行时分配） |
| `.edata` | 0x149000 | 116 | 导出表 |
| `.idata` | 0x14A000 | 2,636 | 导入表 |
| `.tls` | 0x14B000 | 16 | 线程局部存储 |
| `.reloc` | 0x14C000 | 3,572 | 基址重定位表 |
| 8 个匿名节 | — | 总计 ~189K | LLVM/Nim 内部数据 |

LLVM 嵌入 8 个匿名节，占总大小 23% 左右。

---

## game_engine.dll

### 性质

Godot 引擎的 C-ABI 薄包装层。**只关心窗口/输入事件**，对电路语义无直接价值。

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

**本次调研结论：不需要**。
- 这些函数只暴露窗口管理 + 键盘输入事件
- 电路仿真的核心数据流走 `compile.dll` + `replay.nim`，不走 `game_engine.dll`
- 集成它没有带来额外能力

如果未来要注入 hook 拦截玩家操作（用于自动重放），可能要用 `pre_render`/`post_render` 回调。但本次不做。

---

## 后续建议

1. **优先做 IDA / Ghidra 静态分析 `compile.dll`**——确认 `compile` 的真实函数签名（参数个数、类型、是否 cdecl）
2. **优先做动态追踪**——在游戏运行 `replay.nim` 时用 x64dbg 设断点，看 `compile` 被如何调用
3. **写一个最小的 Nim C-ABI shim**——Rust 调 shim，shim 调 `compile`，规避 GC ABI 风险

这些是 `20-design/index.md` 中真正动手前的必经步骤。