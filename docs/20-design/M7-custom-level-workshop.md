---
title: M7 · 自制关卡创意工坊 — 规划
last_updated: 2026-08-15
scope: design
status: M7 设计（2026-08-11 初版；2026-08-15 计划评审修正：补齐 zip 安全 / 持久化 / 导入闭环 / 校验诚实定位 / 强制游戏锁）
---

# M7 · 自制关卡创意工坊 — 规划

> **一句话定位**：给本项目补一个**非官方**的自制关卡工具链，覆盖"打包 → 分享 → 导入 → 验证 → 卸载"全链路，**不替代**未来官方 Workshop。
>
> **范围**：本规划 = 设计。**不**含"如何创作关卡"教程（参考 [官方 Wiki: Custom_level_creation](https://turingcomplete.wiki/wiki/Custom_level_creation/)，CC BY-SA 4.0）。

---

## 一、上一版错误的再校准（2026-08-11）

归档版见 `90-appendix/archived-investigations/custom-level-packaging-2026-08-11-pre-correction.md`。**5 处关键错误**摘要：

| # | 错误 | 正确版本 |
|---|---|---|
| 1 | 架构关卡 zip 必须含 `*.isa` / `*.asm` | ❌ 架构关卡定义与组件关卡**同形**（仅多一个 `default_architecture` 字段指向 Overture/Symphony）；ISA 和 CPU 电路**不在** `campaign/<level>/` 内 |
| 2 | zip 含 `schematics/architecture/<level>/<scheme>/` 层 | ❌ CPU 电路属玩家存档（`%APPDATA%\...schematics\architecture\<arch>\`），**不属关卡定义** |
| 3 | Overture 是玩家搭的 CPU | ❌ Overture / Symphony 是 **Stuffe 预填的玩家存档模板**（首次启动时复制到玩家存档） |
| 4 | "架构关多一层交付物" | ❌ 架构关交付物 = 组件关卡 + `default_architecture` 字段；CPU 仍由玩家用 Overture/Symphony 或自搭 |
| 5 | `dependencies` 字段按"ISA 依赖 CPU"设计 | ❌ 玩家 CPU 不在 zip 里，dependencies 字段本期永远空数组 |

> **核心 insight**：自制关卡包**只装关卡定义**（`campaign/<level>_custom/*`），**不装玩家 CPU 模板**。玩家导入后用 Overture / Symphony / 自搭 CPU 解关卡。

---

## 二、目标与边界

### 2.1 做什么

| 功能 | 描述 |
|---|---|
| **Import** | 玩家导入 `.zip` → zip 安全 + 13 项校验 → 落到游戏目录 → 显示分步加载指引 |
| **Export** | 玩家从游戏目录选关卡 → 打成标准 `.zip` + 自动生成 `manifest.json` |
| **Validate** | 任何"写"操作前自动跑 13 项校验（**文件完整性 / 结构合法性**，不保证可玩） |
| **List** | UI 显示当前已导入的自制关卡列表（与官方 98 关分离；核对目录存在性） |
| **Guide** | 导入后给出可复制的分步加载指引（`dev_mode` / `load <level_id>`，§4.5） |
| **Remove** | 玩家卸载已导入的自制关卡 |
| **Browse** | 工具从 GitHub Release 拉取社区自制关卡清单（仅 M7+1） |

### 2.2 不做什么

- ❌ 中央服务器 / 平台账号体系
- ❌ 评论 / 评分 / 订阅统计（这些不进 manifest.json 也不做平台层）
- ❌ 改游戏 exe / dll / `levels.txt`
- ❌ 跨平台（仅 Windows）
- ❌ 自动"把自制关卡连到主菜单地图" / 驱动游戏控制台（这是游戏 dev_mode 的边界，工具不自动化；工具只**提供分步加载指引**，见 §4.5）
- ❌ 强制删除玩家已导入的关卡（玩家自主控制）
- ❌ 反作弊规避 / 商业用途

### 2.3 M7 已完成的步骤

| 步骤 | Commit |
|---|---|
| 调研：5 处错误纠正 + 归档 | `01461de` |
| 调研：v13/v14 write codec 可行性（~190 行） | `c287259` |
| 调研：meta.txt Wiki 校对（~30 字段） | `c3d39b2` |
| 调研：architecture-levels.md 重写 | `94bd3e4` |
| **设计：本规划** | 🆕 当前 |
| **实现：v13/v14 write codec** | ❌ |
| **实现：13 项校验 pipeline** | ❌ |
| **实现：Tauri UI + import / export / list / remove** | ❌ |
| **实现：GitHub Release browse** | ❌ M7+1 |

---

## 三、关卡包格式（v1.0 spec）

### 3.1 zip 结构

```
my_custom_level.zip
├── manifest.json                          # 必需
├── README.md                              # 可选（玩家说明）
└── campaign/
    └── <level_id>/                        # 必需
        ├── circuit.data                   # v13 / v14（必需）
        ├── meta.txt                       # 必需（必含 4 字段）
        ├── ui.txt                         # 可选（少数关卡无）
        ├── test.si                        # 必需（Simplex DSL）
        ├── label.txt                      # 可选（实测仅 1 关有）
        ├── default.isa                    # 仅 assembly_programming 类
        ├── new_program.asm                # 可选（部分架构关）
        ├── hint_0..N.data / hint_0..N.txt # 可选（教学关）
        ├── hint_solution.data / hint_solution.txt  # 可选
        └── *.png / *.cvd.png              # ui.txt 引用必须存在
```

**硬约束**：
- ❌ **不含** `schematics/architecture/*.isa` / `*.asm` / CPU 电路（玩家存档，不属关卡定义）
- ❌ **不含** `*.pk`（自定义元件分享包，是另一回事）
- ❌ **不允许** `campaign/main/...`（会覆盖主菜单）
- ✅ 所有路径必须 `campaign/<level_id>/...`

### 3.2 manifest.json schema (v1)

```json
{
  "schema_version": 1,
  "level_id": "and_gate_custom",
  "title": "My AND Gate",
  "author": "player123",
  "version": "1.0.0",
  "kind": "combinational",
  "non_official": true,
  "default_architecture": null,
  "dependencies": [],
  "files": {
    "circuit_data_version": "v13",
    "hints": ["hint_0", "hint_solution"],
    "visual_assets": ["preview.png"],
    "additional": ["default.isa", "new_program.asm"]
  },
  "notes": "Optional human-readable description for players.",
  "created_at": "2026-08-11",
  "homepage": "https://github.com/user/my_level/releases/v1.0.0"
}
```

| 字段 | 必需 | 说明 |
|---|---|---|
| `schema_version` | ✅ | 当前固定 `1`，未来字段变更时升级 |
| `level_id` | ✅ | snake_case；**不与官方 98 关重名** |
| `title` | ✅ | UI 显示名 |
| `author` | ✅ | 自由字符串（不验证） |
| `version` | ✅ | semver 推荐；格式不强制 |
| `kind` | ✅ | `misc` / `combinational` / `sequential` / `architecture` / `factory` |
| `non_official` | ✅ | **必须 `true`**（强制显式声明非官方） |
| `default_architecture` | ❌ | `kind=architecture` 时为 `"Overture"` / `"Symphony"` / 自定义 |
| `dependencies` | ❌ | 自定义元件引用（v1.0 留空数组） |
| `files.circuit_data_version` | ✅ | `v13` 或 `v14`（**不放 v15**——v15 是玩家存档） |
| `files.hints` | ❌ | hint 文件名列表（不含后缀） |
| `files.visual_assets` | ❌ | png 文件名列表 |
| `files.additional` | ❌ | 其他可选文件清单 |
| `notes` | ❌ | 玩家描述 |
| `created_at` | ❌ | ISO 8601 |
| `homepage` | ❌ | 分享页面 URL（用于发现） |

> **严格档案级**：manifest.json **不携带** 平台层字段（订阅数 / 评分 / 评论 / 作者 ID / hub_id）。这样与未来官方 Workshop schema **零冲突**。

---

## 四、导入流程与 13 项校验

### 4.1 流程

```
[玩家选 .zip]
   ↓
[解压到临时目录]（先做 zip 安全：路径归一化 + 解压体积上限，见 §4.2 #13）
   ↓
[13 项校验] ─→ 任一失败 → 拒绝 + 具体错误回显
   ↓ (全通过)
[强制检查游戏未运行（tasklist）] ─→ 运行中 → 拒绝 + 提示关闭
   ↓
[写游戏目录]
   ↓
[刷新 UI 关卡列表]
   ↓
[显示分步加载指引（§4.5）]
```

### 4.2 13 项校验清单

| # | 检查 | 失败处理 | 实现位置 |
|---|---|---|---|
| 1 | `manifest.json` 是合法 JSON | 拒绝 | 工具 |
| 2 | `schema_version == 1` | 拒绝（未来版本） | 工具 |
| 3 | `non_official == true` | 拒绝 | 工具 |
| 4 | `level_id` 不与官方 98 关重名 | 拒绝 | 工具（加载官方列表） |
| 5 | `level_id` 不与本地已导入自制关卡重名 | 提示覆盖 | 工具 |
| 6 | `kind` 是合法枚举；`kind=architecture` 时 `manifest.default_architecture` 必须合法（Overture / Symphony / 自定义） | 拒绝 | 工具 |
| 7 | `circuit.data` 用 v13/v14 codec 解码通过 | 拒绝 + 报错 | **v13/v14 codec** |
| 8 | `meta.txt` 必含 `kind` / `size` / `title` / `dialogue` | 拒绝 | 工具 |
| 9 | `ui.txt` 引用的 `*.png` 都在 zip 内 | 拒绝（游戏会崩） | 工具 |
| 10 | `test.si` 用 compile.dll 编译通过 | 拒绝 + 编译错误 | **compile.dll 集成（D-1）** — ⚠️ **MVP 跳过** |
| 11 | `*.isa` / `*.asm` 配对（仅特殊架构关需要） | 拒绝 | 工具 |
| 12 | hint 文件成对（`.data` + `.txt`）+ 版本与同关卡 `circuit.data` 一致 | 拒绝 | 工具 |
| 13 | zip 安全：无内嵌 zip / 可执行 / 脚本；**条目路径归一化后不得逃逸关卡目录**（拒绝 `..` / 绝对路径 / 软链）；**解压总体积上限 100 MB**（对齐 save_monger `MAX_UNCOMPRESSED_SIZE`） | 拒绝（基本安全） | 工具 |

**校验 #10 是核心难点**——依赖 `compile.dll` 集成（D-1）。**MVP 方案（已决策 2026-08-15）**：跳过 #10。

> ⚠️ **诚实定位**：13 项校验 = **文件完整性 / 结构合法性校验**，**不保证关卡可玩**。跳过 #10 后，`test.si` 语法错误、`meta.txt` 字段类型错误、`circuit.data` 解码通过但组件引用非法等，都可能通过全部校验却在游戏内崩溃/软锁。UI 必须在导入结果处明确标注"未深度验证，需进游戏自测"。升级到"语法级编译检查"见 §10.2 待决策。

### 4.3 导入放置路径

| 来源 zip 内 | 解到本地 |
|---|---|
| `campaign/<level_id>/*` | `<game_dir>\campaign\<level_id>\*` |

`<game_dir>` 通过 `detect_game_dir` 自动检测（找不到 → 拒绝）。**绝不**触碰 `campaign/main/`。

### 4.4 卸载流程

（先强制检测游戏未运行）→ 工具删除 `<game_dir>\campaign\<level_id>\` → 工具记录 `uninstalled.json`（**仅本地 config，不上传**）。

### 4.5 分步加载指引（导入闭环）

导入只把文件放进 `campaign/<level_id>/`，**关卡不会自动出现在地图上**（见 level-data.md §2.2.4）。工具在导入完成后生成一份**可复制的分步指引**（不接管游戏）：

1. 打开游戏控制台（主菜单 `☰` + 按 `q`）
2. 输入 `dev_mode on`
3. 输入 `load <level_id>` —— 直接加载关卡测试
4. （可选，需永久上地图）放 Level component → 底部面板填 `<level_id>` → 接线 → `save_level`
5. 输入 `dev_mode off`

> 每次导入/导出前强制提醒：**游戏需关闭 + Steam 云同步关闭**（写 `campaign/` 与云同步冲突，见 §10.3）。

### 4.6 持久化：游戏更新会清空自定义关卡

自定义关卡落在**游戏安装目录** `campaign/<level_id>/`。游戏更新 / Steam `Verify Files` 会覆盖该目录（level-data.md §2.2.4），**玩家导入的关卡会静默消失**，工具的"已安装列表"会与磁盘失同步。

**处理**：
- **List 时核对目录存在性**：缺失则标"已被游戏更新移除"，不再显示为"已安装"
- **一键重导入**：工具记录每个已装关卡对应的源 `.zip` 路径 + `sha256`（本地 config），缺失后可一键恢复
- **覆盖/卸载不误伤进度**：玩家通关存档在 `%APPDATA%\...\schematics\<level>\`（v15），与 `campaign/<level_id>/`（v13/v14）分离，覆盖 campaign/ 不触碰玩家进度

---

## 五、导出流程

### 5.1 流程

```
[选游戏内关卡 + 自填 level_id / author / version / notes]
   ↓
[读 <game_dir>\campaign\<level_id>\*]
   ↓
[v13/v14 codec 重新打包 circuit.data]   ← 关键（不直接复制原文件）
   ↓
[生成 manifest.json]
   ↓
[zip 压缩到 <output_path>]
   ↓
[UI 显示 .zip 路径 + "可上传到 GitHub Releases"]
```

### 5.2 自动 re-pack circuit.data

> **不直接复制原 `circuit.data`**。游戏写的 v13/v14 可能有未对齐的字段（`cost_variant` sentinel、`selected_programs` Map 顺序），直接复制可能导致其他玩家导入失败。工具读 → 解析 → 重新写 → 保证 canonical 编码。

**实现依赖**：`v13/v14 write codec`（~190 行，依据 `c287259` 调研）。

> ⚠️ **已知边界（circuit-data-format.md §v13/v14 风险点 #1）**：原文件 `cost` 若是 `cvk_min_gate` / `cvk_min_delay` sentinel，直接 round-trip 可能产出游戏拒载的文件。**M7 验收必须含**：拿真实 v13 样本「重打包 → 游戏 `load <level_id>` 实测可加载」，不能只靠单元 round-trip（单元测试通过 ≠ 游戏接受）。

### 5.3 玩家上传到 GitHub Releases（推荐）

工具**不集成** GitHub 上传（避免 API key 管理）。导出的 `.zip` 玩家自己：
1. 在自己关卡仓库（GitHub repo）
2. 在 GitHub web 端手动创建 Release → 拖 `.zip`
3. 复制 Release URL 填到 `manifest.homepage`（可选）

---

## 六、Workshop 真正要解决的事：发现 / 分发

工具有 3 个候选分发模式：

### 6.1 模式 A：纯本地（导入 / 导出工具）

- 玩家通过 Discord / 邮箱 / 文件网盘互传 `.zip`
- 工具只做 import / export / list
- **成本**：0 行服务器代码
- **缺点**：无发现机制，类似 "DLL hell"

### 6.2 模式 B：GitHub 索引（推荐，M7+1）

- 公共 GitHub 仓库（如 `turing-complete-custom-levels-index`）维护 `index.json`：
  ```json
  [
    {
      "level_id": "and_gate_custom",
      "title": "My AND Gate",
      "author": "player123",
      "version": "1.0.0",
      "download_url": "https://github.com/.../releases/v1.0.0/level.zip",
      "sha256": "...",
      "manifest": {...}
    }
  ]
  ```
- 工具启动时 fetch `index.json`（GitHub Pages / raw.githubusercontent.com）
- UI 列出所有关卡 → 一键下载 + 校验 + 导入
- **成本**：工具 + 社区仓库（无需服务器）
- **风险**：依赖 GitHub 可用性；**索引治理（谁有权限？index 怎么更新？防 spam）是模式 B 的核心难点，且尚未设计**——这是独立治理问题，M7 不解决、也不给伪方案，留待 M7+1 单独立项

### 6.3 模式 C：自定义服务

- 维护一个简单 web 服务（Flask / Cloudflare Workers）
- 成本高、需要 SEO / DL / 评分 / 评论等
- **不推荐**——与"非官方工具"定位不符

### 6.4 推荐

**MVP（M7）= 模式 A 完整 + 工具内置 "GitHub Browse" 按钮**（模式 B 的轻量版）：

1. 工具支持导入本地 `.zip`（模式 A 完整）
2. 工具内置 "Browse GitHub" 页：调用 GitHub Releases API 列出指定仓库的关卡 → 手动下载至本地 → 走模式 A 导入
3. 完整模式 B（fetch 索引 + 自动发现）作为 M7+1

> 模式 B 完整实现**推迟**到 M7+1：模式 B 涉及"索引仓库治理"（谁有权限？index 怎么更新？）。这是独立的治理问题，不应和工具实现绑在一起。

---

## 七、UI 设计（Tauri 2 + vanilla TS）

### 7.1 页面布局

Tauri 应用增 1 个 tab：

```
[Settings] [Backups] [Custom Levels 🆕] [Help]
```

### 7.2 Custom Levels tab 内容

```
┌─────────────────────────────────────────────────────────┐
│  Custom Levels (3 installed)                            │
├─────────────────────────────────────────────────────────┤
│  [Import from .zip]  [Browse GitHub]  [Export]  [Remove]│
├─────────────────────────────────────────────────────────┤
│  Installed:                                              │
│   ☐ and_gate_custom  v1.0.0  by player123  [uninstall] │
│   ☐ maze_3x3         v0.2.1  by alice      [uninstall] │
│   ☐ full_adder_v2    v1.0.0  by bob        [uninstall] │
├─────────────────────────────────────────────────────────┤
│  ⚠︎  Before import/export (auto-checked):                 │
│     • Game not running (enforced)                        │
│     • Disable Steam Cloud sync                           │
├─────────────────────────────────────────────────────────┤
│  [Browse GitHub]                                         │
│  Browse community-hosted levels from GitHub Releases.   │
│  [Open browser] [Refresh list]                           │
└─────────────────────────────────────────────────────────┘

> **Load guidance（§4.5）**：导入完成后，界面展示可复制三步 `dev_mode on` → `load <level_id>` → `dev_mode off`，附 `[Copy steps]` 按钮。
```

### 7.3 关键 UX 决策

- **每次写操作前强制检测游戏未运行**（tasklist `Turing Complete.exe`，运行中拒绝写）——不是仅提示，是硬性拦截（对齐 tc-save-lab `_assert_game_not_running`）
- **失败错误要可读**：校验失败的 zip 给出具体哪个文件哪条规则失败
- **导入后给出分步加载指引**（§4.5）：一键复制 `dev_mode on` / `load <level_id>` / `dev_mode off`，并诚实说明"永久上地图需手动 dev_mode"（不在工具自动化范围内）
- **导入结果诚实标注**：因校验 #10 跳过，UI 明示"未深度验证，需进游戏自测"
- **不显示自制关卡评分 / 评论**（无平台层）

---

## 八、MVP 范围（M7 v0.2.0）

### 8.1 包含

- 关卡包格式 + manifest.json v1.0 spec
- **Import**：13 项校验（**文件完整性 / 结构合法性；#10 跳过，UI 诚实标注"不保证可玩"**）
- **Import**：zip 安全（路径穿越拒绝 + 解压体积 ≤ 100 MB）
- **Export**：自动 re-pack `circuit.data`（**含真实样本 game-load 实测**，见 §5.2）
- **UI**：Custom Levels tab（import / export / list / remove / **分步加载指引**）
- **持久化**：List 核对目录存在性 + 一键重导入（§4.6）
- **错误回显**：每个校验失败给出具体消息
- **单元测试**：每个校验点独立测试 + codec round-trip

### 8.2 不包含（M7+1+ 再说）

- ❌ GitHub 索引 + 自动发现（模式 B 完整）
- ❌ 评论 / 评分
- ❌ 校验 #10（compile.dll 集成）的完整实现
- ❌ 自定义元件依赖（`dependencies` 字段实际引用）
- ❌ 跨平台
- ❌ 官方 Workshop 格式转换

### 8.3 工期估算

| 工作 | 估算行数 | 工期 |
|---|---|---|
| v13/v14 write codec（port from Stuffe/save_monger Nim） | ~190 | 1-2 天 |
| 13 项校验 pipeline（含 zip 安全：路径归一化 + 体积上限） | ~450 | 1-2 天 |
| import / export 命令实现（含 game-not-running 强制检查） | ~330 | 1 天 |
| 持久化（List 核对 + 一键重导入） | ~120 | 0.5 天 |
| Tauri UI（Custom Levels tab + 分步加载指引） | ~250 TS + ~120 Rust | 1-2 天 |
| 单元测试（含 write codec 真实样本 game-load 实测） | ~350 | 1 天 |
| **合计** | **~1810 行** | **6-8 天** |

### 8.4 commit 切分（按 [CLAUDE.md §5 提交原子化]）

1. `feat(codec): port v13/v14 write codec from Stuffe/save_monger Nim`
2. `feat(packaging): manifest.json v1.0 schema + 13 项校验 pipeline`
3. `feat(packaging): import command (zip -> game dir)`
4. `feat(packaging): export command (game dir -> zip)`
5. `feat(ui): Custom Levels tab (import / export / list / remove)`
6. `test(packaging): 13 项校验 + codec round-trip 单元测试`

---

## 九、后续路线（M7+1+）

| 阶段 | 工作 | 备注 |
|---|---|---|
| **M7+1** | GitHub 索引仓库 + 工具内嵌自动发现 | 模式 B 完整实现 |
| **M7+2** | 校验 #10（compile.dll 集成） | 依赖 D-1 完成 |
| **M7+3** | 评分 / 评论（GitHub Discussions 作为后端） | 延迟到有需求 |
| **M7+4** | 官方 Workshop 格式转换 | 官方出 Workshop 后 |
| **M7+5** | 跨平台（macOS） | 工具本身 Windows-only 仅打包 |

---

## 十、风险与决策点

### 10.1 已决策（基于 2026-08-11 调研）

| 项 | 决策 | 理由 |
|---|---|---|
| Workshop 形态 | 模式 A + 模式 B 轻量版 | 最低成本 + 渐进增强 |
| manifest.json 范围 | 严格档案级（无平台字段） | 与未来官方 Workshop schema 零冲突 |
| 关卡包不含 CPU / ISA | 玩家导入后用 Overture/Symphony 或自搭 | 玩家存档不在关卡定义内 |
| 校验 #10 MVP | 跳过 / 简化提示 | 依赖 compile.dll 集成，先发 MVP |
| v13/v14 codec 实现 | 直接 port from Stuffe/save_monger Nim | Stuffe 是游戏作者，权威 |
| `circuit.data` 重新打包 | 读 → 解析 → 写 | 保证 canonical 编码 |

### 10.2 待决策（需要用户输入）

| 项 | 选项 | 建议 / 状态 |
|---|---|---|
| **官方 Workshop 是否已存在** | ✅ **已核实（2026-08-15）**：官方 Workshop 未上线，开发者仅"计划"中，M7 前提成立 | 保持 manifest 档案级，便于未来对齐 |
| **GitHub 索引仓库命名** | `turing-complete-custom-levels-index` / `tc-custom-levels` / 其他 | 启用前决定（M7+1） |
| **校验 #10 优先级** | ✅ **已决策（2026-08-15）**：MVP 跳过 + UI 诚实标注"不保证可玩" | 升级到"语法级编译检查"留待 M7+2 |
| **跨平台** | 完全不做 / 仅打包 | 建议完全不做 |
| **`tc-save-lab` 复用** | 照搬 v13/v14 codec vs 自写 | 建议自写（参考 Stuffe/save_monger Nim，更权威） |
| **更新升级机制** | v1 仅首次导入；重导入同 `level_id` 提示覆盖（覆盖 campaign/ 不误伤 AppData 通关存档） | v1.1 再设计版本比较 / 增量更新 |

### 10.3 风险

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| 官方 Workshop 在我们做完前发布 | 中 | 高 | 保持 manifest.json 档案级，便于降级 |
| Stuffe 修改 `circuit.data` schema | 低 | 高 | 模式 B 索引能即时发现问题 |
| compile.dll 集成风险（D-1 未决） | 中 | 中 | MVP 跳过 #10 + UI 诚实标注 |
| 玩家误操作覆盖官方关卡 | 低 | 极高 | 校验 #4 + 写前强制游戏关闭检测 |
| **游戏更新 / Verify Files 清空自定义关卡** | 高 | 中 | §4.6：List 核对存在性 + 一键重导入 |
| **zip 路径穿越 / zip bomb** | 中 | 高 | 校验 #13：路径归一化 + 解压体积上限 |
| GitHub 索引仓库被 spam | 中 | 中 | PR 审核 + 工具内黑白名单 |
| v13/v14 write codec 写错导致游戏加载失败 | 中 | 高 | round-trip 单元测试 + **真实样本 game-load 实测（M7 验收内，非 M7 后）** |

---

## 十一、引用

- [官方 Wiki: Custom_level_creation](https://turingcomplete.wiki/wiki/Custom_level_creation/) — CC BY-SA 4.0
- [官方 Wiki: Adding_your_level_to_the_map](https://turingcomplete.wiki/wiki/Custom_level_creation/Adding_your_level_to_the_map) — CC BY-SA 4.0
- `docs/10-investigation/circuit-data-format.md` §v13/v14 写 codec 可行性（`c287259`）
- `docs/10-investigation/architecture-levels.md` §一 / §二（`94bd3e4`）
- `docs/10-investigation/level-data.md` §2.2 / §2.2.1 / §2.2.2（`c3d39b2`）
- `docs/90-appendix/archived-investigations/custom-level-packaging-2026-08-11-pre-correction.md` — 5 处错误警示
- `docs/20-design/index.md` §D-1 / §D-7 — compile.dll 集成前置
- `Stuffe/save_monger` (CC0) — v13/v14 codec 权威参考
