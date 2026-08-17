---
title: 分支状态盘点 + 战略决策（merge / 继续 / 搁置）
date: 2026-08-11
status: done
resolved: 2026-08-17
decision: A (合并收尾)
---

# 分支状态盘点 + 战略决策（merge / 继续 / 搁置）

## 三个分支的事实

| 分支 | HEAD | 内容 | 备注 |
|---|---|---|---|
| `master` | `0831825` | M1–M6 全部 + 调研文档 + tc_save_monger revert | 干净发布分支，零 codec / DLL 代码 |
| `test/verify-cli` | `e48fd62` | M1–M5 + circuit codec + DLL 集成 + verify UI + sim-shim 工具 + 10 关夹具 | **真正的干活分支**；M6 角色皮肤未实现（test/verify-cli 在 `5ba899f` 分叉，早于 M6 开发） |
| `verify/tc-save-monger` | (旧) | 失败的 Rust crate 尝试（v6-only panic） | 已打 `verify-attempt-failed-2026-08-11` tag，仅作历史保留 |

> 分叉时间线：master 在 `5ba899f`（替换图标）后继续往 character M6 + tc_save_monger 走；test/verify-cli 在 `5ba899f` 后继续往调研 + codec + DLL 集成走。`32e2fa2` 是 `verify/tc-save-monger` 的分叉点（master 上 tc_save_monger 集成 commit），与 test/verify-cli **互相不影响**。

## test/verify-cli 已完成的工作（实测可跑）

### `src-tauri/src/circuit/`（~2200 行 Rust）

- `codec.rs`（535）— v15 完整读写
- `legacy.rs`（421）— v7 / v13 / v14 只读
- `binary.rs`（327）— 小端 binary 原语
- `snappy.rs`（63）— Snappy 压缩/解压
- `model.rs`（218）— 数据结构
- `pins.rs`（605）— 连通性解析（端口自 tc-save-lab Python）

### `src-tauri/src/dll/`（~2800 行 Rust）

- `signature.rs`（67）— compile 函数 ABI 反推（4 参数 + 40 字节输出）
- `loader.rs`（180）— shim.dll 加载（3 条 fallback 路径）
- `runtime.rs`（296）— JIT 机器码执行
- `exec.rs`（802）— 驱动执行 + win race 修复
- `gen.rs`（896）— DSL 生成器（静态骨架 + 测试模板 + I/O 接线 + kind 112 z-flag 探针）
- `test_si.rs`（505）— 解析 `campaign/<level>/test.si` → LevelTemplate

### `src-tauri/src/`

- `bin/verify.rs`（155）— 进程隔离验证器 CLI（每次 spawn 新进程绕过 compile.dll 单次限制）
- `game.rs`（89）— 游戏检测（compile.dll + campaign/ 完整安装）
- `lib.rs` — 新增 `list_schematics` / `read_circuit` / `write_circuit` / `verify_circuit` / `is_game_available` 命令

### `sim-shim/`（Nim + Python 工具链）

- `shim.nim`（123）— Nim C-ABI shim.dll 包装 `compile.dll::compile`
- `prefix.dsl`（1506）— DSL preamble
- `convert_dialect.py` / `extract_dsl.py` — DSL 转换/提取工具
- `fixtures/` — 10 个关卡的 `circuit.data` + `hint_solution.data`
- `fixtures/test_si/` — 9 个 `.si` 文件

### 前端

- `index.html` + `main.ts` + `styles.css` + `i18n.ts` — 电路验证 section（关卡下拉 + 方案表 + 验证按钮 + pass/fail 染色）

### 实测通过

- **正例 pass**：`and_gate` / `or_gate` / `not_gate` / `xor_gate` / `nor_gate` / `bit_adder` / `byte_not` / `byte_nand` / `byte_xor`（含 hint 方案）
- **反例 fail**：`byte_xor Default`（坏电路）、断开输出导线——证明判别力
- 已知 kind 112 z-flag 探针已支持（最新 commit）

## master 与 test/verify-cli 的关键差异

| 能力 | master | test/verify-cli |
|---|---|---|
| M1–M5 备份/等级/翻译/i18n | ✅ | ✅ |
| M6 角色皮肤 | ✅（master 后续开发） | ❌（分叉早于 M6，未实现；合并时无冲突，master 自然带入） |
| v15 codec 读/写 | ❌ | ✅ |
| compile.dll 集成 | ❌ | ✅ |
| 验证 UI section | ❌ | ✅ |
| `verify.exe` / `shim.dll` 打包 | ❌ | ✅ |
| WiX 安装器（含 shim.dll） | ❌ | ✅ |

## 文档与代码的错位

master 上的 `docs/10-investigation/circuit-data-format.md` 和 `docs/20-design/index.md` 写"调研沉淀 + D-1 / D-3 / D-7 / D-5 待办"。**但 D-1 / D-7 在 test/verify-cli 上已经实现并验证**。

master 文档是过时的：

1. "Port v15.nim → Rust"被列为 open work —— **已完成**（test/verify-cli's `circuit/codec.rs`）
2. "compile.dll ABI 调研"被列为 open work —— **已完成**（`signature.rs` + shim.dll）
3. "端到端验证"被列为 open work —— **已完成**（3 正例 + 2 反例 + 后续扩展）

> 任何"调研性 docs"应该在合 master 时同步更新；否则合完后文档与代码又错位。

## 战略决策点

**核心问题：这个功能（小众的电路上层自动化 + LLM 优化基础）值不值得推到 master + 发版？**

| 方向 | 含义 | 适合情况 |
|---|---|---|
| **A. 合并收尾** | 把 test/verify-cli 合到 master（M6 角色皮肤 master 自带，无冲突）；docs 同步；发 v0.4.0 | 自用 / 社区用 / 作品集；已能跑通 |
| **B. 继续往 LLM 闭环推** | 在 test/verify-cli 上加 LLM 集成 + PoC 关卡（`and_gate` / `not_gate`） | 学习 / 研究 / 技术展示；不在乎用户基数 |
| **C. 搁置** | master 维持现状，test/verify-cli 标 abandoned tag（参考实现已存档） | 不打算发版 / 维护成本高于价值 |

## 客观评估（不替用户决定）

**功能小众程度**：

- 仅服务 Turing Complete 玩家 + 想用 LLM 优化电路的小群体
- 替代方案：玩家手动解题（游戏设计本身）
- 用户基数：游戏 modder / 玩家社区规模有限

**价值不在用户基数，而在**：

- 学习价值高（DLL ABI / JIT / DSL 生成 / 跨进程游戏驱动）
- 技术含量罕见（能驱动游戏本体的工具不多）
- 工作已完成 23 commits / ~5000 行 Rust / ~500 行 Nim+Python
- 维护成本：游戏本体更新不频繁，维护负担可控

## 待用户决策

- [x] ~~用户从 A / B / C 中选一个（或组合）~~ → **选 A**
- [x] ~~决策后，把本文件移入 `todo/completed/`，按决策内容新建对应的 `todo/in_progress/` 任务~~

---

## 结果（2026-08-17 归档）

**决策 A：合并收尾。** 已于 2026-08-15 前后完成实质执行（今天才形式归档）：

### 合并落地

`test/verify-cli` → `master` 共 **38 个 commit** fast-forward，全部已 push 到 `origin` (GitHub) 和 `gitee`。当前 HEAD：

```
1e847e4 (HEAD -> master, origin/master, gitee/master, gitee/HEAD)
         chore: add .editorconfig to lock LF end-of-line at editor level
c409dd2  chore: add .gitattributes to lock LF line endings
8e418c4  refactor(ui,tauri): 撤前端电路测试视图 + test_circuit 命令，验证归入 test.exe CLI
879df15  docs(sdk): M9 静态逆向调研（hook 可行性评估）+ SDK README + and_gate 示例
         + byte_adder 已知限制
1b22b56  fix(dll): 输出字段按 word_size 排序映射
6db5116  feat(circuit): 字输入/输出支持位级连接
209b6a3  fix(ui): 电路测试下拉框/结果深色模式适配
4371bcd  fix(dll): 字符串 switch 的 case 也加 case 前缀
982a560  fix(tauri): default-run
c8071e4  feat(ui): 电路测试视图 + test_circuit + compile.dll 查找路径 + shim.dll 打包资源
8abb43a  refactor: 重命名 verify → test
dc49f55  fix(dll): DSL switch case 加 case 前缀（对齐 2026-08-11 compile.dll 方言）
9cd53f5  docs(design): M8 mod SDK 设计
deab5c0  refactor(sdk): 抽 circuit/dll 为独立 tc-mod-sdk crate
17bd042  feat(circuit): 搬运 test/verify-cli 电路 SDK 代码
         (circuit codec + dll 编译执行 + verify CLI + sim-shim 工具链)
... (更早 23 commits: M8 SDK codec/dll 实现、sim-shim 工具链、fixtures 等)
```

### 主线副作用（决策时未明确，落地时一并清理）

- **`verify` → `test` 重命名**（`8abb43a`）：跑关卡测试语义更准确，与 M7 import 校验区分；前端电路测试 UI 也撤了，验证入口收敛到 `test.exe` CLI
- **DSL 方言对齐**（`dc49f55`、`4371bcd`）：compile.dll 2026-08-11 更新后 switch case 需 `case` 前缀；同步修了 `gen.rs` 4 处 + 字符串 switch 模板
- **M9 静态逆向**（`879df15`）：从最初的"D-7 注入机制 = 选项 B"延伸到评估 hook 可行性，并入 SDK README
- **`.gitattributes` + `.editorconfig`**（`c409dd2`、`1e847e4`）：合并过程中发现的 22496/22496 行 CRLF/LF 噪声，处理掉并锁住 LF

### 后续衍生 / 待跟进

- **byte_adder 连通性 bug**（仍在 `todo/planning/2026-08-15-byte-adder-connectivity.md`，`status: known-limitation`）—— 合并后的真实剩余 bug，不阻塞主线
- **v0.4.0 发版** —— 决策文本说"发 v0.4.0"，但当前 Cargo.lock 还是 0.3.0（gitee 上 master HEAD 之前停在 `8b4f350 fix(tauri): 同步 Cargo.lock 版本 0.2.0 → 0.3.0`）。发版本身属于 housekeeping，需要时单独开 `todo/in_progress/` 任务
- **D-5 LLM 闭环** —— 决策 A 没做，未来想做再开新分支

### 相关

- `docs/20-design/M8-mod-sdk.md`
- `tc-mod-sdk/README.md`（Known limitations）
- memory: [[branch-strategy-2026-08-11]] [[test-verify-cli-merge]]