---
title: 关卡与存档数据
last_updated: 2026-08-11
scope: investigation
status: 已审（2026-08-11 实测 campaign 目录：98 关卡；hint 多步提示系统补全）
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

> ⚠️ **安全警告（wiki 校对）**：`settings.txt` 包含**个性化令牌**，可用于干扰 Steam 个人资料。**不应在公共场合分享包含此文件的存档备份**——若要分享，仅分享 `schematics/` 目录（或更窄：仅特定原理图）。

### 1.5 `steam_autocloud.vdf`

Steam 云同步标记，由 Steam 客户端管理，本项目不读不写。

> ⚠️ **恢复冲突（wiki 校对）**：Steam 云同步会**干扰手动恢复备份**——Steam 会尝试恢复到它最后已知的版本。建议恢复流程：
> 1. 关闭 Steam 云同步
> 2. 关闭游戏
> 3. **用备份覆盖存档目录**（最好先删当前目录以避免残留）
> 4. 启动游戏 → Steam 显示"云冲突" → 选**"本地存档"**上传恢复的数据
> 5. 重新打开云同步
>
> 这一流程对本项目 `backup.rs` 的用户提示有直接影响：恢复向导应警告用户先关闭云同步。

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
├── campaign/                      # 关卡定义（98 个子目录 + .png 资源）
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

**关卡定义**。每个子目录对应一个关卡，目录名（snake_case）与 `levels.txt` 中的 `level_id` 一致。

**Stuffe 是游戏开发者**（同时维护 `tc_*` 系列仓库），曾公开 `Stuffe/tc_campaign` 作为关卡数据仓库——该仓库**当前不可访问**（已归档/迁移），不作为参考源。

**本地访问**：游戏安装目录下自带 `campaign/` 目录（`E:\SteamLibrary\steamapps\common\Turing Complete\campaign\`），含**全部 98 个关卡**的完整定义文件（**2026-08-11 实测**，非 88）。**调查 campaign 直接读本地即可**——内容与历史 `tc_campaign` 一致。

Stuffe 其他仍可访问的相关仓库：

- [`isa_spec`](https://github.com/Stuffe/isa_spec)（Assembly，MIT）—— ISA 规范文档（详见 [`architecture-levels.md`](architecture-levels.md) §Spec.isa 文件结构）
- [`save_monger`](https://github.com/Stuffe/save_monger)（Nim）—— 第三方存档管理工具，与本项目同类

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

98 个关卡子目录，加上几百个 `.png` / `.cvd.png` 视觉资源（关卡插画/位图，可能为全局共享）。

**关卡定义格式**（wiki 已校 2026-08-10；2026-08-11 实测补 hint 系统）—— 每个关卡子目录含**至少 4 个核心文件**：

| 文件 | 必需 | 用途 | 格式 |
|---|---|---|---|
| `circuit.data` | ✅ | 关卡默认布局（含红色不可删除组件、建议组件等） | v13/v14 二进制（详见 [`circuit-data-format.md`](circuit-data-format.md)） |
| `meta.txt` | ✅ | 关卡元数据：标题、教程对话、画布尺寸、默认 ISA、默认程序等 | INI-like key-value（§2.2.1） |
| `ui.txt` | ✅ | 屏幕底部面板的文字/图片元数据 | 方括号条目（§2.2.2） |
| `test.si` | ✅ | 初始化与验证玩家电路的代码 | Simplex DSL（详见 [`compile-signature.md`](compile-signature.md) §test.si API 校对） |
| `hint_0.data` / `hint_0.txt` | ❌ | **第 1 步提示**（电路 + 教学文字） | v13/v14 + i18n 元组 |
| `hint_1.data` / `hint_1.txt` | ❌ | 第 2 步提示…… | 同上 |
| `hint_N.data` / `hint_N.txt` | ❌ | 第 N 步提示（37 关卡有） | 同上 |
| `hint_solution.data` | ❌ | **官方示例解**（66 关卡有） | v13/v14 |
| `hint_solution.txt` | ❌ | 示例解的说明文字 | i18n 元组 |
| `*.png` / `*.cvd.png` | ❌ | 关卡视觉资源 | — |

> ⚠️ **"官方示例解" ≠ "最优解"**。`hint_solution.data` 是 Stuffe 写给玩家卡关时参考的实现，**目的是教学思路**（往往故意写得"标准"而非"极简"，让新手看得懂）。玩家完全可能做出**门数更少 / 延迟更低**的方案。详见 [`hint-system.md`](hint-system.md)。

#### 2.2.1 `meta.txt` 格式

INI-like key-value 格式（来源 wiki `Custom_level_creation/meta.txt`）：

| 字段 | 必需 | 取值 | 说明 |
|---|---|---|---|
| `kind` | 是 | `misc` / `combinational` / `sequential` / `architecture` / `factory` | 关卡类型（**架构关卡对应 `architecture`**） |
| `size` | 是 | U16 | 画布尺寸 |
| `title` | 是 | string | 关卡显示名（可与目录名不同，如 `binary_search/` → "Storage Cracker"） |
| `dialogue` | 是 | multi-line | 教程对话（用 `mentor_centered` / `info` / `overture` 等图片占位符） |
| `tests` | 否 | int | 测试运行次数 |
| `tick_past_fail` | 否 | bool | 失败后是否继续 tick |
| `next_level` | 否 | string | 下一关卡 ID |
| `components_available` | 否 | int / list | 可用组件清单（-1 = 不限） |
| `add_components` / `remove_components` | 否 | list | 修改 build 菜单 |
| `immutable_program` / `immutable_spec` | 否 | bool | 防止玩家编辑默认 `.asm` / `.isa` |

> ⚠️ wiki 标注 "early access 2.0.16 alpha"——稳定版字段可能略有差异。建议与 `Stuffe/tc_campaign` 实测对照。

#### 2.2.2 `ui.txt` 格式

每行一个方括号条目（来源 wiki `Custom_level_creation/ui.txt`）：

```
[text id="text_id" text="Any text you wish to display" font=mono size=24 align=left x=78 y=90 hidden=true]
[image id="image_id" file="filename.png" x=78 y=90 hidden=true]
```

**text** 参数：`id` / `text` / `font`（目前仅 `mono`）/ `size`（24 标准）/ `align`（`left`/`right`，默认 center，相对中心）/ `x`（负值=左）/ `y`（负值=上）/ `hidden`（true 时初始隐藏，test.si 可设为 false）

**image** 参数：`id` / `file` / `x` / `y` / `hidden`

**限制**：
- 图片必须**编译进游戏文件**；未编译图片会让游戏崩溃
- 可临时用 `../<other_level>/<file>.png` 路径占位
- `ui.txt` **不热重载**——改完需重启游戏

#### 2.2.3 Hint 系统（多步提示）

> 详细调研见 [`hint-system.md`](hint-system.md)。本节仅作摘要。

教学关卡自带**多步提示系统**（`.data` + `.txt` 配套），玩家在游戏内点击 Hint 按钮逐级展开：

- `hint_0.data` / `hint_0.txt` —— 第 1 步（最简单的子电路）
- `hint_1.data` / `hint_1.txt` —— 第 2 步
- ... 累加 ...
- `hint_solution.data` / `hint_solution.txt` —— **官方示例解**（完整电路 + 说明文字）

**实测覆盖**（2026-08-11，`E:\SteamLibrary\steamapps\common\Turing Complete\campaign\`）：

| 类别 | 关卡数 | 占比 |
|---|---|---|
| 总关卡 | 98 | 100% |
| 有 `hint_solution.data` | 66 | 67% |
| 有任意 `hint_*.data`（含 `hint_solution`） | 67 | 68% |
| 有多步提示（`hint_0` 到 `hint_N`） | 37 | 38% |
| 无任何 hint | 31 | 32% |

**关键事实**：
- `hint_*.data` 文件格式版本 **= 同关卡 `circuit.data` 版本**（v13 或 v14，**不走 schematics 的 v15**）
- 读取这些文件需要 v13/v14 codec（test/verify-cli 的 `circuit/legacy.rs` 已有 v13/v14 只读）
- `*.txt` 内容形如 `[text text=(31337_68158507707802, \`...English text...\`)]`——i18n key + 英文 fallback
- **"官方示例解" ≠ "最优解"**（详见 [`hint-system.md` §示例解与最优解的区别](hint-system.md)）

**典型有 hint 的关卡**：`always_on` / `and_gate` / `and_gate_3` / `bit_inverter` / `bit_switch` / `counter` / `full_adder` / `the_bus` / `ram_component` 等（皆教学关卡）。

**典型无 hint 的关卡**：`assembly_programming` / `binary_programming` / `binary_search` / `*_racer` 等（架构 / 编程 / 竞赛类关卡，电路不是解题路径）。

#### 2.2.4 把关卡放入地图

（来源 wiki `Custom_level_creation/Adding_your_level_to_the_map`）

关卡做好后，要在游戏主菜单的**世界地图**上看到它，需要额外步骤：

1. 在主菜单**打开游戏控制台**（左上汉堡菜单 `☰` + 按 `q`）
2. 控制台输入 `dev_mode on` —— 启用 level map 上的"Level component"菜单与所有组件
3. 放置 **Level component**：粉红 pin 是输入，红色 pin 是输出；旋转至锁图标方向正确
4. 选中组件，在底部面板的 Level 文本框编辑关卡名（默认显示 `Level ' 'not found`）
5. 用导线连接到上一个关卡（或其他能给出 ON 信号的组件，如 Constant ON）—— 收到 ON 信号后状态变黄（可用但未通关）
6. 控制台输入 `save_level` —— **覆盖 `campaign/main/circuit.data`**
7. 完成更新后控制台输入 `dev_mode off`（dev_mode 开启时无法直接从地图加载关卡，要用底部面板的 Load 按钮）

**重要警告**：

- `circuit.data` 是**二进制文件**，无法自动 merge
- 任意游戏更新 / Steam `Verify Files` 会用默认地图覆盖你的修改
- **务必备份 `campaign/main/`** 目录以便恢复
- 想避免冲突：可以先用 `load <level_id>` 在游戏内加载关卡测试，最后再 `save_level` 写回地图

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