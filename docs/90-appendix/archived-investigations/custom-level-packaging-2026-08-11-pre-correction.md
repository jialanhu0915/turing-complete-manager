---
title: 自制关卡打包/分享（非官方工具）[SUPERSEDED 2026-08-11]
last_updated: 2026-08-11
scope: investigation-archived
status: SUPERSEDED 2026-08-11 — 初稿含未实测的推测，被替代版覆盖
---

# [SUPERSEDED 2026-08-11] 自制关卡打包/分享

> ⚠️ **本版本已被替代**。2026-08-11 实地核对（`E:\SteamLibrary\...campaign\*` 与 `%APPDATA%\Turing Complete\schematics\architecture\*`）后纠正。
>
> 替代版位置：`docs/10-investigation/custom-level-packaging.md`（尚未写，下一步）。
>
> **本版本的关键错误**：
>
> 1. **§二.2 错**——声称架构关 zip 必须含 `*.isa` / `*.asm`。实测 11 个架构关里只有 `assembly_programming`（`kind=sequential`，不是 architecture）一个含 `default.isa` + `new_program.asm`，其余 10 个均无
> 2. **§三 zip 结构错**——假设了 `schematics/architecture/<level>/<scheme>/{*.isa,*.asm,circuit.data}` 这层。**CPU 电路 + ISA 不属于关卡定义**，它们是玩家存档（`%APPDATA%\Turing Complete\schematics\architecture\<arch>\`）
> 3. **§二.2 把 `Overture` 推断错**——写"玩家搭的 CPU（个人工具）"。实测 `Overture` 和 `Symphony` 是 **Stuffe 预填的玩家存档模板**（首次启动时复制到玩家存档），不是玩家搭建的
> 4. **§二.2 把架构关卡描述错**——"玩家交付物多一层（ISA + CPU + 程序）"。**关卡定义不包含 ISA / CPU**——关卡定义只有 `circuit.data`（关卡默认输入输出布局）+ `meta.txt` + `test.si`（外加可选的 ui.txt / label.txt / hint_*.data / *.png）
> 5. **§三依赖字段 `dependencies` 错**——按"ISA 依赖 CPU"的假设设计了 dependencies 字段，**实际 zip 里不含 CPU 文件，这个字段永远为空**
>
> **保留原因**：作为"基于前几轮对话推断、未实测"的反面教材，避免后续重蹈覆辙。下次做类似调研前必须先实测文件系统，不要从二手描述推断。

---

> **核心定位**（原内容）：本项目提供一个**非官方**的玩家自制关卡打包/导入工具，让玩家之间互传自定义关卡定义。**不替代**任何官方未来可能推出的 Workshop / 自制关卡平台；**官方推出后工具降级**（详见 [§降级路径](#降级路径)）。

> ⚠️ **本调研 ≠ 自制关卡教程**。**如何创作**一个关卡请参考官方/社区 Wiki [`turingcomplete.wiki/wiki/Custom_level_creation/`](https://turingcomplete.wiki/wiki/Custom_level_creation/)（CC BY-SA 4.0）。本调研只覆盖"打包/分享/导入"这一段——官方 Wiki 暂未明确文档化。

---

## 一、工具边界

| 做 | 不做 |
|---|---|
| 玩家把自制关卡打成 zip 给别人 | 维护任何中央服务器 |
| 验证 zip 内文件合法性 | 爬官方 schematic hub 数据 |
| 解 zip 到游戏本地目录 | 接 Steam Workshop API |
| 强制标注"非官方" | 触 `campaign/main/circuit.data` |
| 官方出 Workshop 后降级停摆 | 强行删除玩家已导入的关卡 |
| 仅 Windows 平台（与项目范围一致） | 跨平台支持 |

---

## 二、自制关卡文件清单

每个自制关卡由**一到两组目录树**组成（取决于关卡类型）。

### 2.1 通用必需文件（所有关卡）

| 文件 | 必需 | 格式 | 校验 |
|---|---|---|---|
| `circuit.data` | ✅ | v13 / v14 二进制（详见 [`circuit-data-format.md`](circuit-data-format.md)） | 用 `circuit/legacy.rs` v13/v14 codec 解码通过 |
| `meta.txt` | ✅ | INI-like key-value | 必含 `kind` / `size` / `title` / `dialogue`（[`level-data.md` §2.2.1](level-data.md)） |
| `ui.txt` | ✅ | 方括号条目 | 引用的 `*.png` 必须在 zip 内（否则游戏崩溃） |
| `test.si` | ✅ | Simplex DSL | 用 compile.dll 编译通过（[`compile-signature.md`](compile-signature.md) §test.si） |

### 2.2 架构关卡额外必需文件

> **本质（用户校对 2026-08-11）**：架构关卡里**一切都是玩家从零定义的**——ISA（汇编到机器码的对应表）、CPU 电路（用基础组件搭出的门级实现）、汇编程序都是玩家产物。**唯一由游戏规定的只有输入/输出接口契约**（`test.si` 的 `arch_check_output` / `arch_get_input`）。
>
> 这与组件关卡的边界**完全一致**——组件关是"游戏规定 I/O，玩家实现电路"；架构关是"游戏规定 I/O，玩家定义 ISA + 实现 CPU + 写汇编程序"。**架构关多出的不是'游戏规定更多'，而是'玩家交付物多一层'**。

如果 `meta.txt::kind == architecture`，**必须额外**有：

| 文件 | 必需 | 格式 | 谁定义 |
|---|---|---|---|
| `*.isa` | ✅ | ISA 声明：架构名、变体、字节序、寄存器、指令编码（详见 [`architecture-levels.md` §四](architecture-levels.md)） | **玩家**（自定义 ISA） |
| `*.asm` | ✅ | 用上面 `.isa` 写的汇编程序 | **玩家** |
| `circuit.data` | ✅ | CPU 电路：玩家用基础组件（AND/OR/Register 等）搭出的门级实现（v13/v14 二进制） | **玩家** |

> ⚠️ **架构关卡的字节级协议盲区**：`arch_check_output(test, input, output)` / `arch_get_input(test)` 用 `Int` 而非结构体，**标量到位的拆分方式未实测**（[`architecture-levels.md` §三 盲区](architecture-levels.md)）。自制架构关的合法性验证范围目前**仅限于**文件齐全 + `.isa` 可解析，**不覆盖运行时验证**——玩家必须自己保证 ISA + CPU + 汇编程序三者自洽。

### 2.3 可选文件（教学关推荐）

| 文件 | 用途 | 详见 |
|---|---|---|
| `hint_0.data` / `hint_0.txt` | 第 1 步提示 | [`hint-system.md`](hint-system.md) |
| `hint_1..N.data` / `hint_1..N.txt` | 第 N 步提示 | 同上 |
| `hint_solution.data` / `hint_solution.txt` | 官方示例解（**非最优解**） | 同上 §示例解与最优解的区别 |
| `*.png` / `*.cvd.png` | 关卡视觉资源（ui.txt 引用必须有文件） | [`level-data.md` §2.2.2](level-data.md) |

> ⚠️ **`.data` 和 `.txt` 一一对应**——要么都放、要么都不放；放单边视为非法包。
>
> ⚠️ **`.png` 文件必须存在**——`ui.txt` 引用的 `*.png` 如不在包内，**游戏启动时会崩溃**（[`level-data.md` §2.2.2 限制](level-data.md)）。

---

## 三、打包格式约定（zip 结构）

### 3.1 完整 zip 结构

```
my_custom_level.zip
├── manifest.json                       # 元数据（必需）
├── README.md                           # 玩家说明（可选）
├── campaign/
│   └── <level_id>/                     # 关卡定义（必需）
│       ├── circuit.data
│       ├── meta.txt
│       ├── ui.txt
│       ├── test.si
│       ├── hint_0.data                 # 可选
│       ├── hint_0.txt                  # 可选（与 hint_0.data 一一对应）
│       ├── hint_solution.data          # 可选
│       ├── hint_solution.txt           # 可选
│       └── preview.png                 # 可选（关卡视觉资源）
└── schematics/
    └── architecture/
        └── <level_id>/                 # 架构关专用（仅 kind=architecture 时需要）
            └── <scheme>/
                ├── *.isa
                ├── *.asm
                └── circuit.data
```

### 3.2 `manifest.json` 字段

```json
{
  "schema_version": 1,
  "level_id": "and_gate_custom",
  "title": "My AND Gate",
  "author": "player123",
  "version": "1.0.0",
  "kind": "combinational",
  "non_official": true,
  "dependencies": [],
  "files": {
    "circuit_data_version": "v13",
    "hints": ["hint_0", "hint_solution"],
    "visual_assets": ["preview.png"]
  },
  "notes": "Optional human-readable description for players"
}
```

字段说明：

| 字段 | 必需 | 说明 |
|---|---|---|
| `schema_version` | ✅ | 当前固定 `1`；未来字段变更时升级 |
| `level_id` | ✅ | 与目录名一致；合法 snake_case；**不与官方 98 关重名** |
| `title` | ✅ | 关卡显示名（可与 `level_id` 不同） |
| `author` | ✅ | 玩家标识（任意字符串，不验证） |
| `version` | ✅ | 玩家自定义版本号（任意字符串） |
| `kind` | ✅ | `misc` / `combinational` / `sequential` / `architecture` / `factory`（与 `meta.txt::kind` 一致） |
| `non_official` | ✅ | **必须 `true`**——强制显式声明非官方来源 |
| `dependencies` | ❌ | 自定义元件引用列表（本期工具可放空数组） |
| `files.circuit_data_version` | ✅ | 必须是 `v13` 或 `v14`（**不放 v15**——v15 是玩家存档格式，不是关卡定义格式） |
| `files.hints` | ❌ | hint 文件名列表（不含后缀；只列实际存在的） |
| `files.visual_assets` | ❌ | png 文件名列表 |
| `notes` | ❌ | 人类可读说明（玩家互相交流用） |

---

## 四、导入合法性检查清单

工具导入 zip 时**必须**通过的所有检查（任一失败 → 拒绝导入）：

| # | 检查 | 失败处理 |
|---|---|---|
| 1 | `manifest.json` 存在且是合法 JSON | 拒绝，UI 报错 |
| 2 | `manifest.schema_version == 1` | 拒绝（不识别未来版本） |
| 3 | `manifest.non_official == true` | 拒绝（强制声明） |
| 4 | `manifest.level_id` 不与官方 98 关重名 | 拒绝（防覆盖官方关卡） |
| 5 | `manifest.level_id` 不与本地已导入的自制关卡重名 | UI 提示玩家改名或覆盖 |
| 6 | `manifest.kind` 是合法枚举值 | 拒绝 |
| 7 | `circuit.data` 能用 v13/v14 codec 解码 | 拒绝，UI 显示具体错误 |
| 8 | `meta.txt` 必含 `kind` / `size` / `title` / `dialogue` | 拒绝 |
| 9 | `ui.txt` 引用的 `*.png` 都在 zip 内 | 拒绝（否则游戏崩溃） |
| 10 | `test.si` 能用 compile.dll 编译通过 | 拒绝，UI 显示编译错误 |
| 11 | 架构关：`*.isa` / `*.asm` 都存在 | 拒绝 |
| 12 | hint 文件成对（`.data` + `.txt`） | 拒绝 |
| 13 | zip 内无 zip / 无可执行文件 / 无脚本 | 拒绝（基本安全检查） |

> 校验 #10 用 `compile.dll` 是核心难点——需要 test/verify-cli 的 verify 子命令（已实现，参考 [`compile-signature.md` §执行验证](compile-signature.md)）。

---

## 五、导入放置路径

导入时，工具按以下路径解压（**绝不触碰 `campaign/main/`**）：

| 来源 zip 内 | 解到本地 |
|---|---|
| `campaign/<level_id>/*` | `<game_dir>\campaign\<level_id>\*` |
| `schematics/architecture/<level_id>/<scheme>/*` | `%APPDATA%\Turing Complete\schematics\architecture\<level_id>\<scheme>\*` |

其中 `<game_dir>` = `E:\SteamLibrary\steamapps\common\Turing Complete\`（自动检测，详见 `src-tauri/src/translations.rs::detect_game_dir`）。

> ⚠️ **不自动放入主菜单地图**——`campaign/main/circuit.data`（主菜单地图）**任何游戏更新都会被覆盖**（[`level-data.md` §2.2.4](level-data.md)）。玩家需自行用游戏 dev_mode 流程把新关卡连到地图（详见 [§玩家后续操作](#七玩家后续操作)）。

---

## 六、导出侧（玩家打包自己的关卡）

工具导出时，按以下结构生成 zip：

1. 玩家在 Tauri UI 选关卡类型（组件关 / 架构关）+ level_id
2. 工具读 `<game_dir>\campaign\<level_id>\*`（组件关）或 `%APPDATA%\...schematics\architecture\<level_id>\<scheme>\*`（架构关）
3. 按 [§三 zip 结构](#三打包格式约定zip-结构) 生成 zip
4. 工具自动生成 `manifest.json`（玩家填 author / version / notes）
5. 输出 `<output_path>.zip` 给玩家

> 工具**不修改**源关卡目录——只读。

---

## 七、玩家后续操作

工具只完成"文件落地"。**玩家需要自己用游戏机制完成"放入地图"**：

1. **关闭游戏**（避免文件锁冲突）
2. **关闭 Steam 云同步**（避免恢复冲突，详见 [`level-data.md` §1.5](level-data.md)）
3. **启动游戏** → 主菜单汉堡菜单 `☰` + 按 `q` → 打开控制台
4. 控制台输入 `dev_mode on`（启用 Level component 菜单）
5. **放置 Level component** → 旋转对齐 → 编辑底部 Level 文本框为 `<level_id>`
6. **用 Constant ON 等组件接入信号** → 收到 ON 后状态变黄
7. 控制台输入 `save_level`（写入 `campaign/main/circuit.data`）
8. 控制台输入 `dev_mode off` → 用 Load 按钮加载关卡测试
9. 确认无误后再次 `save_level` 写回正式地图

> 这套流程参考 [`level-data.md` §2.2.4](level-data.md)。**工具不自动化"放入地图"这一步**——这是官方机制本身的边界（地图会被游戏更新覆盖）。

---

## 八、降级路径

**官方推出 Steam Workshop / 官方自制关卡平台后**，本工具按以下顺序降级：

| 阶段 | 行动 |
|---|---|
| 公告期 | README 加 "⚠️ 官方 Workshop 已上线，建议使用" 提示 |
| 迁移期（6 个月） | 保留所有功能 + 提供"导出为官方 Workshop 格式"转换工具（若官方格式公开） |
| 归档期 | GitHub release 打 `archived-YYYY-MM` tag；release notes 说明迁移路径 |
| 工具停摆 | **不强制删除玩家已导入的关卡**——玩家自己决定是否删除 |

**不与官方 Workshop 抢食**：

- ❌ 不维护中央服务器
- ❌ 不爬官方 hub 数据
- ❌ 不接 Steam Workshop API
- ❌ 不引入 hub_id / 评分 / 订阅 / 评论等平台字段到 manifest.json

**manifest.json 严格保持"档案级"**——只描述关卡本身，不携带任何平台层元数据。这样：
- 与官方平台 schema 零冲突
- 玩家随时可迁移到官方平台
- 我们工具停了，玩家数据依然有效（标准 zip + 人类可读的 json）

---

## 九、待续 / 盲区

- ❌ **架构关卡的运行时验证**——`arch_check_output` / `arch_get_input` 用 `Int` 而非结构体，**未实测**（[`architecture-levels.md` §三](architecture-levels.md)）。工具对架构关的合法性验证**仅限文件齐全**，不覆盖运行时。
- ❌ **官方 schema 升级**——`circuit.data` 未来如升级到 v16+，v13/v14 codec 都要更新
- ❌ **自定义元件依赖（`dependencies`）**——本期留空；后续如果支持，要在 manifest 引入 `references` 字段或单独的 `components/` 目录
- ❌ **跨语言关卡**——目前仅中文 / 英文；i18n 字段未设计
- ❌ **官方 Custom_level_creation Wiki 与我们的关系**——我们是工具侧，Wiki 是教程侧；是否需要在 Wiki 引用我们 / 我们引用 Wiki 待决定（CC BY-SA 4.0 兼容）

---

## 十、相关文档

- [`level-data.md`](level-data.md) §2.2 — 关卡定义文件清单（meta.txt / ui.txt / circuit.data / test.si）
- [`hint-system.md`](hint-system.md) — hint 文件体系（教学关可选）
- [`architecture-levels.md`](architecture-levels.md) — 架构关卡额外文件（.isa / .asm）+ API 盲区
- [`circuit-data-format.md`](circuit-data-format.md) — circuit.data v13/v14 二进制格式
- [`compile-signature.md`](compile-signature.md) §test.si — test.si 合法性验证入口
- [`docs/20-design/index.md`](../20-design/index.md) — M7 设计起点

## 引用源

- 官方/社区 Wiki：[`turingcomplete.wiki/wiki/Custom_level_creation`](https://turingcomplete.wiki/wiki/Custom_level_creation/)（CC BY-SA 4.0）
- 官方/社区 Wiki：[`turingcomplete.wiki/wiki/Custom_level_creation/Adding_your_level_to_the_map`](https://turingcomplete.wiki/wiki/Custom_level_creation/Adding_your_level_to_the_map)（CC BY-SA 4.0）