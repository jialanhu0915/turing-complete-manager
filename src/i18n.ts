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

    // 通用
    COMMON_CANCEL: "取消",
    COMMON_CONFIRM: "确定",

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

    // 侧栏
    SIDEBAR_BACKUP: "备份",
    SIDEBAR_CHARACTER: "角色",
    SIDEBAR_LEVELS: "关卡",
    SIDEBAR_CONFIG: "配置",
    SIDEBAR_QQ_LABEL: "QQ 群",

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

    // 角色替换
    CHARACTER_TITLE: "角色替换（mentor 对话人物）",
    CHARACTER_HINT:
      "把游戏的 mentor 对话人物换成你喜欢的图片。游戏本体原图首次使用时自动备份。",
    CHARACTER_DEFAULT_GROUP: "预装角色",
    CHARACTER_CUSTOM_GROUP: "我的角色",
    CHARACTER_DEFAULT_EMPTY: "暂无预装角色。请把角色目录放到 <code>default_characters/</code> 后重新打包。",
    CHARACTER_NEW: "新建我的角色",
    CHARACTER_RESTORE: "恢复游戏原图",
    CHARACTER_NEW_TITLE: "新建角色",
    CHARACTER_NAME_PLACEHOLDER: "角色名称（例：我的猫）",
    CHARACTER_CONFIRM_NEW: "创建",
    CHARACTER_SLOT_NEUTRAL: "中性",
    CHARACTER_SLOT_SMILE: "微笑",
    CHARACTER_SLOT_EMPTY: "（无）",
    CHARACTER_UPLOAD: "上传",
    CHARACTER_APPLY: "应用",
    CHARACTER_DUPLICATE: "复制为我的",
    CHARACTER_DELETE: "删除",
    CHARACTER_CONFIRM_DELETE: "确认删除角色「{name}」？此操作不可撤销。",
    CHARACTER_ACTIVE_BADGE: "当前生效",
    CHARACTER_MATTING_TITLE: "上传图片并抠图",
    CHARACTER_MATTING_HINT:
      "选择背景色 + 调节容差滑块，预览抠图效果。点「保存抠图结果」后保存。",
    CHARACTER_BG_COLOR: "背景色",
    CHARACTER_THRESHOLD: "容差",
    CHARACTER_THRESHOLD_VAL: "{n}",
    CHARACTER_BG_APPLY: "保存抠图结果",
    CHARACTER_UPLOAD_IMAGE: "选择 PNG…",
    CHARACTER_GAME_OK: "游戏目录：{path}",
    CHARACTER_GAME_MISSING: "未检测到游戏（请确认 Steam 已安装并启动过一次）",
    CHARACTER_INSTALL_READONLY:
      "应用安装在只读目录，无法保存数据。请重装到非系统盘（如 D:/Program Files/）。",
    CHARACTER_SNAPSHOT_OK: "原图快照：已保存",
    CHARACTER_SNAPSHOT_PENDING: "原图快照：未（首次应用时自动创建）",
    CHARACTER_EMPTY: "暂无角色。",
    CHARACTER_STATUS_LOADING: "加载中…",

    // Rust 错误
    CHARACTER_NAME_EMPTY: "角色名不能为空",
    CHARACTER_NAME_DUP: "已存在同名角色",
    CHARACTER_NOT_FOUND: "角色不存在",
    CHARACTER_READONLY: "预装角色只读，请用「复制为我的」再编辑",
    CHARACTER_INVALID_SLOT: "无效的 slot（只支持 neutral/smile）",
    CHARACTER_WRITE_FAILED: "写入图片失败：{err}",
    CHARACTER_APPLY_FAILED: "应用角色失败：{err}",
    CHARACTER_DELETE_FAILED: "删除失败：{err}",
    CHARACTER_CREATE_FAILED: "创建失败：{err}",
    CHARACTER_DUPLICATE_FAILED: "复制失败：{err}",
    CHARACTER_UPLOAD_FAILED: "上传失败：{err}",
    CHARACTER_RESTORE_FAILED: "恢复原图失败：{err}",
    INDEX_WRITE_FAILED: "写入索引失败：{err}",
    INSTALL_DIR_NOT_FOUND: "无法定位安装目录",
    GAME_NOT_AVAILABLE: "未检测到游戏（请确认 Steam 已安装并启动过一次）",
    DIALOGUE_DIR_MISSING: "游戏对话目录不存在：{path}",
    SNAPSHOT_FAILED: "快照原图失败：{err}",
    APPLY_WRITE_FAILED: "写入游戏目录失败：{err}",
    NO_SNAPSHOT: "尚未快照原图（首次应用时自动创建）",
    RESTORE_WRITE_FAILED: "恢复原图失败：{err}",
    NOT_CONFIGURED: "未配置。请先完成首次启动向导。",
    GAME_RUNNING_BACKUP: "检测到 Turing Complete 正在运行，请先关闭游戏再备份",
    GAME_RUNNING_RESTORE: "检测到 Turing Complete 正在运行，请先关闭游戏再恢复",
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
  },

  "en-US": {
    APP_TITLE: "Turing Complete Manager",
    INIT_FAILED: "Initialization failed: {err}",
    OP_FAILED: "Operation failed: {err}",

    COMMON_CANCEL: "Cancel",
    COMMON_CONFIRM: "OK",

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

    // Sidebar
    SIDEBAR_BACKUP: "Backups",
    SIDEBAR_CHARACTER: "Characters",
    SIDEBAR_LEVELS: "Levels",
    SIDEBAR_CONFIG: "Settings",
    SIDEBAR_QQ_LABEL: "QQ Group",

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

    // Character replacement
    CHARACTER_TITLE: "Character replacement (mentor dialogue portraits)",
    CHARACTER_HINT:
      "Swap the in-game mentor dialogue portraits for your own images. The game's originals are auto-backed up on first use.",
    CHARACTER_DEFAULT_GROUP: "Bundled characters",
    CHARACTER_CUSTOM_GROUP: "My characters",
    CHARACTER_DEFAULT_EMPTY:
      'No bundled characters. Drop a character folder into <code>default_characters/</code> and rebuild.',
    CHARACTER_NEW: "New character",
    CHARACTER_RESTORE: "Restore game originals",
    CHARACTER_NEW_TITLE: "New character",
    CHARACTER_NAME_PLACEHOLDER: "Character name (e.g. My Cat)",
    CHARACTER_CONFIRM_NEW: "Create",
    CHARACTER_SLOT_NEUTRAL: "Neutral",
    CHARACTER_SLOT_SMILE: "Smile",
    CHARACTER_SLOT_EMPTY: "(empty)",
    CHARACTER_UPLOAD: "Upload",
    CHARACTER_APPLY: "Apply",
    CHARACTER_DUPLICATE: "Duplicate as mine",
    CHARACTER_DELETE: "Delete",
    CHARACTER_CONFIRM_DELETE:
      'Delete character "{name}"? This cannot be undone.',
    CHARACTER_ACTIVE_BADGE: "Active",
    CHARACTER_MATTING_TITLE: "Upload and key out background",
    CHARACTER_MATTING_HINT:
      'Pick the background color and tweak the tolerance slider to preview. Click "Apply" to save.',
    CHARACTER_BG_COLOR: "Background color",
    CHARACTER_THRESHOLD: "Tolerance",
    CHARACTER_THRESHOLD_VAL: "{n}",
    CHARACTER_BG_APPLY: "Apply",
    CHARACTER_UPLOAD_IMAGE: "Choose PNG…",
    CHARACTER_GAME_OK: "Game directory: {path}",
    CHARACTER_GAME_MISSING:
      "Game not detected (make sure Steam is installed and the game has been launched once)",
    CHARACTER_INSTALL_READONLY:
      "The app is installed in a read-only directory; data cannot be saved. Please reinstall to a non-system drive (e.g. D:/Program Files/).",
    CHARACTER_SNAPSHOT_OK: "Originals backed up",
    CHARACTER_SNAPSHOT_PENDING: "Originals: not yet backed up (auto-created on first apply)",
    CHARACTER_EMPTY: "No characters yet.",
    CHARACTER_STATUS_LOADING: "Loading…",

    // Rust errors
    CHARACTER_NAME_EMPTY: "Character name cannot be empty",
    CHARACTER_NAME_DUP: "A character with that name already exists",
    CHARACTER_NOT_FOUND: "Character not found",
    CHARACTER_READONLY:
      'Bundled characters are read-only — use "Duplicate as mine" to edit.',
    CHARACTER_INVALID_SLOT: "Invalid slot (only neutral/smile)",
    CHARACTER_WRITE_FAILED: "Failed to write image: {err}",
    CHARACTER_APPLY_FAILED: "Failed to apply character: {err}",
    CHARACTER_DELETE_FAILED: "Failed to delete: {err}",
    CHARACTER_CREATE_FAILED: "Failed to create: {err}",
    CHARACTER_DUPLICATE_FAILED: "Failed to duplicate: {err}",
    CHARACTER_UPLOAD_FAILED: "Failed to upload: {err}",
    CHARACTER_RESTORE_FAILED: "Failed to restore originals: {err}",
    INDEX_WRITE_FAILED: "Failed to write index: {err}",
    INSTALL_DIR_NOT_FOUND: "Cannot locate install directory",
    GAME_NOT_AVAILABLE: "Game not detected (make sure Steam is installed and the game has been launched once)",
    DIALOGUE_DIR_MISSING: "Game dialogue directory not found: {path}",
    SNAPSHOT_FAILED: "Failed to snapshot originals: {err}",
    APPLY_WRITE_FAILED: "Failed to write to game directory: {err}",
    NO_SNAPSHOT: "No snapshot yet (created automatically on first apply)",
    RESTORE_WRITE_FAILED: "Failed to restore originals: {err}",

    NOT_CONFIGURED: "Not configured. Please complete the first-run wizard.",
    GAME_RUNNING_BACKUP:
      "Turing Complete is running. Close the game before backing up.",
    GAME_RUNNING_RESTORE:
      "Turing Complete is running. Close the game before restoring.",
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
  },
};