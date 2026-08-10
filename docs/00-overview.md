---
title: 项目总览
last_updated: 2026-08-10
scope: overview
status: 已审（2026-08-10 整合 Stuffe 官方仓库 + tc_save_monger crate 路线）
---

# 项目总览

## 是什么

`turing-complete-manager` 是一个 **Steam 游戏 Turing Complete 的第三方存档管理器**。
游戏本身用 Godot 引擎实现，存档以纯文本 (`levels.txt`)、二进制 (`circuit.data`)、
注册表 (`HKCU\Software\Turing Complete`) 多种形式散落在 Windows 用户目录。

本项目提供一个 Tauri 2 桌面应用，UI 用原生 TypeScript（无框架），用于：

- 浏览/解锁 88 个关卡
- 备份/恢复存档
- 检测游戏安装目录与存档目录
- 自动定时备份

M1–M6 已完成，详见 `CHANGELOG` / `README.md`。

## 为什么要做这次调研

游戏目录 `E:\SteamLibrary\steamapps\common\Turing Complete\` 下有几个诱人的资产：

| 文件 | 大小 | 性质 |
|---|---|---|
| `compile.dll` | 1.78 MB | Nim 编译产物，内嵌 LLVM 后端 |
| `game_engine.dll` | 1.99 MB | Godot 引擎 C-ABI 薄包装 |
| `replay.nim` | 79 MB / 227 万行 | **可被 `compile.dll` 编译执行的 Nim 源** |
| `Turing Complete.exe` | 15.8 MB | Godot 主程序 |

它们提供了「让大模型对电路做优化」的潜在切入点。本次调研的目的是：

1. **摸清** 哪些资产可以从外部调用（ABI、签名、依赖）
2. **摸清** `replay.nim` 的 schema（生成方式、嵌入数据结构）
3. **沉淀** 文档，为后续 CLI / LLM 集成计划提供基础

## 本次边界

### 已落地（调研结果沉淀在 docs/）

- ✅ 静态分析两个 DLL 的 PE 结构与导出符号 → `10-investigation/dll-analysis.md`
- ✅ grep/抽样分析 `replay.nim` 的 schema → `10-investigation/replay-format.md`
- ✅ `circuit.data` 二进制格式已通过外部参考实现 `tc-save-lab` 完整破解（v15 严格读写 + v7/v13/v14 只读）→ `10-investigation/circuit-data-format.md`
- ✅ 关卡脚手架提取（输入/输出 pin 元数据）已有 Python 实现可直接复用
- ✅ 92 个主线关卡 immutable 组件清单已生成（`examples/*/scaffold/immutable.json`）
- ✅ **Stuffe（游戏作者）公开的官方 codec** 已 clone 到 `reference/save_monger/`（CC0，含 v0..v15 全版本）→ `10-investigation/circuit-data-format.md` §Stuffe/save_monger
- ✅ **Stuffe 公开的 ISA 规范处理器** 已 clone 到 `reference/isa_spec/`（MIT）→ `10-investigation/architecture-levels.md` §权威实现
- ✅ **官方 ComponentKind 枚举** 已校对：125 slot（0..124，101 active + 24 deleted）

### 待办（后续独立计划）

- ❌ 实际调用 `compile.dll` 驱动游戏本体（`compile` 函数签名未确定）
- ❌ ~~Rust 移植 codec 到 Tauri app~~ → **改用 `tc_save_monger` crate**（CC0 + Rust port，crates.io，Credit: danielrab）
- ❌ `replay.nim` 解析器（**当前不需要**——`replay.nim` 只是仿真录屏，不直接服务电路优化）
- ❌ CLI 工具 + LLM 优化循环
- ❌ 修改游戏本体 / Steam Cloud 同步 / `levels.txt`（已与游戏自己维护机制冲突）

详见 `20-design/index.md`。

## 术语对照

| 术语 | 含义 |
|---|---|
| 玩家电路 (circuit) | 关卡内玩家拼出的逻辑电路 |
| 关卡 (level) | 一个 puzzle，有输入/输出 pin 与目标 |
| 存档 (save) | 玩家进度的本地持久化 |
| replay | 游戏一次会话的执行轨迹，序列化到 `replay.nim` |
| compile.dll | Nim 编译器 + LLVM 后端的 DLL，可运行时编译 `.nim` |
| simulator state | 仿真器通过 `Ptr` 暴露给嵌入代码的内存区域 |
| `Bits` / `Bytes` | save_monger 的 Nim 强类型包装（围绕 `int`）—— 二进制上就是 i64 |
| `InitialDataKind` | RAM 初始化类型枚举（`ini_zeroes` / `ini_assembler` / `ini_punch_card` / `ini_file` / `ini_hex_editor` / `ini_persistent`） |
| `.pk` 文件 | 玩家分享包（`circuit.data` + `spec.isa` + `new_program.asm` + 附属文件，Snappy 压缩） |
| Spec.isa | 架构关卡的 ISA 声明 DSL（`Stuffe/isa_spec` 完整实现已 clone） |
| ARCHITECTURE_KINDS | `{com_level_input_switched=62, com_level_output_switched=70}` —— 架构关卡只用这 2 个 kind |

## 关联资源

- 项目根目录：`B:\VS_Code_Project\turing-complete-manager`
- 游戏目录：`E:\SteamLibrary\steamapps\common\Turing Complete`
- 用户存档目录：`C:\Users\<user>\AppData\Roaming\Turing Complete`
- **Stuffe 官方仓库**（已 clone 到 `reference/`，在 .gitignore 排除）：
  - [`Stuffe/save_monger`](https://github.com/Stuffe/save_monger)（**CC0**）—— 游戏作者本人维护的官方存档读写代码，**最权威参考**
  - [`Stuffe/isa_spec`](https://github.com/Stuffe/isa_spec)（**MIT**）—— ISA 规范处理器实现
- **Rust 依赖**：[`tc_save_monger`](https://crates.io/crates/tc_save_monger) —— save_monger 的 Rust 移植版（Credit: danielrab），可直接当 Cargo 依赖
- **外部参考实现**：`B:\VS_Code_Project\turing-complete-optimizer`（`tc-save-lab`）
  - 已实现 `circuit.data` 完整 v15 读写 + v7/v13/v14 只读解码
  - 92 个主线关卡的脚手架/基线/候选目录
  - 离线组合逻辑穷举验证（语义对齐 `replay.nim`）
  - **完全离线**：零代码触碰 `compile.dll` / `replay.nim` / 游戏进程
  - 我们 manager CLI 的策略：**优先 `tc_save_monger` crate**，次选 tc-save-lab 移植

## 相关文档

- `10-investigation/architecture-levels.md`
- `10-investigation/circuit-data-format.md`
- `10-investigation/command-state.md`
- `10-investigation/component-catalog.md`
- `10-investigation/dll-analysis.md`
- `10-investigation/level-data.md`
- `10-investigation/replay-format.md`
- `10-investigation/simplex-language.md`
- `20-design/index.md`