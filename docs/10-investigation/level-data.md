---
title: 关卡与存档数据
last_updated: 2026-08-10
scope: investigation
status: 已审（2026-08-10 补架构关卡三件套）
---

# 关卡与存档数据

Turing Complete 在三个地方存放数据：

1. **用户存档目录** — Windows AppData，每个玩家一份
2. **游戏安装目录** — Steam 目录下的资源
3. **运行时** — 内存 + `replay.nim`

本文档盘点前两处。

---

## 一、用户存档目录

```
C:\Users\<user>\AppData\Roaming\Turing Complete\
├── levels.txt                    # 关卡进度（CSV）
├── levels_backups/               # 自动备份（每次 save_levels 时生成）
│   ├── levels_2026-08-07_170219.txt
│   └── levels_2026-08-07_170236.txt
├── schematics/                   # 玩家电路存档（每个关卡一个子目录）
│   ├── double_number/
│   │   └── 缺省/
│   │       ├── circuit.data            # 电路数据（二进制）
│   │       └── circuit_backup_0.data
│   ├── architecture/
│   │   ├── binary_search/
│   │   │   └── first/
│   │   │       ├── circuit.data
│   │   │       └── circuit_backup_0.data
│   │   ├── Overture/
│   │   │   └── binary_search/
│   │   │       └── new_program.asm     # ASM/ISA 程序（架构关卡）
│   │   └── ...
│   └── ...
├── settings.txt                  # 游戏设置（不可读，已确认）
└── steam_autocloud.vdf           # Steam 云同步标记
```

### 1.1 `levels.txt`

CSV-ish 格式，一行一关卡：

```
"level_id",<completed_bool>,"scheme_name"[,"data"]
```

实例：

```
"byte_mod",false,"",
"symphony_alu",true,"Default 1",4719&54&1|
"saving_bytes",true,"Defalut",73&5&1|
```

字段含义：
- `level_id` — 关卡 ID（必填）
- `completed_bool` — `true` / `false`
- `scheme_name` — 玩家电路方案名（可空字符串）
- `data` — 可选，每段用 `&` 分隔数字，段之间用 `|`

本项目已实现读/写（`src-tauri/src/levels.rs`）。

### 1.2 `schematics/<level>/<scheme>/circuit.data`

**玩家电路存档**。二进制格式。

抽样：

```
schematics/double_number/缺省/circuit.data          239 B
schematics/double_number/缺省/circuit_backup_0.data  238 B
```

同一关卡的 backup 只比当前小 1 字节，可能是版本号字段。

总计：
- 84 个关卡有存档
- 112 个 (level, scheme) 组合
- 112 个 `circuit.data` + 225 个 `circuit_backup_*.data`
- 大部分文件 < 1 KB，最大 5439 B

**结构骨架已识别**，完整 schema 已通过外部参考实现 [`tc-save-lab`](../../20-design/index.md) 实测验证：

- **v15**（玩家存档主流版本）：严格读写已实现
- **v7/v13/v14**（旧格式，少量遗留）：只读已实现
- 实测解码样本：`and_gate`、`not_gate`、`or_gate`、`full_adder`、`byte_adder` 等共 5 个本地存档，与 Python tc-save-lab 输出字段完全一致

详见 [`circuit-data-format.md`](circuit-data-format.md)。

### 1.3 `schematics/architecture/<level>/<scheme>/*.asm` + `*.isa`

架构（Architecture）类关卡保存 **汇编程序 + ISA 声明**（`.asm` + `.isa` 文件），不是电路图本身。完整的架构关卡由**三件套**构成：

- `*.asm` — 用自定义 ISA 写的程序
- `*.isa` — ISA 声明文件（架构名、寄存器、指令字段、操作码）
- `circuit.data` — 该 ISA 对应的 CPU 电路（用基础组件搭出的门级实现）

例：`schematics/architecture/Overture/binary_search/new_program.asm`（配套 `.isa` 应位于同目录）

本项目**不优化**架构关卡的电路生成，但**备份逻辑应包含 `.isa` 文件**——否则恢复后会丢失 ISA 定义，`.asm` 无法编译。

### 1.4 `settings.txt`

游戏设置。文档中描述为不可读（用户确认）。

文件大小约 4.8 KB。

### 1.5 `steam_autocloud.vdf`

Steam 云同步标记，由 Steam 客户端管理，本项目不读不写。

---

## 二、游戏安装目录

```
E:\SteamLibrary\steamapps\common\Turing Complete\
├── Turing Complete.exe            # 15.8 MB，Godot 主程序
├── compile.dll                    # 1.78 MB，Nim 编译器（已单独分析）
├── game_engine.dll                # 1.99 MB，Godot 引擎包装（已单独分析）
├── replay.nim                     # 79 MB，运行时重放脚本（已单独分析）
├── libgcc_s_seh-1.dll            # 83 KB，GCC 运行时
├── libwinpthread-1.dll           # 55 KB，pthread 库
├── soft_oal.dll                   # 2.1 MB，OpenAL 软实现（音频）
├── steam_api64.dll                # 309 KB，Steamworks SDK
├── asset/                         # 游戏内图片、字体、声音等
├── campaign/                      # 关卡定义（88 个子目录 + .png 资源）
├── godot/                         # Godot 引擎资源
└── translations/                  # i18n 翻译
```

### 2.1 `asset/`

游戏内静态资源：

- 字体（OpenType）
- 图片（关卡背景、UI 图标）
- 音效、音乐

本项目**不需要**访问这些。

### 2.2 `campaign/`

**关卡定义**。每个子目录对应一个关卡，名字与 `levels.txt` 中的 `level_id` 一致。

样例：
```
campaign/
├── always_on/
├── and_gate/
├── and_gate_3/
├── any_doubles/
├── assembly_programming/
├── binary_programming/
├── binary_racer/
├── binary_search/
├── bit_30_off.cvd.png        # 关卡视觉资源（Color Vision Deficiency 友好）
├── bit_30_off.png
├── bit_30_on.png
├── bit_30_z.png
├── bit_40_off.cvd.png
├── bit_40_off.png
├── bit_40_on.png
├── bit_40_z.png
├── bit_adder/
├── bit_inverter/
├── bit_switch/
├── byte_adder/
└── ...
```

88 个关卡子目录，加上几百个 `.png` / `.cvd.png` 视觉资源。

**关卡定义格式未逆向**。猜测：
- 子目录里有关卡描述文件（JSON / 二进制 / 自定义格式）
- `.png` 是关卡插画

> 留作后续工作：逆向 `campaign/<level_id>/` 内的关卡定义文件，得到「输入 pin 数 / 输出 pin 数 / 测试用例」等元数据。

### 2.3 `godot/`

Godot 引擎的 Pck 文件（资源包）。本项目不需要访问。

### 2.4 `translations/`

游戏翻译文件。Tauri 应用可借鉴其中的关卡名翻译（已在 `src-tauri/src/translations.rs` 中使用部分）。

---

## 三、运行时数据

### 3.1 `replay.nim`

已在 `replay-format.md` 中详述。

关键事实：每次玩家操作都重写整个文件，所以它反映了「从游戏开始到当前时刻」的所有操作历史。

### 3.2 内存中的仿真状态

通过 `Ptr` 在 `compile.dll` 编译出的 Nim 代码里暴露：
- `commands` / `settings` — 控制 + 状态
- `input_replay` — 测试输入序列
- `output_history_pins` — 输出引脚历史
- `error_buffer` / `ui_buffer` — 错误与 UI 状态

详细字段见 `command-state.md`。

---

## 四、数据流图

```
  关卡定义（campaign/<id>/）            玩家电路存档（schematics/<id>/<scheme>/circuit.data）
            │                                          │
            ▼                                          ▼
       ┌─────────────────────────────────────────────────────┐
       │                  游戏运行时                         │
       │   ┌─────────────┐       ┌──────────────────┐       │
       │   │   Godot     │ ◀───▶ │   compile.dll    │       │
       │   │   主程序     │       │   (Nim 编译器)   │       │
       │   │             │       │                  │       │
       │   │   读取       │       │   编译 + 执行    │       │
       │   │   circuit   │       │   replay.nim     │       │
       │   │   .data     │       │                  │       │
       │   └─────────────┘       └──────────────────┘       │
       │            │                       │                │
       │            ▼                       ▼                │
       │      仿真状态 (内存)         ┌──────────┐          │
       │      input_replay /          │ levels.txt│         │
       │      output_history_pins    │  (写通关) │          │
       │                             └──────────┘          │
       └─────────────────────────────────────────────────────┘
                          │
                          ▼
                 用户存档目录
                 (AppData\Roaming\Turing Complete\)
```

---

## 五、当前项目做了什么

| 数据 | 操作 | 文件 |
|---|---|---|
| `levels.txt` | 读、改、备份 | `src-tauri/src/levels.rs` |
| `levels_backups/` | 自动创建、按行号替换第二列 | `src-tauri/src/levels.rs` |
| `schematics/` | 整目录 zip 备份/恢复 | `src-tauri/src/backup.rs` |
| `settings.txt` | ❌ 未处理 | — |
| `circuit.data` | ❌ 未解析（仅整文件备份） | — |
| `campaign/` | ❌ 未读取 | — |
| `compile.dll` | ❌ 未调用 | — |
| `replay.nim` | ❌ 未解析 | — |
| `translations/` | 部分（关卡名翻译） | `src-tauri/src/translations.rs` |

---

## 六、对后续工作的影响

要做 LLM 电路优化，需要：

1. **逆向 `circuit.data`** —— 拿到电路拓扑数据，让 LLM 能生成
2. **逆向 `campaign/<id>/`** —— 拿到关卡定义（输入/输出 pin、测试用例）
3. **找到注入点** —— 把新电路写回 `circuit.data` 后驱动游戏跑仿真

如果只做离线批量：
- 解析 `replay.nim` 也行（已有部分信息：`#COUNTS` 数组、UI 状态）
- 但丢失电路拓扑信息，无法做有意义的优化

**推荐路径**：
1. 先逆向 `circuit.data` 拿到拓扑
2. 同时逆向 `campaign/<id>/` 拿到目标
3. 再考虑是否需要 `compile.dll` 调用（可能不需要，直接操作文件 + 游戏即可）