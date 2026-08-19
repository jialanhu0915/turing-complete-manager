---
title: M8 · TC Mod SDK — Mod 开发者工具箱
last_updated: 2026-08-17
scope: design
status: 已校准定位（2026-08-17：mod SDK 必需含 hook 能力；当前已交付 codec + compile + runtime 三层；hook 0%，是核心缺口）
---

# M8 · TC Mod SDK — Mod 开发者工具箱

> **一句话定位**：给 Turing Complete mod 开发者的 SDK，**必须**包含数据格式、编译执行、仿真运行时协议、**hook 层**四类能力——前三者让 mod 验证自己造的电路，**hook 层让 mod 实际改写游戏行为**。前三者已部分交付；**hook 层是核心缺口**（仅做了可行性调研，代码 0%）。
>
> **心智模型**：四层能力组合成完整 mod 工具箱——
>
> | 层 | 能力 | 状态 |
> |---|---|---|
> | ① 数据格式 | circuit.data / campaign / test.si / spec.isa 读写 | ✅ 已交付 |
> | ② 编译执行 | compile.dll ABI + JIT 机器码执行 | ✅ 已交付 |
> | ③ 仿真运行时协议 | replay.nim + 内存映射状态 | ✅ 已交付（验证用例）；通用抽象待补 |
> | ④ **Hook 层** | 劫持 `compile.dll::compile` + 注册 mod 回调 | ❌ **mod 开发核心能力，0%** |
>
> ⚠️ **修正说明（2026-08-17）**：早期把 hook 层定位为"激进路线，与 SDK 并行非互斥"——**修正为"必修"**。mod 开发要在游戏编译期 / 运行时改行为，没有 hook 就不算 mod 工具箱。M9（[[rev-eng-survey]]）调研已确认 `compile.dll::compile` ABI 已知、DLL injection 可行。

---

## 一、已交付 API 面（三层）

> 本节只列**已交付的** API 面 = 三层；完整 mod SDK 框架含第四层 **hook 层**，状态见上方 §心智模型表（❌ 0%）。

| 层 | 内容 | 现状 |
|---|---|---|
| ① 数据格式 | `circuit.data`（v7/v13/v14/v15）、`campaign/`、`.pk`、`test.si`、`spec.isa` | 官方 `save_monger`（CC0）+ `isa_spec`（MIT）；`circuit/` 已实现 v15 读写 + v7/v13/v14 只读 |
| ② 编译执行 | `compile.dll` 的 3 导出（`NimMain`/`NimDestroyGlobals`/`compile`）+ `compile()` ABI | `dll/` 已反推并跑通（shim.dll + JIT 执行） |
| ③ 仿真运行时协议 | `replay.nim` 的 `SimulatorRequest`/`SimCommand`/`StateIndex`/`TestResult` + 内存映射状态 | `dll/test_si.rs` + `exec.rs` + `runtime.rs` 已解析并驱动 |

> **2026-08-17 改名记录**：本节标题原为"三层 API 面"，首句"游戏的事实 API 由三层构成，SDK 全部覆盖"。与上方 §心智模型"四层能力"重新定位后该表述产生歧义——容易被读成"SDK 只有三层"。改名为"已交付 API 面（三层）"，首句明确"已交付"边界。技术内容（表 3 行）保持原样。

---

## 二、现状（已搬运，commit `17bd042`）

| 模块 | 文件数 | 依赖 | app 耦合 |
|---|---|---|---|
| `circuit/` | 6（binary/codec/legacy/model/pins/snappy） | `snap`、`serde` | 仅 `codec.rs:509` 测试用 `config::default_save_dir()` |
| `dll/` | 6（exec/gen/loader/runtime/signature/test_si） | `libloading`、`serde_json` | 仅 `runtime.rs:108` 测试里一句**多余**的 `config::default_save_dir()`（后被 `let _ =` 丢弃） |
| `game.rs` | 1 | `translations::detect_game_dir` | app 级，**留在 app**（游戏目录检测） |
| `bin/test.rs` | 1 | `circuit` + `dll` | 消费方，改 import 指向 crate |
| `sim-shim/` | 37 | Nim + Python（构建 shim.dll + 测试夹具） | 无 |

已接线：lib.rs 加 3 个电路读写 Tauri 命令（`list_schematics`/`read_circuit`/`write_circuit`，供前端未来电路编辑器使用）；验证能力下沉到 `test.exe` CLI（独立进程），不在前端暴露。`cargo check --lib --bins` 通过（8 个 pre-existing warning）。

---

## 三、crate 抽取方案

- **crate 名**：`tc-mod-sdk`
- **位置**：repo 根 `tc-mod-sdk/`，src-tauri 用 **path dependency** 引入（**不引入 Cargo workspace**，避免 Tauri 打包的 workspace 复杂度）
- **结构**：
  ```
  tc-mod-sdk/
  ├── Cargo.toml            # name=tc-mod-sdk, deps=[snap, libloading, serde, serde_json]
  └── src/
      ├── lib.rs            # pub mod circuit; pub mod dll;
      ├── circuit/          # 从 src-tauri/src/circuit 移入
      └── dll/              # 从 src-tauri/src/dll 移入
  ```
- **公共 API**（lib.rs 重导出）：
  ```rust
  pub mod circuit;   // decode_v15/encode_v15/decode_circuit, legacy v7/v13/v14, model
  pub mod dll;       // test_si::parse, gen::generate, runtime::run_circuit_test, loader, signature
  ```

**两处测试耦合的解耦**：
1. `codec.rs:509` → 测试读真实存档改成读 `TC_SAVE_DIR` 环境变量（skip-if-unset）
2. `runtime.rs:108` → 删除多余语句（本身就是 no-op）

**app 侧改动**：
- `lib.rs`：删 `pub mod circuit; pub mod dll;`，加 `use tc_mod_sdk::circuit;`（game.rs 留在 app）
- `bin/test.rs`：`turing_complete_manager_lib::circuit/dll` → `tc_mod_sdk::circuit/dll`
- `Cargo.toml`：加 `tc-mod-sdk = { path = "../tc-mod-sdk" }`，移除 `snap`/`libloading`（移到 crate）

---

## 四、许可边界（贡献出去前必须画线）

- ✅ **复用无风险**：格式 codec 权威源 `save_monger`（**CC0**）、`isa_spec`（**MIT**），可合法重写。
- ❌ **不随 SDK 再分发** `compile.dll` / `replay.nim` / `game_engine.dll` —— 商业游戏本体。SDK 是**包装器**：运行时检测用户本机游戏目录去加载，绝不打包进 crate。
- ❌ 不改游戏 exe/dll、不绕过 Steam 校验。

---

## 五、里程碑

| 阶段 | 工作 | 状态 |
|---|---|---|
| M8-0 | 新分支 + 搬运 test/verify-cli 有用代码 | ✅ commit `17bd042` |
| M8-1 | 抽 crate（circuit/ + dll/ → tc-mod-sdk）+ 解耦 + path dep | ✅ |
| M8-2 | 验证：cargo check + codec round-trip + test CLI 实测 | ✅（and_gate / or_gate / not_gate / xor_gate / full_adder / bit_adder pass；byte_adder 已知限制） |
| M8-3 | 前端验证 UI section | 🗑️ 撤销（2026-08-15）—— 验证归入 `test.exe` CLI，不通过前端 |
| M8-4 | and_gate 起步示例 + README + byte_adder 已知限制记录 | ✅（cargo publish 暂停） |
| **M8-5** | **`tc-mod-hook` crate —— 劫持 `compile.dll::compile` 改写游戏行为** | 🟡 **PoC 完成**（2026-08-19，commit 即将提交）。Trampoline 技术已端到端验证：拦截、参数透传、卸载恢复。下一步：注入器（`inject.exe`）+ 真实游戏进程 PoC。详见 `todo/in_progress/2026-08-19-M8-5-hook-poc.md` |

---

## 六、决策

| 项 | 决策 / 状态 |
|---|---|
| crate 引入方式 | ✅ **path dependency**（避免 Tauri workspace 复杂度）—— `deab5c0` |
| crate 名 | ✅ **`tc-mod-sdk`** |
| 前端 test UI | ✅ **不纳入**（2026-08-15）：验证能力已下沉到 `test.exe` CLI；前端不再触发 —— `8e418c4` |
| **hook 层是否纳入 SDK** | ✅ **是，必须**（mod 开发的必需能力；不交付则 SDK 仅是电路自动化工具） |
| **M7（自定义关卡）归 SDK 还是 manager** | ❓ **open** —— 见 `docs/20-design/M7-custom-level-workshop.md`；与 SDK 的关系后续单独定 |

---

## 七、引用

- `docs/10-investigation/circuit-data-format.md`（codec schema + 权威源）
- `docs/10-investigation/compile-signature.md`（compile() ABI）
- `docs/10-investigation/dll-analysis.md`（compile.dll 3 导出）
- `docs/10-investigation/command-state.md`（仿真运行时内存映射状态 —— ③ 层的事实源）
- `docs/10-investigation/rev-eng-survey.md`（M9 hook 可行性调研 —— ④ 层的依据）
- `docs/20-design/M7-custom-level-workshop.md`（自定义关卡 —— 与 SDK 归类 open）
- `docs/20-design/index.md`（D-1/D-7 注入机制）
- memory: [[game-runtime-architecture]] [[compile-dll-dsl-restrictions]] [[jit-calling-convention]] [[dsl-generator-test-si]]
