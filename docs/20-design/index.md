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

调研已完成（见 `10-investigation/`）。**没有任何设计文档**，也没有实现任何代码。

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

### D-3. `circuit.data` 格式逆向
- 玩家电路存档二进制格式
- 需要头几个字节：版本号
- 主体：组件列表 + 连接关系 + 位置信息
- 详见 `10-investigation/level-data.md`

### D-4. CLI 工具（`tcc`）
- 子命令：`parse-replay`、`emit-replay`、`validate-circuit`、`optimize`
- 入口：`src-tauri/src/bin/tcc.rs`（与 Tauri app 共享 `replay` 模块）
- 详见 `10-investigation/` 各文档（设计阶段再展开）

### D-5. LLM 电路优化循环
- 关卡定义 + LLM 提示 → 候选电路 → 验证 → 反馈
- 需要 LLM API key 或本地模型 endpoint
- 闭环测试：`tcc optimize --level=and_gate --max-iter=5`
- PoC 关卡建议：`and_gate` / `double_number`（简单）

### D-6. campaign 关卡定义解析
- 提取输入 pin 数、输出 pin 数、测试用例、目标功能描述
- 喂给 LLM 作为 prompt 上下文

## 推荐顺序

```
D-3 (circuit.data 逆向)  ← 先拿到数据源
   ↓
D-6 (campaign 解析)      ← 再拿到目标
   ↓
D-4 (CLI 工具)           ← 把数据源/目标串起来
   ↓
D-5 (LLM 优化循环)        ← 闭环验证
   ↓
D-1, D-2 (可选, 仅当需要)
```

D-1 与 D-2 仅在「不想走游戏 UI 自动化」路线时才必要。

## 不在范围内（明确排除）

- 修改游戏本体（exe / dll）
- 修改 Steam Cloud 同步
- 多平台支持（仅 Windows）
- 反作弊规避
- 商业用途（仅供个人学习）