---
title: 设计方向占位
last_updated: 2026-08-08
scope: design
status: 占位
---

# 设计方向占位

> ⚠️ **本次 wiki 不包含任何设计内容**。
> 本目录保留作为后续设计文档的位置。

## 当前状态

调研第一轮已完成（见 `10-investigation/`）。**circuit.data 的结构骨架已识别**（magic byte、固定头/尾、ASCII 标签），完整 schema 未完成。

**没有任何设计文档**，也没有实现任何代码。

## 后续可能的设计方向

以下项目**不在本次范围内**，留作未来独立计划：

### D-1. `compile.dll` 调用 ABI 集成
- 在 Windows 上通过 `libloading` LoadLibrary
- 调用 `NimMain()` 初始化，再 `compile(...)` 执行 replay.nim
- 需要先 IDA/Ghidra 静态分析 `compile` 函数签名
- 可能需要写 thin C shim 规避 Nim GC ABI 风险
- 详见 `10-investigation/dll-analysis.md`「后续建议」节

### D-2. `replay.nim` 解析器
- 手写 Nim 词法分析器（不需要完整 parser）
- 识别：`^var |^type Enum\[|^actions\.add\(` 这几种开头
- 输出结构化 `Circuit { components, actions, ui }` 对象
- 单元测试：parse → emit → parse 应该字段相等（round-trip）
- 详见 `10-investigation/replay-format.md`

### D-3. `circuit.data` 完整 schema 逆向（W-1）
- 玩家电路存档二进制格式
- **当前状态**：骨架已识别（见 `circuit-data-format.md`），完整 schema 未完成
- 待做：
  - W-1 完整 schema 逆向（推荐 IDA/Ghidra 反编译 compile.dll）
  - W-2 确认 byte 0x10 = 首块类型
  - W-3 写回合法性测试
  - W-4 提供 Python 读写库
- 详见 `10-investigation/circuit-data-format.md`「待续工作」节

### D-4. CLI 工具（`tcc`）
- 子命令：`validate-circuit`、`inspect-level`、`optimize`（按当前 CLI 目标精简）
- 入口：`src-tauri/src/bin/tcc.rs`（与 Tauri app 共享模块）
- **核心子命令**：`validate-circuit --level=<id> --circuit=<path>` —— 用游戏本体验证电路
- 详见 `10-investigation/` 各文档（设计阶段再展开）

### D-5. LLM 电路优化循环
- **目标**：LLM 生成电路 → 写到新 scheme 文件夹 → 游戏本体验证 → 反馈 LLM
- **存档隔离**：每轮创建 `schematics/<level>/optimize-NNN/` 子目录，失败删除
- 需要 LLM API key 或本地模型 endpoint
- 闭环测试：`tcc optimize --level=and_gate --max-iter=5`
- PoC 关卡建议：`and_gate` / `not_gate` / `or_gate`（最简单）

### D-6. campaign 关卡定义解析
- 提取输入 pin 数、输出 pin 数、测试用例、目标功能描述
- 喂给 LLM 作为 prompt 上下文
- 仍未逆向

### D-7. 注入机制（如何让游戏加载指定方案）
- 选项 A：改 `levels.txt` 第 3 列（**已排除**——游戏自己维护，干扰有风险）
- 选项 B：通过 `compile.dll` 直接调用（D-1 完成后才能走）
- 选项 C：游戏 UI 自动化（最脆弱，最后手段）

## 推荐顺序

```
W-1 (circuit.data schema)  ← 拿到电路读写能力
   ↓
D-6 (campaign 解析)        ← 拿到关卡定义
   ↓
D-7 (注入机制选定)          ← 决定如何驱动游戏
   ↓
D-4 (CLI 工具)             ← 把能力串起来
   ↓
D-5 (LLM 优化循环)          ← 闭环验证
   ↓
D-1, D-2 (可选)
```

## CLI 核心工作流（共识）

```
schematics/<level>/
├── 缺省/                    ← 玩家原方案
│   └── circuit.data
├── optimize-001/            ← 候选 1
│   └── circuit.data
├── optimize-002/            ← 候选 2
│   └── circuit.data
└── optimize-003/            ← 失败，已删除
```

每轮优化：
1. 备份（已实现，M5-1 自动备份）
2. 创建 `optimize-NNN/circuit.data`
3. 让游戏加载这个方案跑测试（D-7 选定后）
4. **fail → 直接删除文件夹**，玩家存档零接触
5. **pass → 保留**，让玩家/LLM 决定是否覆盖原方案

## 不在范围内（明确排除）

- 修改游戏本体（exe / dll）
- 修改 Steam Cloud 同步
- 修改 `levels.txt`（游戏自己维护）
- 多平台支持（仅 Windows）
- 反作弊规避
- 商业用途（仅供个人学习）