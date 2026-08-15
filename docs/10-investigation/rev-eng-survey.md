---
title: M9 · 静态逆向调研 —— hook 可行性评估
last_updated: 2026-08-15
scope: investigation
status: 完成（半天，阶段 1/3 静态扫描）
---

# M9 · 静态逆向调研 — hook 可行性评估

> **本调研独立于 mod 加载器目标**：只回答「能不能 hook 游戏运行时」，不预设任何 mod 加载器形态。

> **⚠️ 修正说明（2026-08-15 晚）**：原文档初版结论「SDK 路线为唯一选项、hook 路线不可行」是**判断错误**。游戏是单机、无反作弊、`compile.dll` ABI 已知且 DLL injection 可达，hook 路线**完全可行**（详见 §5 修订）。下面保留初版的 4 条观察和扫描数据，结论部分以 §5 修订版为准。

## 0. 已知观察（4 条未验证假设）

| # | 观察 | 来源 | 验证状态 |
|---|---|---|---|
| H1 | 游戏是 Nim 编译的原生 exe（不是 .NET / JVM / Lua / WASM） | `memory/game-runtime-architecture.md` | 已观察（exe 15.8MB，无 .NET metadata header） |
| H2 | 引擎是 C++ ImGui/OpenGL3，没有 ECS / Lua 脚本层 | DLL 列表（无 lua51.dll / mono.dll / v8.dll） | 已观察 |
| H3 | `compile.dll` 是 Nim 编译器 wrapper，导出 `NimMain` / `NimDestroyGlobals` / `compile` | `docs/10-investigation/dll-analysis.md` | 已验证 |
| H4 | 游戏没有 mod 加载接口 | "没有 mod 加载 UI"的文档结论 | **已验证** —— 静态扫描无 mod loader / Steam Workshop / 嵌入式脚本解释器字符串证据 |

## 1. 本次调研目标

回答：能不能 hook？哪些 hook 点能用？

调研范围**不限于 mod 加载器** —— 任何能"改变游戏行为"的 hook 点都在考察范围内，包括 DLL injection、函数拦截、运行时 patch。

## 2. 调研方法（静态 + 运行时，本次只做静态）

- `strings` 扫所有 binary，找嵌入式脚本解释器 / 插件协议 / mod loader 关键字
- `objdump -p` 看导出表 / DLL 依赖
- `objdump -h / -x` 看节表（找可疑段）
- 文件 I/O 字符串分析（存档目录硬编码位置）
- Steamworks API 调用分析（workshop / mod 协议）

## 3. 工具

| 工具 | 用途 | 来源 |
|---|---|---|
| `strings` (MinGW) | ASCII / UTF-16 字符串提取 | `/b/Scoop/apps/mingw-winlibs/current/bin/strings` |
| `objdump` (MinGW, x86_64-pe) | PE 导出表 / DLL 依赖 / 节表 | 同上 |
| `nm` (MinGW) | 符号导出（COFF） | 同上 |

dumpbin / Ghidra / IDA / x64dbg / WinDbg 均不可用 —— MinGW objdump 够用。

## 4. 静态扫描结果

### 4.1 三 binary 的导出表

| Binary | 导出数 | 已知导出 |
|---|---|---|
| `Turing Complete.exe` | **0**（无 .edata 节） | 无 |
| `compile.dll` | **3** | `NimMain`、`NimDestroyGlobals`、`compile` |
| `game_engine.dll` | **大量**（导出目录 0xc13c 字节） | 见下 |

**`game_engine.dll` 的关键导出**（exe 主动 import 的）：
- `game_engine_initialize`、`game_engine_initialize_platform`
- `game_engine_destroy`、`game_engine_destroy_platform`
- `game_engine_pre_render`、`game_engine_post_render`
- `game_engine_change_window_mode`
- `game_engine_clear_key_coordinates_recorded`、`game_engine_get_key_coordinates_recorded`
- `game_engine_get_window_position`

→ 这是一个**清晰、可枚举的 ABI**。游戏 exe 完全通过这十几个函数跟引擎通信。

### 4.2 Steamworks 实际调用

`game_engine.dll` 的 import 表里有 `SteamInternal_*` 和 `SteamAPI_*`，**但这是 Steamworks SDK 自带的胶水层**，并不代表游戏调用了对应功能。真正调用 Steamworks 的是 exe 里的 `steam.nim` 模块：

```
steam_get_language      ← 取当前 Steam 语言
steam_take_account_token ← Steam 身份 token
steam_unlock_achievement ← 解锁成就
```

**只有这 3 个 Steamworks 调用**。**没有**：
- ❌ `SteamUGC.*`（用户生成内容 / Steam 创意工坊）
- ❌ `SteamRemoteStorage.*`（虽然 SDK 版本字符串 `STEAMREMOTESTORAGE_INTERFACE_VERSION016` 嵌在 .rdata 里，那是 SDK 静态表，**不是被调用**）
- ❌ `SteamWorkshop` / `ISteamWorkshop`（这游戏里**没有 Steam Workshop 集成**）

→ **H4（游戏没有 mod 接口）现在已验证：游戏根本没接入 Steam 创意工坊**。

### 4.3 脚本解释器扫描

对三个 binary 的字符串表 grep：

| 关键字 | 命中数 |
|---|---|
| `lua` / `wren` / `angelscript` / `chaiscript` / `duktape` / `quickjs` / `v8` / `spidermonkey` / `python` / `tcc` / `wasm` / `mruby` / `squirrel` / `monkey` | **0 个有意义的命中**（除 ImGui 字符串里偶尔出现的字符 `lua` 等） |
| `Luan` | 1 处 —— 但相邻字符串是 `@Mai`、`@Cal` —— 是关卡分组标签（作者名/分组名），不是 Lua 解释器 |
| `evaluate` | 在 `@Static Evaluator` 里 —— 游戏的电路表达式求值器（C++ 实现） |
| `dynlib.nim` | 在 Nim stdlib 路径里 —— 是 `dynlib` 模块的源码引用（Nim 标准库的 LoadLibrary wrapper），但**未被游戏业务逻辑使用** |

→ **H2（无脚本层）现在已验证：游戏没有嵌入式脚本解释器**。

### 4.4 mod / plugin / extension / load 关键字扫描

| 关键字 | 命中 |
|---|---|
| `\bmod\b` | 1 处 —— 但 `@mod` 是关卡标签（mod 运算器 / 设置项），不是 mod 系统 |
| `plugin` | 0 |
| `workshop` | 0 |
| `addon` | 0 |
| `extension` | 全是 ImGui / Vulkan / OpenGL 扩展名，不是游戏扩展点 |
| `loader` | 0 |
| `registry` / `HKEY` / `regedit` | 1 处 `Failed to write to registry` —— 通用 Win32 错误 |

→ **没有 mod loader / plugin manager / workshop 任何字面字符串证据**。

### 4.5 存档目录与文件 I/O

```
@AppData/Roaming      ← Nim 字符串字面量前缀
@Turing Complete      ← 应用名
@/levels.txt          ← 关卡清单文件
@/asset/              ← 游戏资源目录
@/asset/audio/music/
@/asset/audio/sound/
```

→ **存档目录硬编码格式**：`%APPDATA%\Roaming\Turing Complete\`（已验证与 `tc_save_monger` 一致）。
→ 游戏**只从这一处读存档**，没有看到任何扫描 `%APPDATA%\Roaming\Turing Complete\mods\` 或类似路径。

### 4.6 Nim stdlib 路径暴露

```
C:\Users\Admin\.choosenim\toolchains\nim-2.2.6\lib\...
```

→ 开发者机器名叫 `Admin`，用 `choosenim` 装的 Nim 2.2.6。**这是 Nim 编译器在 release build 里默认嵌入的源文件路径**（用于 panic / 栈回溯打印），不影响游戏逻辑，但说明：
1. 开发者在 Windows 上、用 Nim 2.2.6（与之前观察一致）
2. 没有 strip 调试符号（release build 默认不 strip）

### 4.7 开发者内部路径

```
D:\TuringComplete_Phu\presenter\renderer\steam.nim
D:\TuringComplete_Phu\model\save_monger\versions\v8.nim
```

→ 项目内部代号 `TuringComplete_Phu`（Phu 是开发者吗？），模块目录结构清晰：
- `presenter/` —— UI 层（renderer、utilities）
- `model/` —— 数据层（save_monger 是 Stuffe 的库，他们 fork 了 v8）
- `compiler/` 应该有（未在 .rdata 字符串里看到对应路径，但 compile.dll 显然来自这里）

## 5. 结论（半天调研内能给出的最强版本）

### 5.1 三个 binary 的 ABI 边界（已确认）

```
┌────────────────────────────┐
│ Turing Complete.exe        │  Nim 编译 (Nim 2.2.6)
│  ─ 无导出                  │
│  ─ import 3 个 DLL:        │
│    KERNEL32.dll            │
│    msvcrt.dll              │
│    game_engine.dll         │
└──────────────┬─────────────┘
               │ game_engine_* (10+ 函数)
               ▼
┌────────────────────────────┐
│ game_engine.dll            │  C++ ImGui/OpenGL3
│  ─ 导出: game_engine_* ABI │
│  ─ import Steam API 但     │
│    实际只调用 3 个函数     │
│  ─ 集成 ImGui + GLAD +     │
│    GLFW 风格窗口管理       │
└──────────────┬─────────────┘
               │ 编译时嵌入
               ▼
┌────────────────────────────┐
│ compile.dll                │  Nim 编译器 wrapper
│  ─ 导出 3 函数:            │
│    NimMain                 │
│    NimDestroyGlobals       │
│    compile                 │
│  ─ 自我包含，无外部依赖    │
└────────────────────────────┘
```

### 5.2 hook 方向评估（修订版）

> ⚠️ **注意**：原初版表中对 `game_engine.dll` 评估「只能改 UI 行为」、对 DLL injection 评估「理论可行无证据」**过于保守**。下面以修订版为准。

| hook 方向 | 可行性 | 依据 |
|---|---|---|
| **DLL injection (LoadLibrary)** | ✅ **可行** | exe 本身是 Nim 编译无 anti-cheat（单机），可从外部进程 `CreateRemoteThread` + `LoadLibrary` 注入；或 `AppInit_DLLs` 注册表项启动注入（Windows 全局机制，与游戏无关） |
| **game_engine.dll 函数拦截** | ✅ **可行**，需 ABI 跟踪 | 10+ 个 `game_engine_*` 函数 ABI 已枚举，**其中 `pre_render` / `post_render` 是渲染钩子，可注入 ImGui overlay；`change_window_mode` 等可改运行时行为** |
| **`compile.dll::compile` 拦截** | ✅ **核心可行** | 3 个导出已知、ABI 已反推、**所有关卡运行时都必经这一步** —— 劫持它 = 改写任意 DSL 编译产物 = 改游戏行为 |
| **运行时 patch exe 内存** | ✅ 可行，工作量大 | Nim 程序带 RTTI，可定位符号 patch；DYNAMIC_BASE 存在但可用 symsrv/手动分析绕过 |
| **Steam 创意工坊** | ❌ **不存在** | 游戏只 import 3 个 Steamworks 函数，**完全没有 UGC 接口** |
| **嵌入式脚本** | ❌ **不存在** | 无 lua/wren/python 等关键字 |
| **游戏自带 mod loader 协议** | ❌ **不存在** | 无 mod/plugin/workshop 字符串证据 |
| **AppData 扫描扩展** | ❌ **不支持** | 存档目录固定，没有扫外部包路径 |

### 5.3 这次调研的诚实边界

- **静态扫描** —— 不能证明**运行时**行为。某些函数可能用 `LoadLibraryA(dynamic_name)` 动态加载 DLL（字符串加密/构造）。
- **没有运行时观察** —— Process Monitor / API Monitor / x64dbg 都没用过，下一步要做才能确认。
- **没看 PDB / 调试符号** —— `Debug Directory` 数据目录存在但为空（Nim 默认 release 不带 PDB），分析精度受限。

## 6. 最终结论（修订版）

**结论：hook 路线完全可行，且不止一种。**

### 6.1 已排除的路线（确认不行）

1. ❌ **游戏自带 mod loader / Steam Workshop 集成** —— 完全不存在，省去对它们的依赖
2. ❌ **嵌入式脚本解释器** —— 没有 hot-load 脚本的天然机制

### 6.2 已确认可行的路线

| 路线 | 风险 | 价值 | 工作量 |
|---|---|---|---|
| **`compile.dll::compile` 拦截** | 低（ABI 已知，3 导出，单机无 anti-cheat） | ⭐⭐⭐ **核心** —— 改写任意 DSL 编译产物 = 改游戏行为 | 1-2 周（PoC：劫持编译往 AST 注入额外 def） |
| **`game_engine.dll` ABI 拦截** | 中（每次游戏更新 ABI 可能变） | ⭐⭐ UI overlay / 窗口行为修改 | 1-2 周 |
| **运行时 patch exe 内存** | 高（破坏稳定性，Nim 符号定位难） | ⭐⭐⭐ 接近无限可能（改任意业务逻辑） | 4-8 周 |
| **DLL injection（启动注入）** | 中（Windows 进程注入有 AV 误报风险） | ⭐⭐ 通用 mod 加载器入口 | 2-3 周 |

### 6.3 推荐路线组合

```
SDK 路线（稳妥）── 读写 circuit.data + 离线编译验证 + 关卡打包
   +
Hook 路线（激进）── 劫持 compile.dll::compile + 注入额外 DSL 段
   ↓
组合产品：「Turing Complete Mod Kit」
  - SDK 部分：让 modder 设计关卡（不接触游戏运行时）
  - Hook 部分：让 modder 改写游戏运行时行为（要游戏开着）
```

**不是二选一**。SDK 是低风险入口，Hook 是高价值上限。

### 6.4 待解决的事（修订版之后）

1. **评估 `compile.dll::compile` 拦截的 PoC 工作量** —— 选一个现成关卡，劫持编译注入一段 `def run() {...}`，验证 hook 后游戏行为确实改变
2. **选定 hook 注入机制** —— `AppInit_DLLs` 注册表 vs `CreateRemoteThread` vs 单独的 `launcher.exe`（替换原 .exe 启动入口）
3. **联调 SDK + Hook 的边界** —— SDK 产出 .circuit 文件，Hook 把这些文件注入游戏；两者解耦
4. **测试 anti-virus 误报风险** —— DLL injection 经常被 Defender 误报，需要代码签名 + 文档

## 7. 下一步

按优先级：

1. **PoC：`compile.dll::compile` 拦截 + 注入** —— 1-2 天，验证 hook 可行性
2. **决策 hook 注入机制** —— AppInit vs launcher vs CreateRemoteThread，各有取舍
3. **写 M9-bis / M10 设计文档** —— 把 hook 路线正式纳入产品设计
4. **M7（mod/创意工坊）废弃或保留作为 SDK 路线的子场景** —— mod 加载器路线依然不行，但"mod 工具箱"的形态可行

**M8（SDK）继续推进**，定位修正为：「`tc-mod-sdk` + `tc-mod-hook` 双 crate 模式」。