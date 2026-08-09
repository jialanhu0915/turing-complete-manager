export type Locale = "zh-CN" | "en-US";
export type Dict = Record<string, string>;

export const fallbackLocale: Locale = "zh-CN";

let current: Locale = fallbackLocale;

export function setLocale(l: Locale): void {
  current = l;
  document.documentElement.lang = l;
}

export function getLocale(): Locale {
  return current;
}

/** 翻译。优先当前语言，回退到 zh-CN，最后回退到 key 本身。
 *  支持 {name} / {n} / {err} 等占位符。 */
export function t(key: string, params?: Record<string, string | number>): string {
  const dict = DICT[current] ?? DICT[fallbackLocale];
  let s = dict[key] ?? DICT[fallbackLocale][key] ?? key;
  if (params) {
    for (const [k, v] of Object.entries(params)) {
      s = s.replace(new RegExp(`\\{${k}\\}`, "g"), String(v));
    }
  }
  return s;
}

/** 把后端原始错误码（KEY 或带参数的 "KEY|arg1|arg2"）翻译成当前语言。
 *  协议：KEY 后用 | 分隔参数；按出现顺序替换翻译字符串里的 {X} 占位符。 */
export function tErr(raw: string): string {
  const [key, ...args] = raw.split("|");
  const dict = DICT[current] ?? DICT[fallbackLocale];
  let s = dict[key] ?? DICT[fallbackLocale][key] ?? raw;
  let i = 0;
  s = s.replace(/\{[^}]+\}/g, () => args[i++] ?? "{?}");
  return s;
}

/** 扫描整个文档，把 [data-i18n="KEY"] 的 textContent 替换。
 *  [data-i18n-placeholder="KEY"] 替换 input 的 placeholder。 */
export function applyI18n(): void {
  document.querySelectorAll<HTMLElement>("[data-i18n]").forEach((el) => {
    el.textContent = t(el.dataset.i18n!);
  });
  document.querySelectorAll<HTMLInputElement>("[data-i18n-placeholder]").forEach((el) => {
    el.placeholder = t(el.dataset.i18nPlaceholder!);
  });
}

const DICT: Record<Locale, Dict> = {
  "zh-CN": {
    // 通用
    APP_TITLE: "Turing Complete Manager",
    INIT_FAILED: "初始化失败：{err}",
    OP_FAILED: "操作失败：{err}",

    // 向导
    WIZARD_STEP_OF: "第 {current} / 3 步",
    WIZARD_SAVE_TITLE: "选择存档目录",
    WIZARD_SAVE_OK: "✓ 已检测到存档目录",
    WIZARD_SAVE_MISSING:
      "⚠ 未在该路径检测到存档目录，请确认游戏至少启动过一次",
    WIZARD_BACKUP_TITLE: "选择备份目录",
    WIZARD_BACKUP_HINT:
      "默认与软件安装目录相同（其下的 backups 子目录）。",
    WIZARD_LANG_TITLE: "选择界面语言",
    WIZARD_LANG_ZH: "简体中文",
    WIZARD_LANG_EN: "English",
    WIZARD_PREV: "上一步",
    WIZARD_NEXT: "下一步",
    WIZARD_FINISH: "完成",
    WIZARD_EMPTY_PATH: "存档目录和备份目录都不能为空",
    WIZARD_BROWSE: "浏览…",

    // 配置
    CONFIG_TITLE: "配置",
    CONFIG_SAVE_DIR: "存档目录",
    CONFIG_BACKUP_DIR: "备份目录",
    CONFIG_LANGUAGE: "界面语言",
    CONFIG_LANG_ZH: "简体中文",
    CONFIG_LANG_EN: "English",
    CONFIG_RESET: "重置配置",
    CONFIG_RESET_CONFIRM:
      "确认重置配置？存档本身不会被修改，只是回到首次启动向导。",
    CONFIG_RESET_FAILED: "重置失败：{err}",

    // 自动备份
    AUTO_TITLE: "自动备份",
    AUTO_HINT:
      "软件运行期间按间隔自动备份。游戏中也会执行（热备）。超出保留个数时最旧的备份会被自动清理。",
    AUTO_ENABLED: "启用自动备份",
    AUTO_INTERVAL: "间隔（分钟）",
    AUTO_KEEP: "保留个数",
    AUTO_SAVE: "保存自动备份设置",
    AUTO_SAVING: "保存中…",
    AUTO_SAVED: "✓ 已保存",
    AUTO_FAILED_PREFIX: "✗ ",
    AUTO_INTERVAL_ERR: "间隔必须在 1-1440 分钟之间",
    AUTO_KEEP_ERR: "保留个数必须在 1-999 之间",
    AUTO_SAVE_FAILED: "保存失败：{err}",

    // 备份管理
    BACKUP_TITLE: "备份管理",
    GAME_LOADING: "检查中…",
    GAME_RUNNING: "游戏：运行中",
    GAME_NOT_RUNNING: "游戏：未运行",
    GAME_UNKNOWN: "游戏状态未知",
    BACKUP_NOW: "立即备份",
    BACKUP_REFRESH: "刷新列表",
    HEADER_TIME: "备份时间",
    HEADER_SIZE: "大小",
    HEADER_ACTIONS: "操作",
    BACKUP_LOADING: "加载中…",
    BACKUP_EMPTY: "暂无备份。点击「立即备份」创建第一个。",
    BACKUP_RESTORE: "恢复",
    BACKUP_DELETE: "删除",
    BACKUP_LOAD_FAILED: "加载失败：{err}",
    BACKUP_CREATE_FAILED: "备份失败：{err}",
    BACKUP_RESTORE_CONFIRM:
      "确认恢复到备份「{name}」？\n恢复前会自动保存当前状态，可以再回退。",
    BACKUP_RESTORE_DONE: "恢复完成。\n回退用快照：{name}",
    BACKUP_RESTORE_FAILED: "恢复失败：{err}",
    BACKUP_DELETE_CONFIRM: "确认删除备份「{name}」？此操作不可撤销。",
    BACKUP_DELETE_FAILED: "删除失败：{err}",

    // 关卡编辑器
    LEVELS_TITLE: "关卡编辑器（levels.txt）",
    LEVELS_HINT:
      "切换每关的通关标志。仅修改第二列，其他字段原样保留。修改前会自动备份原文件到存档目录下的 levels_backups/。",
    LEVELS_RELOAD: "重新读取",
    LEVELS_SEARCH_PH: "过滤关卡 ID…",
    LEVELS_NO_PENDING: "无修改",
    LEVELS_PENDING: "待保存 {n} 处",
    LEVELS_DISCARD: "放弃修改",
    LEVELS_SAVE: "保存修改",
    LEVELS_HEADER_DONE: "通关",
    LEVELS_HEADER_ID: "关卡 ID",
    LEVELS_HEADER_NAME: "关卡名",
    LEVELS_HEADER_SOL: "方案",
    LEVELS_HEADER_RECORDS: "记录数",
    LEVELS_EMPTY: "未读取。点击「重新读取」加载。",
    LEVELS_LOADING: "加载中…",
    LEVELS_NO_MATCH: "没有匹配的关卡。",
    LEVELS_NO_SOLUTION: "(无)",
    LEVELS_LOAD_FAILED: "加载失败：{err}",
    LEVELS_DISCARD_CONFIRM: "确认放弃 {n} 处未保存修改？",
    LEVELS_SAVE_DONE: "保存完成。\n原文件已备份：{name}",
    LEVELS_SAVE_FAILED: "保存失败：{err}",
    LEVELS_LOAD_FAILED_ALERT: "读取失败：{err}",

    // 电路验证
    VERIFY_TITLE: "电路验证",
    VERIFY_HINT: "用游戏本体验证电路。需检测到 Turing Complete 安装（compile.dll + campaign/）。",
    VERIFY_GAME_AVAILABLE: "✓ 已检测到游戏",
    VERIFY_GAME_MISSING: "未检测到游戏（需 compile.dll + campaign/）",
    VERIFY_LEVEL: "关卡",
    VERIFY_REFRESH: "刷新方案",
    VERIFY_LOADING: "加载中…",
    VERIFY_EMPTY: "选择关卡后查看方案列表。",
    VERIFY_NO_SCHEMES: "该关卡没有可验证的方案。",
    VERIFY_EMPTY_LEVELS: "无可用关卡（关卡名清单为空）。",
    VERIFY_RUN: "验证",
    VERIFY_RUNNING: "验证中…",
    VERIFY_PASS: "通过",
    VERIFY_FAIL: "失败",
    VERIFY_WIN: "通关",
    VERIFY_RESULT_WITH_CYCLES: "{result}（{cycles} cycles）",
    VERIFY_FAILED_ALERT: "验证失败：{err}",
    VERIFY_GAME_NOT_LOADED: "请先确保游戏已安装并启用。",

    // Rust 错误
    NOT_CONFIGURED: "未配置。请先完成首次启动向导。",
    GAME_RUNNING_BACKUP: "检测到 Turing Complete 正在运行，请先关闭游戏再备份",
    GAME_RUNNING_RESTORE: "检测到 Turing Complete 正在运行，请先关闭游戏再恢复",
    GAME_NOT_DETECTED: "未检测到 Turing Complete 安装（compile.dll + campaign/）。",
    APPDATA_NOT_SET: "APPDATA 环境变量未设置",
    CONFIG_DIR_FAILED: "创建配置目录失败：{err}",
    CONFIG_WRITE_FAILED: "写入配置失败：{err}",
    SAVE_DIR_NOT_FOUND: "存档目录不存在：{path}",
    SAVE_DIR_NOT_DIR: "存档路径不是目录：{path}",
    ZIP_CREATE_FAILED: "创建 zip 失败：{err}",
    ZIP_OPEN_FAILED: "打开 zip 失败：{err}",
    ZIP_READ_FAILED: "读取 zip 失败：{err}",
    ZIP_FINISH_FAILED: "完成 zip 失败：{err}",
    ZIP_DIR_FAILED: "写入 zip 目录失败：{err}",
    ZIP_FILE_FAILED: "写入 zip 条目失败：{err}",
    ZIP_ENTRY_FAILED: "解析 zip 条目失败：{err}",
    OPEN_FILE_FAILED: "打开文件失败：{err}",
    COPY_FAILED: "压缩失败：{err}",
    READ_ENTRY_FAILED: "读取 zip 条目失败：{err}",
    MKDIR_FAILED: "创建目录失败：{err}",
    MKDIR_PARENT_FAILED: "创建父目录失败：{err}",
    CREATE_FILE_FAILED: "创建文件失败：{err}",
    BACKUP_DIR_FAILED: "创建备份目录失败：{err}",
    BACKUP_NOT_FOUND: "备份不存在：{name}",
    DELETE_FAILED: "删除失败：{err}",
    DELETE_NOT_ZIP: "只允许删除 .zip 文件",
    WALK_FAILED: "遍历存档目录失败：{err}",
    LEVELS_READ_FAILED: "读取 levels.txt 失败：{err}",
    LEVELS_WRITE_FAILED: "写入 levels.txt 失败：{err}",
    LEVELS_BACKUP_DIR_FAILED: "创建备份目录失败：{err}",
    LEVELS_BACKUP_WRITE_FAILED: "写入备份失败：{err}",
    LEVELS_ROW_FORMAT:
      "levels.txt 第 {n} 行格式错误（少于 3 列）",
    LEVELS_BAD_BOOL: "levels.txt 第 {n} 行第二列不是 true/false",
    NO_LEVEL_CHANGES: "没有要保存的修改",
    CIRCUIT_LIST_FAILED: "枚举方案失败：{err}",
    CIRCUIT_READ_FAILED: "读取电路失败：{err}",
    CIRCUIT_WRITE_FAILED: "写入电路失败：{err}",
    CIRCUIT_ENCODE_FAILED: "编码电路失败：{err}",
    CIRCUIT_DECODE_FAILED: "解码电路失败：{err}",
    CIRCUIT_DIR_FAILED: "创建方案目录失败：{err}",
    CIRCUIT_NUL_BYTE: "电路数据包含 NUL 字节",
    VERIFY_NOT_FOUND: "未找到 verify.exe（{path}）。请先 `cargo build --bin verify`。",
    VERIFY_SPAWN_FAILED: "启动 verify 失败：{err}",
    VERIFY_WAIT_FAILED: "等待 verify 失败：{err}",
    VERIFY_PARSE_FAILED: "verify 输出解析失败：{err}",
    VERIFY_LOCATE: "定位 verify.exe 失败：{err}",
  },

  "en-US": {
    APP_TITLE: "Turing Complete Manager",
    INIT_FAILED: "Initialization failed: {err}",
    OP_FAILED: "Operation failed: {err}",

    WIZARD_STEP_OF: "Step {current} of 3",
    WIZARD_SAVE_TITLE: "Choose save directory",
    WIZARD_SAVE_OK: "✓ Save directory detected",
    WIZARD_SAVE_MISSING:
      "⚠ No save directory at this path. Make sure the game has been launched at least once.",
    WIZARD_BACKUP_TITLE: "Choose backup directory",
    WIZARD_BACKUP_HINT:
      "Defaults to a backups subfolder next to the installed app.",
    WIZARD_LANG_TITLE: "Choose UI language",
    WIZARD_LANG_ZH: "简体中文",
    WIZARD_LANG_EN: "English",
    WIZARD_PREV: "Back",
    WIZARD_NEXT: "Next",
    WIZARD_FINISH: "Finish",
    WIZARD_EMPTY_PATH: "Save directory and backup directory cannot be empty",
    WIZARD_BROWSE: "Browse…",

    CONFIG_TITLE: "Settings",
    CONFIG_SAVE_DIR: "Save directory",
    CONFIG_BACKUP_DIR: "Backup directory",
    CONFIG_LANGUAGE: "UI language",
    CONFIG_LANG_ZH: "简体中文",
    CONFIG_LANG_EN: "English",
    CONFIG_RESET: "Reset settings",
    CONFIG_RESET_CONFIRM:
      "Reset settings? Your saves won't be modified — only the first-run wizard will reopen.",
    CONFIG_RESET_FAILED: "Reset failed: {err}",

    AUTO_TITLE: "Auto-backup",
    AUTO_HINT:
      "Backups run automatically at the configured interval while the app is open. Runs even when the game is running (hot backup). The oldest backups are pruned past the retention count.",
    AUTO_ENABLED: "Enable auto-backup",
    AUTO_INTERVAL: "Interval (minutes)",
    AUTO_KEEP: "Keep",
    AUTO_SAVE: "Save auto-backup settings",
    AUTO_SAVING: "Saving…",
    AUTO_SAVED: "✓ Saved",
    AUTO_FAILED_PREFIX: "✗ ",
    AUTO_INTERVAL_ERR: "Interval must be between 1 and 1440 minutes",
    AUTO_KEEP_ERR: "Keep must be between 1 and 999",
    AUTO_SAVE_FAILED: "Save failed: {err}",

    BACKUP_TITLE: "Backups",
    GAME_LOADING: "Checking…",
    GAME_RUNNING: "Game: running",
    GAME_NOT_RUNNING: "Game: not running",
    GAME_UNKNOWN: "Game status unknown",
    BACKUP_NOW: "Backup now",
    BACKUP_REFRESH: "Refresh list",
    HEADER_TIME: "Backup time",
    HEADER_SIZE: "Size",
    HEADER_ACTIONS: "Actions",
    BACKUP_LOADING: "Loading…",
    BACKUP_EMPTY:
      'No backups yet. Click "Backup now" to create one.',
    BACKUP_RESTORE: "Restore",
    BACKUP_DELETE: "Delete",
    BACKUP_LOAD_FAILED: "Load failed: {err}",
    BACKUP_CREATE_FAILED: "Backup failed: {err}",
    BACKUP_RESTORE_CONFIRM:
      'Restore backup "{name}"?\nThe current state will be auto-saved first, so you can roll back.',
    BACKUP_RESTORE_DONE: "Restore complete.\nRollback snapshot: {name}",
    BACKUP_RESTORE_FAILED: "Restore failed: {err}",
    BACKUP_DELETE_CONFIRM:
      'Delete backup "{name}"? This cannot be undone.',
    BACKUP_DELETE_FAILED: "Delete failed: {err}",

    LEVELS_TITLE: "Level editor (levels.txt)",
    LEVELS_HINT:
      "Toggle completion flag for each level. Only column 2 is modified; everything else is preserved. The original file is backed up to levels_backups/ before saving.",
    LEVELS_RELOAD: "Reload",
    LEVELS_SEARCH_PH: "Filter level ID…",
    LEVELS_NO_PENDING: "No changes",
    LEVELS_PENDING: "{n} pending",
    LEVELS_DISCARD: "Discard",
    LEVELS_SAVE: "Save changes",
    LEVELS_HEADER_DONE: "Done",
    LEVELS_HEADER_ID: "Level ID",
    LEVELS_HEADER_NAME: "Level Name",
    LEVELS_HEADER_SOL: "Solution",
    LEVELS_HEADER_RECORDS: "Records",
    LEVELS_EMPTY: 'Not loaded. Click "Reload" to load.',
    LEVELS_LOADING: "Loading…",
    LEVELS_NO_MATCH: "No matching levels.",
    LEVELS_NO_SOLUTION: "(none)",
    LEVELS_LOAD_FAILED: "Load failed: {err}",
    LEVELS_DISCARD_CONFIRM: "Discard {n} unsaved changes?",
    LEVELS_SAVE_DONE: "Save complete.\nOriginal file backed up: {name}",
    LEVELS_SAVE_FAILED: "Save failed: {err}",
    LEVELS_LOAD_FAILED_ALERT: "Load failed: {err}",

    // Circuit verification
    VERIFY_TITLE: "Circuit Verification",
    VERIFY_HINT: "Validate circuits using the game's own simulator. Requires a Turing Complete install (compile.dll + campaign/).",
    VERIFY_GAME_AVAILABLE: "✓ Game detected",
    VERIFY_GAME_MISSING: "Game not detected (need compile.dll + campaign/)",
    VERIFY_LEVEL: "Level",
    VERIFY_REFRESH: "Refresh schemes",
    VERIFY_LOADING: "Loading…",
    VERIFY_EMPTY: "Pick a level to list its schemes.",
    VERIFY_NO_SCHEMES: "This level has no verifiable schemes.",
    VERIFY_EMPTY_LEVELS: "No levels available (level-name list is empty).",
    VERIFY_RUN: "Verify",
    VERIFY_RUNNING: "Verifying…",
    VERIFY_PASS: "PASS",
    VERIFY_FAIL: "FAIL",
    VERIFY_WIN: "WIN",
    VERIFY_RESULT_WITH_CYCLES: "{result} ({cycles} cycles)",
    VERIFY_FAILED_ALERT: "Verification failed: {err}",
    VERIFY_GAME_NOT_LOADED: "Please ensure the game is installed and enabled first.",

    NOT_CONFIGURED: "Not configured. Please complete the first-run wizard.",
    GAME_RUNNING_BACKUP:
      "Turing Complete is running. Close the game before backing up.",
    GAME_RUNNING_RESTORE:
      "Turing Complete is running. Close the game before restoring.",
    GAME_NOT_DETECTED: "Turing Complete is not installed (compile.dll + campaign/ missing).",
    APPDATA_NOT_SET: "APPDATA environment variable not set",
    CONFIG_DIR_FAILED: "Failed to create settings directory: {err}",
    CONFIG_WRITE_FAILED: "Failed to write settings: {err}",
    SAVE_DIR_NOT_FOUND: "Save directory not found: {path}",
    SAVE_DIR_NOT_DIR: "Save path is not a directory: {path}",
    ZIP_CREATE_FAILED: "Failed to create zip: {err}",
    ZIP_OPEN_FAILED: "Failed to open zip: {err}",
    ZIP_READ_FAILED: "Failed to read zip: {err}",
    ZIP_FINISH_FAILED: "Failed to finalize zip: {err}",
    ZIP_DIR_FAILED: "Failed to write zip directory: {err}",
    ZIP_FILE_FAILED: "Failed to write zip entry: {err}",
    ZIP_ENTRY_FAILED: "Failed to parse zip entry: {err}",
    OPEN_FILE_FAILED: "Failed to open file: {err}",
    COPY_FAILED: "Compression failed: {err}",
    READ_ENTRY_FAILED: "Failed to read zip entry: {err}",
    MKDIR_FAILED: "Failed to create directory: {err}",
    MKDIR_PARENT_FAILED: "Failed to create parent directory: {err}",
    CREATE_FILE_FAILED: "Failed to create file: {err}",
    BACKUP_DIR_FAILED: "Failed to create backup directory: {err}",
    BACKUP_NOT_FOUND: "Backup not found: {name}",
    DELETE_FAILED: "Delete failed: {err}",
    DELETE_NOT_ZIP: "Only .zip files can be deleted",
    WALK_FAILED: "Failed to walk save directory: {err}",
    LEVELS_READ_FAILED: "Failed to read levels.txt: {err}",
    LEVELS_WRITE_FAILED: "Failed to write levels.txt: {err}",
    LEVELS_BACKUP_DIR_FAILED: "Failed to create backup directory: {err}",
    LEVELS_BACKUP_WRITE_FAILED: "Failed to write backup: {err}",
    LEVELS_ROW_FORMAT: "levels.txt line {n}: format error (less than 3 columns)",
    LEVELS_BAD_BOOL: "levels.txt line {n}: column 2 is not true/false",
    NO_LEVEL_CHANGES: "No changes to save",
    CIRCUIT_LIST_FAILED: "Failed to list schemes: {err}",
    CIRCUIT_READ_FAILED: "Failed to read circuit: {err}",
    CIRCUIT_WRITE_FAILED: "Failed to write circuit: {err}",
    CIRCUIT_ENCODE_FAILED: "Failed to encode circuit: {err}",
    CIRCUIT_DECODE_FAILED: "Failed to decode circuit: {err}",
    CIRCUIT_DIR_FAILED: "Failed to create scheme directory: {err}",
    CIRCUIT_NUL_BYTE: "Circuit data contains NUL byte",
    VERIFY_NOT_FOUND: "verify.exe not found ({path}). Run `cargo build --bin verify` first.",
    VERIFY_SPAWN_FAILED: "Failed to spawn verify: {err}",
    VERIFY_WAIT_FAILED: "Failed to wait for verify: {err}",
    VERIFY_PARSE_FAILED: "Failed to parse verify output: {err}",
    VERIFY_LOCATE: "Failed to locate verify.exe: {err}",
  },
};