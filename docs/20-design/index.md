---
title: 设计方向（基于 Stuffe 官方仓库 + tc-save-lab 参考）
last_updated: 2026-08-10
scope: design
status: 已重写（Stuffe/save_monger + tc_save_monger crate 路线）
---

# 设计方向（基于 Stuffe 官方仓库 + tc-save-lab 参考）

> ⚠️ **本次 wiki 不包含任何设计内容**。
> 本目录保留作为后续设计文档的位置。
>
> 2026-08-08 重大重新评估：发现 `B:\VS_Code_Project\turing-complete-optimizer\` 项目（`tc-save-lab`）已实现了**大部分离线读写能力**。本设计需相应调整。

## 当前状态

调研完成 + 外部参考实现验证完成：

| 调研项 | 状态 | 来源 |
|---|---|---|
| `circuit.data` schema（v15） | ✅ 已知 | `tc-save-lab/codec.py` 实测可读写 |
| `circuit.data` schema（v7/v13/v14） | ✅ 已知（只读） | `tc-save-lab/legacy_codec.py` |
| `circuit.data` schema（v0..v15 全版本） | ✅ 已知 | [`Stuffe/save_monger`](https://github.com/Stuffe/save_monger)（**CC0，Stuffe 是游戏作者本人**） |
| Spec.isa BNF 与解析器 | ✅ 已知 | [`Stuffe/isa_spec`](https://github.com/Stuffe/isa_spec)（MIT） |
| `tc_save_monger` Rust crate | ✅ 可用 | [crates.io](https://crates.io/crates/tc_save_monger)（Credit: danielrab） |
| 完整 ComponentKind 枚举（125 slot） | ✅ 已知 | `Stuffe/save_monger/common.nim` |
| `ARCHITECTURE_KINDS = {62, 70}` | ✅ 已知 | `Stuffe/save_monger/common.nim` 常量定义 |
| 关卡脚手架（I/O pin 定义） | ✅ 已知 | `tc-save-lab/scaffold.py` |
| 离线组合逻辑穷举验证 | ✅ 已知 | `tc-save-lab/simulate.py` + `vector_sim.py` |
| 原子写回 + 安全检查 | ✅ 已知 | `tc-save-lab/direct_install.py` + `foundry.py` |
| Windows 文件锁重试模式 | ✅ 已知 | `Stuffe/save_monger`（`open_when_ready` / `add_file` / `get_file_and_store` 三处） |
| `compile.dll` 实际调用 | ❌ 未做 | tc-save-lab + save_monger **均不提供**（完全离线） |
| LLM 集成 | ❌ 未做 | — |

**核心结论**：Stuffe 是游戏作者，`Stuffe/save_monger`（CC0）是**官方 codec**，[`tc_save_monger`](https://crates.io/crates/tc_save_monger) Rust crate **可直接依赖**，省数月逆向实现工作量。tc-save-lab（Python 逆向）降级为参考。tc-save-lab 严格离线，**零代码触碰 `compile.dll` / `replay.nim` / 游戏进程**——这是我们 manager CLI 需要补的增量。

---

## 参考实现层次

```
            Stuffe/save_monger (CC0)              tc-save-lab (Python 逆向)
            ────────────────────                  ──────────────────────
读写 circuit.data  ✅ v0..v15 全版本                ✅ v15 严格 + v7/v13/v14 只读
读 campaign       ✅ 同上                         ✅ 同上（独立演化）
提取关卡 I/O       ✅ 常量定义（ARCHITECTURE_KINDS ✅ scaffold.py
                   等见 common.nim）
离线仿真          ✅ （state_to_binary 测试用）    ✅ simulate.py + vector_sim.py
安全写回          ✅ Windows 文件锁重试模式        ✅ atomic + game-running
官方权威性         ✅ Stuffe 本人                   ❌ 第三方逆向
Rust crate          ✅ tc_save_monger (crates.io)  ❌ 只有 Python

                                  ↓ 都可作为依赖

                            manager CLI (要做的)
                            ───────────────────
读写 circuit.data    ✅ 直接用 tc_save_monger crate（无需移植）
读 campaign        ✅ 直接用 tc_save_monger crate
提取关卡 I/O       ✅ 直接用 crate 的常量
离线仿真           ✅ 暂时用 crate + Python fallback

驱动游戏本体        ❌ tc-save-lab + save_monger 都不做   🆕 必须做（D-7 + D-1）
注入电路到游戏      ❌ 同上                                🆕 必须做
读测试结果         ❌ 同上                                🆕 必须做
LLM 集成           ❌ 同上                                🆕（D-5，可后置）
```

**manager CLI 相对于现有参考的本质增量：把电路从"离线写文件"升级为"游戏本体验证"**。

---

## 现状重新盘点：每个 D 项目现在是什么

### D-1. `compile.dll` 调用 ABI 集成 → **变成核心工作**

- tc-save-lab 完全不碰 `compile.dll`
- 我们**必须**自己写 `ctypes.WinDLL` 包装层（或 Rust `libloading`）
- 工作量：先做 IDA/Ghidra 字符串提取 + 试探调用，确定 `compile()` 函数签名
- 与 D-7 的关系：D-7 的"选项 B"（DLL 直接调用）= D-1 的实现
- 详见 `10-investigation/dll-analysis.md`

### D-2. `replay.nim` 解析器 → **降级为可选**

- tc-save-lab 没做（也不需要——他们只关心电路文件本身）
- 我们**目前不需要**完整 parser——我们关心的是测试结果，不是 replay 历史
- 若后续要做"历史 replay 重放"才需要做

### ~~D-3. circuit.data 完整 schema 逆向~~ → ✅ 已被 Stuffe/save_monger 完成

- W-1 不再是 open question
- **官方源**：[`Stuffe/save_monger`](https://github.com/Stuffe/save_monger)（CC0，Stuffe 是游戏作者本人）
- 含 v0..v15 全版本实现 + 16 个版本分发逻辑
- **推荐路径**：直接依赖 [`tc_save_monger`](https://crates.io/crates/tc_save_monger) Rust crate（CC0 + Rust port），不再需要 Python→Rust 移植
- 详见 `10-investigation/circuit-data-format.md` §Stuffe/save_monger

### D-4. CLI 工具（`tcc`）→ **形态需要明确**

- tc-save-lab 已经有完整的 CLI（`tc-save`），15 个 subcommand
- 但**全部离线**——没有"驱动游戏"子命令
- 我们的 CLI = tc-save-lab 的子集 + **新增 `validate` 子命令**（驱动游戏）
- 入口：`src-tauri/src/bin/tcc.rs`（与 Tauri app 共享 codec 模块）
- 关键设计决策：要不要直接 `cargo install tc-save-lab` 当依赖，还是抄实现？

### D-5. LLM 电路优化循环 → **后续工作**

- 闭环：`LLM 生成 → 写到新 scheme → 游戏验证 → 反馈 LLM`
- PoC 关卡：`and_gate` / `not_gate` / `or_gate`（最简单）
- 与 D-4 的关系：调 D-4 的 `validate` 子命令
- 需要 LLM API key 或本地 endpoint

### ~~D-6. campaign 关卡定义解析~~ → ✅ 已被 tc-save-lab 完成

- `tc-save-lab/scaffold.py` 已实现完整提取
- 关键常量（直接复用）：
  ```python
  LEVEL_INPUT_KINDS  = frozenset({60, 61, 62, 63, 64, 65, 106})
  LEVEL_OUTPUT_KINDS = frozenset({40, 58, 68, 69, 70, 73, 74, 75, 77})
  CUSTOM_COMPONENT_KIND = 78
  ```
- 产物：`examples/<level>/scaffold/immutable.json`（92 个主线关卡已有）
- 当前 campaign 格式版本分布（来自 `tc-save-lab` 的 scaffolds.json 摘要）：
  - **v7=15、v13=18、v14=57、v15=2** — 不是单一版本！必须 4 个 codec 都有
  - **406 个 immutable 组件**、**15 个关卡无 scaffold**（架构/编程关卡）

### D-7. 注入机制 → **核心未决问题**

让游戏加载我们写的电路，并跑测试拿结果。三条路：

| 选项 | 复杂度 | 可靠性 | 玩家存档风险 | 备注 |
|---|---|---|---|---|
| **A. 改 `levels.txt` 第 3 列** | 低 | 中 | 中 | 已排除——游戏自己维护 |
| **B. `compile.dll` 直接调用** | 中-高 | 高 | 低 | **推荐**——但要先解 D-1 |
| **C. 游戏 UI 自动化** | 高 | 低 | 中 | 最后手段 |

**当前推荐 B**——但需要先把 D-1 的 `compile()` 函数签名搞清楚。

---

## 新推荐顺序（基于 Stuffe 官方仓库 + tc-save-lab）

```
✅ W-1 (circuit.data v15 schema)        ← Stuffe/save_monger + tc_save_monger crate 已完成
✅ W-2 (Rust 移植 codec)                ← 已废止，改用 tc_save_monger crate
✅ D-6 (campaign 解析)                  ← Stuffe/save_monger + tc-save-lab 已完成
   ↓
🆕 Cargo.toml: 加 `tc_save_monger = "0.4.5"`  ← MIT 直接用（省 W-2）
   ↓
D-1 (compile.dll 函数签名调研)         ← IDA/Ghidra strings + 试探调用
   ↓
D-7 (注入机制选定 = 选项 B)             ← DLL 直接调用
   ↓
D-4 (CLI 工具)                          ← 串起来
   ↓
D-5 (LLM 优化循环)                     ← 闭环验证
   ↓
D-2 (replay.nim 解析) — 可选, 仅当需要
```

---

## CLI 核心工作流（保留）

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
2. 创建 `optimize-NNN/circuit.data`（用 tc-save-lab 的 codec 写）
3. 让游戏加载 + 跑测试（D-7 = 选项 B）
4. 读 `sim_test_result` 槽（已知 memory layout）
5. **fail → 直接删除文件夹**，玩家存档零接触
6. **pass → 保留**，让玩家/LLM 决定是否覆盖原方案

**安全检查**（直接抄 tc-save-lab 的 `direct_install.py` 模式）：
- `tasklist /FI "IMAGENAME eq Turing Complete.exe"` 检查游戏是否运行
- 计划-然后-写：先生成 SHA 校验计划，写前再校验
- 原子写：写到 `.tcc.tmp` → fsync → `os.replace`

---

## 可借鉴的安全 / 工程模式

### 来自 tc-save-lab

| 模式 | 文件 | 我们怎么用 |
|---|---|---|
| 游戏进程检测 | `foundry.py:771-796` (`_assert_game_not_running`) | `tcc validate` 前调用 |
| 原子替换 | `storage.py:156-213` (`atomic_replace_circuit`) | CLI 默认写盘方式 |
| 计划-写-校验 | `direct_install.py:489-712` | D-5 闭环的每轮基线 |
| reparse-point 防御 | `direct_install.py` | 防 symlink 攻击 |
| SHA256 重校验 | 多处 | 写前后比对 |

### 来自 Stuffe/save_monger

| 模式 | 文件 | 我们怎么用 |
|---|---|---|
| **Windows 文件锁重试** | `save_monger.nim:8-19` (`open_when_ready`)、`pk_versions/common.nim:53-77` (`add_file`)、`pk_versions/v0.nim:75-80` (`get_file_and_store`) | `src-tauri/src/backup.rs` 必须有同样模式——retry + sleep(1ms) |
| **`.pk` 文件 random custom_id remapping** | `pk_versions/v0.nim:49-58` | 防 ID 冲突：导入分享包时自动 remap custom_id |
| **Snappy 最大解压尺寸** | `pk_versions/v0.nim:10` (`MAX_UNCOMPRESSED_SIZE = 100_000_000`) | Rust crate 也应有同样安全限制 |

---

## 不在范围内（明确排除）

- 修改游戏本体（exe / dll）
- 修改 Steam Cloud 同步
- 修改 `levels.txt`（游戏自己维护，已与用户确认）
- 多平台支持（仅 Windows）
- 反作弊规避
- 商业用途（仅供个人学习）
- **tc-save-lab 自身代码的强制重写**——优先采用或移植，不从零写

---

## 与外部实现的协作策略

### 优先级

1. **首选**：[`tc_save_monger`](https://crates.io/crates/tc_save_monger) Rust crate —— CC0 + 已存在，比任何移植方案都高效
2. **次选**：直接读 `reference/save_monger/`（Stuffe/save_monger 已 clone 到本地）源码作为权威参考
3. **第三选**：仅当 crate 不可用时，照 `Stuffe/save_monger` 的 Nim 代码移植

### tc-save-lab 角色（降级）

- 它是 Python 第三方逆向项目，独立演化
- 我们需要 Rust 实现（Tauri app 共享），它只有 Python
- 用法：作为算法验证与 scaffold.py 算法的参考（LEVEL_INPUT_KINDS 等常量照搬）

### 我们**新写的代码**只聚焦在 tc-save-lab + save_monger 都不提供的部分

- 游戏本体调用（D-1）
- 注入机制（D-7）
- 测试结果读取
- LLM 集成（D-5）