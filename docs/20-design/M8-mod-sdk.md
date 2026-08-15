---
title: M8 · TC Mod SDK — 电路 SDK 化 + 归并计划
last_updated: 2026-08-15
scope: design
status: 设计中（2026-08-15 起；bring-over 已完成 commit 17bd042，抽 crate 待执行）
---

# M8 · TC Mod SDK — 电路 SDK 化

> **一句话定位**：把 `test/verify-cli` 分支上已跑通的「电路 codec + compile.dll 编译执行」能力，从应用内部实现抽取为一个**独立、版本化、可发布的 Rust crate**，作为第三方开发者做 mod 的接口。
>
> **心智模型**：不是 Forge 式「进程内注入」（游戏是 Nim/原生，无字节码可 hook），而是**包装游戏自己的元编译管线 + 数据格式**（见 [[game-runtime-architecture]]）。

---

## 一、三层 API 面

游戏的事实 API 由三层构成，SDK 全部覆盖：

| 层 | 内容 | 现状 |
|---|---|---|
| ① 数据格式 | `circuit.data`（v7/v13/v14/v15）、`campaign/`、`.pk`、`test.si`、`spec.isa` | 官方 `save_monger`（CC0）+ `isa_spec`（MIT）；`circuit/` 已实现 v15 读写 + v7/v13/v14 只读 |
| ② 编译执行 | `compile.dll` 的 3 导出（`NimMain`/`NimDestroyGlobals`/`compile`）+ `compile()` ABI | `dll/` 已反推并跑通（shim.dll + JIT 执行） |
| ③ 仿真运行时协议 | `replay.nim` 的 `SimulatorRequest`/`SimCommand`/`StateIndex`/`TestResult` + 内存映射状态 | `dll/test_si.rs` + `exec.rs` + `runtime.rs` 已解析并驱动 |

---

## 二、现状（已搬运，commit `17bd042`）

| 模块 | 文件数 | 依赖 | app 耦合 |
|---|---|---|---|
| `circuit/` | 6（binary/codec/legacy/model/pins/snappy） | `snap`、`serde` | 仅 `codec.rs:509` 测试用 `config::default_save_dir()` |
| `dll/` | 6（exec/gen/loader/runtime/signature/test_si） | `libloading`、`serde_json` | 仅 `runtime.rs:108` 测试里一句**多余**的 `config::default_save_dir()`（后被 `let _ =` 丢弃） |
| `game.rs` | 1 | `translations::detect_game_dir` | app 级，**留在 app**（游戏目录检测） |
| `bin/verify.rs` | 1 | `circuit` + `dll` | 消费方，改 import 指向 crate |
| `sim-shim/` | 37 | Nim + Python（构建 shim.dll + 测试夹具） | 无 |

已接线：lib.rs 加 `pub mod circuit/dll` + 5 个 Tauri 命令（`is_game_available`/`list_schematics`/`read_circuit`/`write_circuit`/`verify_circuit`）；Cargo.toml 加 `snap`/`libloading`。`cargo check --lib --bins` 通过（8 个 pre-existing warning）。

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
- `bin/verify.rs`：`turing_complete_manager_lib::circuit/dll` → `tc_mod_sdk::circuit/dll`
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
| M8-1 | 抽 crate（circuit/ + dll/ → tc-mod-sdk）+ 解耦 + path dep | 🆕 本次 |
| M8-2 | 验证：cargo check + codec round-trip + verify CLI 实测 | ❌ |
| M8-3 | 前端验证 UI section（可选，defer） | ❌ |
| M8-4 | 发布 crates.io + and_gate 起步示例 + README 许可声明 | ❌ M8+1 |

---

## 六、待决策

| 项 | 选项 | 建议 |
|---|---|---|
| crate 引入方式 | Cargo workspace vs path dependency | **path dependency**（避免 Tauri workspace 复杂度） |
| crate 发布名 | `tc-mod-sdk` / `tc-circuit-sdk` / 其他 | `tc-mod-sdk` |
| 前端 verify UI 是否纳入本期 | 纳入 / defer | defer（先抽 crate + 后端验证跑通） |

---

## 七、引用

- `docs/10-investigation/circuit-data-format.md`（codec schema + 权威源）
- `docs/10-investigation/compile-signature.md`（compile() ABI）
- `docs/10-investigation/dll-analysis.md`（compile.dll 3 导出）
- `docs/20-design/index.md`（D-1/D-7 注入机制）
- memory: [[game-runtime-architecture]] [[compile-dll-dsl-restrictions]] [[jit-calling-convention]] [[dsl-generator-test-si]]
