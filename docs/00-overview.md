---
title: 项目总览
last_updated: 2026-08-08
scope: overview
status: 已审
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

✅ 在范围：
- 静态分析两个 DLL 的 PE 结构与导出符号
- grep/抽样分析 `replay.nim` 的 schema
- 把发现写成 wiki

❌ 不在范围：
- 实际调用 `compile.dll`（`compile` 函数签名未确定）
- 实现 `replay.nim` 解析器
- 编写 CLI
- 编写 LLM 优化循环
- 修改游戏本体
- 修改存档
- 逆向 `circuit.data` 二进制格式（仅概览）

后续功能另起计划。

## 术语对照

| 术语 | 含义 |
|---|---|
| 玩家电路 (circuit) | 关卡内玩家拼出的逻辑电路 |
| 关卡 (level) | 一个 puzzle，有输入/输出 pin 与目标 |
| 存档 (save) | 玩家进度的本地持久化 |
| replay | 游戏一次会话的执行轨迹，序列化到 `replay.nim` |
| compile.dll | Nim 编译器 + LLVM 后端的 DLL，可运行时编译 `.nim` |
| simulator state | 仿真器通过 `Ptr` 暴露给嵌入代码的内存区域 |

## 关联资源

- 项目根目录：`B:\VS_Code_Project\turing-complete-manager`
- 游戏目录：`E:\SteamLibrary\steamapps\common\Turing Complete`
- 用户存档目录：`C:\Users\<user>\AppData\Roaming\Turing Complete`

## 相关文档

- `10-investigation/dll-analysis.md`
- `10-investigation/replay-format.md`
- `10-investigation/component-catalog.md`
- `10-investigation/command-state.md`
- `10-investigation/level-data.md`
- `20-design/index.md`