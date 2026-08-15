import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import {
  applyI18n,
  getLocale,
  setLocale,
  t,
  tErr,
  type Locale,
} from "./i18n";
import { bindCharacterEvents, refreshCharacters } from "./character";

interface AppConfig {
  save_dir: string;
  backup_dir: string;
  language: string;
  auto_backup_enabled: boolean;
  auto_backup_interval_min: number;
  auto_backup_keep: number;
  game_dir: string | null;
  game_dir_source: "auto" | "manual";
}

interface DetectSaveDir {
  default_path: string;
  exists: boolean;
}

interface BackupInfo {
  name: string;
  created_at: string;
  size_bytes: number;
}

interface LevelRow {
  id: string;
  solution: string;
  completed: boolean;
  records: number;
  line_index: number;
}

interface LevelName {
  en: string;
  "zh-CN": string;
}

interface GameDirStatus {
  path: string | null;
  source: string;
  exists: boolean;
}

interface CircuitTestResult {
  ok: boolean;
  test_result: number;
  cycles_run: number;
  error: string | null;
}

let cfg: AppConfig | null = null;
let currentStep = 1;

type ViewName = "backup" | "character" | "levels" | "config" | "test";

/** 切换主页面内的 view（侧栏点击触发）。隐藏非激活 view + 更新侧栏高亮。 */
function switchView(name: ViewName): void {
  document.querySelectorAll<HTMLElement>("#main .view").forEach((el) => {
    el.hidden = el.dataset.view !== name;
  });
  document.querySelectorAll<HTMLButtonElement>("#main .nav-item").forEach((btn) => {
    const isActive = btn.dataset.view === name;
    btn.classList.toggle("active", isActive);
    if (isActive) btn.setAttribute("aria-current", "page");
    else btn.removeAttribute("aria-current");
  });
  if (name === "test") {
    refreshTestView().catch((e) => console.error("refreshTestView failed:", e));
  }
}

function $<T extends HTMLElement>(sel: string): T {
  const el = document.querySelector<T>(sel);
  if (!el) throw new Error(`element not found: ${sel}`);
  return el;
}

function fmtBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(2)} MB`;
}

function fmtDate(iso: string): string {
  return iso.replace("T", " ");
}

// 游戏状态短期缓存：避免每次刷新都启动 tasklist 子进程（~200ms 开销）
const GAME_STATUS_TTL_MS = 5000;
let gameStatusCache: { value: boolean; ts: number } | null = null;

/** 重新渲染所有依赖语言的 UI 部分。静态部分由 applyI18n() 一次扫描完成。 */
function rerenderDynamic(): void {
  $("#wizard-step-indicator").textContent = t("WIZARD_STEP_OF", { current: currentStep });
  fillAutoBackupForm();
  if (cfg) $("#current-language").textContent = cfg.language;
  $("#game-status").textContent = t("GAME_LOADING");
  $("#game-status").classList.remove("warn");
  refreshGameStatus(true);
  refreshBackupList();
  renderLevelRows();
  updateLevelPendingUI();
  refreshCharacters().catch((e) => console.error("refreshCharacters failed:", e));
}

async function init(): Promise<void> {
  const [configResult, version] = await Promise.all([
    invoke<AppConfig | null>("get_config"),
    invoke<string>("app_version").catch(() => "?"),
  ]);
  cfg = configResult;
  setLocale((cfg?.language as Locale) ?? "zh-CN");
  $("#app-version").textContent = version;
  applyI18n();
  if (cfg) {
    await showMain();
  } else {
    await showWizard();
  }
}

async function showWizard(): Promise<void> {
  $("#wizard").hidden = false;
  $("#main").hidden = true;
  await loadWizardDefaults();
  showStep(1);
}

async function showMain(): Promise<void> {
  $("#wizard").hidden = true;
  $("#main").hidden = false;
  $("#current-save-dir").textContent = cfg!.save_dir;
  $("#current-backup-dir").textContent = cfg!.backup_dir;
  $("#current-language").textContent = cfg!.language;
  $<HTMLSelectElement>("#lang-select").value = cfg!.language;
  fillAutoBackupForm();
  refreshGameDir();
  switchView("backup");
  await Promise.all([
    refreshGameStatus(),
    refreshBackupList(),
    refreshCharacters().catch((e) => console.error("refreshCharacters failed:", e)),
  ]);
}

function fillAutoBackupForm(): void {
  if (!cfg) return;
  $<HTMLInputElement>("#auto-enabled").checked = cfg.auto_backup_enabled;
  $<HTMLInputElement>("#auto-interval").value = String(cfg.auto_backup_interval_min);
  $<HTMLInputElement>("#auto-keep").value = String(cfg.auto_backup_keep);
}

function readAutoBackupForm(): {
  enabled: boolean;
  interval: number;
  keep: number;
} | null {
  const enabled = $<HTMLInputElement>("#auto-enabled").checked;
  const interval = Number($<HTMLInputElement>("#auto-interval").value);
  const keep = Number($<HTMLInputElement>("#auto-keep").value);
  if (!Number.isFinite(interval) || interval < 1 || interval > 1440) {
    alert(t("AUTO_INTERVAL_ERR"));
    return null;
  }
  if (!Number.isFinite(keep) || keep < 1 || keep > 999) {
    alert(t("AUTO_KEEP_ERR"));
    return null;
  }
  return { enabled, interval, keep };
}

async function saveAutoBackup(): Promise<void> {
  if (!cfg) return;
  const form = readAutoBackupForm();
  if (!form) return;
  const next: AppConfig = {
    ...cfg,
    auto_backup_enabled: form.enabled,
    auto_backup_interval_min: form.interval,
    auto_backup_keep: form.keep,
  };
  const btn = $<HTMLButtonElement>("#auto-save");
  const status = $("#auto-status");
  btn.disabled = true;
  status.textContent = t("AUTO_SAVING");
  try {
    await invoke("set_config", { cfg: next });
    cfg = next;
    status.textContent = t("AUTO_SAVED");
    status.classList.remove("warn");
  } catch (e) {
    status.textContent = `${t("AUTO_FAILED_PREFIX")}${tErr(String(e))}`;
    status.classList.add("warn");
  } finally {
    btn.disabled = false;
  }
}

async function loadWizardDefaults(): Promise<void> {
  const [detect, backupDefault] = await Promise.all([
    invoke<DetectSaveDir>("detect_save_dir"),
    invoke<string>("detect_backup_dir"),
  ]);
  const saveInput = $<HTMLInputElement>("#save-dir-input");
  saveInput.value = detect.default_path;

  const hint = $("#save-dir-hint");
  if (detect.exists) {
    hint.textContent = t("WIZARD_SAVE_OK");
    hint.classList.remove("warn");
  } else {
    hint.textContent = t("WIZARD_SAVE_MISSING");
    hint.classList.add("warn");
  }

  $<HTMLInputElement>("#backup-dir-input").value = backupDefault;
}

function showStep(n: number): void {
  currentStep = n;
  for (let i = 1; i <= 3; i++) {
    $(`#step-${i}`).hidden = i !== n;
  }
  $("#wizard-step-indicator").textContent = t("WIZARD_STEP_OF", { current: n });
  $("#wizard-prev").hidden = n === 1;
  $("#wizard-next").hidden = n === 3;
  $("#wizard-finish").hidden = n !== 3;
}

async function pickFolder(target: HTMLInputElement): Promise<void> {
  const selected = await open({
    directory: true,
    multiple: false,
    defaultPath: target.value || undefined,
  });
  if (typeof selected === "string") {
    target.value = selected;
  }
}

function readSelectedLang(): string {
  const checked = document.querySelector<HTMLInputElement>(
    'input[name="lang"]:checked'
  );
  return checked?.value ?? "zh-CN";
}

async function finishWizard(): Promise<void> {
  const saveDir = $<HTMLInputElement>("#save-dir-input").value.trim();
  const backupDir = $<HTMLInputElement>("#backup-dir-input").value.trim();
  if (!saveDir || !backupDir) {
    alert(t("WIZARD_EMPTY_PATH"));
    return;
  }
  cfg = {
    save_dir: saveDir,
    backup_dir: backupDir,
    language: readSelectedLang(),
    auto_backup_enabled: false,
    auto_backup_interval_min: 30,
    auto_backup_keep: 20,
    game_dir: null,
    game_dir_source: "auto",
  };
  await invoke("set_config", { cfg });
  await showMain();
}

async function resetConfig(): Promise<void> {
  if (!confirm(t("CONFIG_RESET_CONFIRM"))) return;
  cfg = null;
  await showWizard();
}

async function refreshGameDir(): Promise<void> {
  const pathEl = $("#current-game-dir");
  const badge = $<HTMLElement>("#current-game-dir-source");
  const warn = $("#game-dir-warn");
  if (!cfg) return;
  if (!cfg.game_dir) {
    pathEl.textContent = "—";
    badge.textContent = "";
    warn.hidden = false;
    warn.textContent = t("CONFIG_GAME_DIR_NOT_FOUND");
    return;
  }
  pathEl.textContent = cfg.game_dir;
  badge.textContent =
    cfg.game_dir_source === "manual"
      ? t("CONFIG_GAME_DIR_SOURCE_MANUAL")
      : t("CONFIG_GAME_DIR_SOURCE_AUTO");
  // 异步校验：缓存路径可能已被用户移动 / 卸载
  try {
    const s = await invoke<GameDirStatus>("get_game_dir_status");
    if (s.exists) {
      warn.hidden = true;
      warn.textContent = "";
    } else {
      warn.hidden = false;
      warn.textContent = t("CONFIG_GAME_DIR_INVALID");
    }
  } catch {
    // 校验失败不致命 —— 保留 path 显示
  }
}

async function redetectGameDir(): Promise<void> {
  const btn = $<HTMLButtonElement>("#redetect-game-dir");
  btn.disabled = true;
  try {
    const path = await invoke<string | null>("detect_game_dir");
    if (cfg) {
      cfg.game_dir = path;
      cfg.game_dir_source = "auto";
    }
    await refreshGameDir();
    await refreshCharacters().catch((e) =>
      console.error("refreshCharacters failed:", e),
    );
  } catch (e) {
    alert(t("CONFIG_GAME_DIR_REDETECT_FAILED", { err: tErr(String(e)) }));
  } finally {
    btn.disabled = false;
  }
}

async function pickGameDir(): Promise<void> {
  const selected = await open({
    directory: true,
    multiple: false,
    defaultPath: cfg?.game_dir ?? undefined,
  });
  if (typeof selected !== "string") return;
  try {
    await invoke("set_game_dir", { path: selected });
    if (cfg) {
      cfg.game_dir = selected;
      cfg.game_dir_source = "manual";
    }
    await refreshGameDir();
    await refreshCharacters().catch((e) =>
      console.error("refreshCharacters failed:", e),
    );
  } catch (e) {
    alert(t("CONFIG_GAME_DIR_SET_FAILED", { err: tErr(String(e)) }));
  }
}

async function refreshGameStatus(force = false): Promise<void> {
  const el = $("#game-status");
  const now = Date.now();
  if (!force && gameStatusCache && now - gameStatusCache.ts < GAME_STATUS_TTL_MS) {
    const running = gameStatusCache.value;
    el.textContent = running ? t("GAME_RUNNING") : t("GAME_NOT_RUNNING");
    el.classList.toggle("warn", running);
    return;
  }
  try {
    const running = await invoke<boolean>("is_game_running");
    gameStatusCache = { value: running, ts: now };
    el.textContent = running ? t("GAME_RUNNING") : t("GAME_NOT_RUNNING");
    el.classList.toggle("warn", running);
  } catch {
    el.textContent = t("GAME_UNKNOWN");
  }
}

async function refreshBackupList(): Promise<void> {
  const tbody = $("#backup-rows");
  tbody.innerHTML = `<tr><td colspan="3" class="empty">${escapeHtml(t("BACKUP_LOADING"))}</td></tr>`;
  try {
    const list = await invoke<BackupInfo[]>("list_backups");
    if (list.length === 0) {
      tbody.innerHTML = `<tr><td colspan="3" class="empty">${escapeHtml(t("BACKUP_EMPTY"))}</td></tr>`;
      return;
    }
    tbody.innerHTML = "";
    for (const b of list) {
      const tr = document.createElement("tr");
      tr.innerHTML = `
        <td>${fmtDate(b.created_at)}</td>
        <td>${fmtBytes(b.size_bytes)}</td>
        <td class="row-actions">
          <button type="button" class="action-restore" data-name="${escapeAttr(b.name)}">${escapeHtml(t("BACKUP_RESTORE"))}</button>
          <button type="button" class="action-delete" data-name="${escapeAttr(b.name)}">${escapeHtml(t("BACKUP_DELETE"))}</button>
        </td>`;
      tbody.appendChild(tr);
    }
    tbody.querySelectorAll<HTMLButtonElement>(".action-restore").forEach((btn) => {
      btn.addEventListener("click", () => doRestore(btn.dataset.name!));
    });
    tbody.querySelectorAll<HTMLButtonElement>(".action-delete").forEach((btn) => {
      btn.addEventListener("click", () => doDelete(btn.dataset.name!));
    });
  } catch (e) {
    tbody.innerHTML = `<tr><td colspan="3" class="empty">${escapeHtml(tErr(String(e)))}</td></tr>`;
  }
}

async function doCreate(): Promise<void> {
  const btn = $<HTMLButtonElement>("#create-backup");
  btn.disabled = true;
  try {
    await invoke<BackupInfo>("create_backup");
    await refreshBackupList();
  } catch (e) {
    alert(t("BACKUP_CREATE_FAILED", { err: tErr(String(e)) }));
  } finally {
    btn.disabled = false;
  }
}

async function doRestore(name: string): Promise<void> {
  if (!confirm(t("BACKUP_RESTORE_CONFIRM", { name }))) return;
  try {
    const autoName = await invoke<string>("restore_backup", { name });
    alert(t("BACKUP_RESTORE_DONE", { name: autoName }));
    await refreshBackupList();
  } catch (e) {
    alert(t("BACKUP_RESTORE_FAILED", { err: tErr(String(e)) }));
  }
}

async function doDelete(name: string): Promise<void> {
  if (!confirm(t("BACKUP_DELETE_CONFIRM", { name }))) return;
  try {
    await invoke("delete_backup", { name });
    await refreshBackupList();
  } catch (e) {
    alert(t("BACKUP_DELETE_FAILED", { err: tErr(String(e)) }));
  }
}

// ===== 关卡编辑器 =====

let levelRows: LevelRow[] = [];
let levelNames = new Map<string, LevelName>();
let levelPending = new Map<number, boolean>();
let levelFilter = "";

/** 关卡 ID 对应在当前语言下的显示名。无翻译时回退 ID。 */
function displayName(id: string): string {
  const n = levelNames.get(id);
  if (!n) return id;
  const locale = getLocale();
  return (locale === "zh-CN" ? n["zh-CN"] : n.en) || id;
}

function pendingCount(): number {
  return levelPending.size;
}

function renderLevelRows(): void {
  const tbody = $("#levels-rows");
  const filter = levelFilter.trim().toLowerCase();
  const rows = filter
    ? levelRows.filter((r) => {
        const name = displayName(r.id).toLowerCase();
        return r.id.toLowerCase().includes(filter) || name.includes(filter);
      })
    : levelRows;

  if (levelRows.length === 0) {
    tbody.innerHTML = `<tr><td colspan="4" class="empty">${escapeHtml(t("LEVELS_EMPTY"))}</td></tr>`;
    return;
  }
  if (rows.length === 0) {
    tbody.innerHTML = `<tr><td colspan="4" class="empty">${escapeHtml(t("LEVELS_NO_MATCH"))}</td></tr>`;
    return;
  }

  tbody.innerHTML = "";
  for (const r of rows) {
    const pendingVal = levelPending.get(r.line_index);
    const checked = pendingVal !== undefined ? pendingVal : r.completed;
    const solutionCell = r.solution
      ? escapeHtml(r.solution)
      : `<span class="muted">${escapeHtml(t("LEVELS_NO_SOLUTION"))}</span>`;
    const tr = document.createElement("tr");
    tr.innerHTML = `
      <td><input type="checkbox" data-line-index="${r.line_index}" ${checked ? "checked" : ""} /></td>
      <td><div class="level-name">${escapeHtml(displayName(r.id))}</div><div class="muted level-id">${escapeHtml(r.id)}</div></td>
      <td>${solutionCell}</td>
      <td>${r.records}</td>`;
    tbody.appendChild(tr);
  }
  tbody
    .querySelectorAll<HTMLInputElement>("input[type=checkbox][data-line-index]")
    .forEach((cb) => {
      cb.addEventListener("change", () => {
        const idx = Number(cb.dataset.lineIndex);
        const original = levelRows.find((r) => r.line_index === idx)?.completed;
        const nowChecked = cb.checked;
        if (original === nowChecked) {
          levelPending.delete(idx);
        } else {
          levelPending.set(idx, nowChecked);
        }
        updateLevelPendingUI();
      });
    });
}

function updateLevelPendingUI(): void {
  const n = pendingCount();
  const pendingEl = $("#levels-pending");
  pendingEl.textContent = n === 0 ? t("LEVELS_NO_PENDING") : t("LEVELS_PENDING", { n });
  pendingEl.classList.toggle("warn", n > 0);
  $<HTMLButtonElement>("#levels-save").disabled = n === 0;
  $<HTMLButtonElement>("#levels-discard").disabled = n === 0;
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function escapeAttr(s: string): string {
  return escapeHtml(s);
}

async function loadLevels(): Promise<void> {
  const tbody = $("#levels-rows");
  tbody.innerHTML = `<tr><td colspan="4" class="empty">${escapeHtml(t("LEVELS_LOADING"))}</td></tr>`;
  try {
    // 拉关卡行 + 翻译表（后者失败不致命 —— 显示会回退到 ID）
    const [rows, names] = await Promise.all([
      invoke<LevelRow[]>("list_levels"),
      invoke<Record<string, LevelName>>("list_level_names"),
    ]);
    levelRows = rows;
    levelNames = new Map(Object.entries(names));
    levelPending.clear();
    renderLevelRows();
    updateLevelPendingUI();
  } catch (e) {
    tbody.innerHTML = `<tr><td colspan="4" class="empty">${escapeHtml(tErr(String(e)))}</td></tr>`;
  }
}

function discardLevelChanges(): void {
  levelPending.clear();
  renderLevelRows();
  updateLevelPendingUI();
}

async function saveLevelChanges(): Promise<void> {
  if (pendingCount() === 0) return;
  const updates = Array.from(levelPending.entries()).map(([line_index, completed]) => ({
    line_index,
    completed,
  }));
  const saveBtn = $<HTMLButtonElement>("#levels-save");
  saveBtn.disabled = true;
  try {
    const backupName = await invoke<string>("save_levels", { updates });
    alert(t("LEVELS_SAVE_DONE", { name: backupName }));
    await loadLevels();
  } catch (e) {
    alert(t("LEVELS_SAVE_FAILED", { err: tErr(String(e)) }));
    saveBtn.disabled = false;
  }
}

// ===== 电路测试 =====

let testGameAvailable = false;

async function refreshTestView(): Promise<void> {
  const missing = $("#test-game-missing");
  const runBtn = $<HTMLButtonElement>("#test-run");
  try {
    testGameAvailable = await invoke<boolean>("is_game_available");
  } catch {
    testGameAvailable = false;
  }
  missing.hidden = testGameAvailable;
  // 复用全局 levelRows / levelNames；未加载则先加载
  if (levelRows.length === 0) {
    try {
      const [rows, names] = await Promise.all([
        invoke<LevelRow[]>("list_levels"),
        invoke<Record<string, LevelName>>("list_level_names"),
      ]);
      levelRows = rows;
      levelNames = new Map(Object.entries(names));
    } catch {
      // 失败不致命，下拉保持空
    }
  }
  fillTestLevelSelect();
  runBtn.disabled = !testGameAvailable;
}

function fillTestLevelSelect(): void {
  const sel = $<HTMLSelectElement>("#test-level");
  sel.innerHTML = "";
  for (const r of levelRows) {
    const opt = document.createElement("option");
    opt.value = r.id;
    opt.textContent = displayName(r.id);
    sel.appendChild(opt);
  }
  if (levelRows.length > 0) {
    loadTestSchemes(levelRows[0].id);
  }
}

async function loadTestSchemes(levelId: string): Promise<void> {
  const sel = $<HTMLSelectElement>("#test-scheme");
  sel.innerHTML = "";
  let schemes: string[] = [];
  try {
    schemes = await invoke<string[]>("list_schematics", { levelId });
  } catch {
    schemes = [];
  }
  if (schemes.length === 0) {
    const opt = document.createElement("option");
    opt.value = "";
    opt.textContent = t("TEST_NO_SCHEME");
    sel.appendChild(opt);
    $<HTMLButtonElement>("#test-run").disabled = true;
    return;
  }
  for (const s of schemes) {
    const opt = document.createElement("option");
    opt.value = s;
    opt.textContent = s;
    sel.appendChild(opt);
  }
  $<HTMLButtonElement>("#test-run").disabled = !testGameAvailable;
}

async function runTest(): Promise<void> {
  const level = $<HTMLSelectElement>("#test-level").value;
  const scheme = $<HTMLSelectElement>("#test-scheme").value;
  const resultEl = $("#test-result");
  resultEl.hidden = false;
  resultEl.className = "test-result";
  resultEl.textContent = t("TEST_RUNNING");
  const btn = $<HTMLButtonElement>("#test-run");
  btn.disabled = true;
  try {
    const r = await invoke<CircuitTestResult>("test_circuit", {
      levelId: level,
      schemeId: scheme,
    });
    if (r.ok && r.test_result === 0) {
      resultEl.textContent = t("TEST_PASS", { cycles: r.cycles_run });
      resultEl.classList.add("pass");
    } else if (r.ok && r.test_result === 1) {
      resultEl.textContent = t("TEST_WIN", { cycles: r.cycles_run });
      resultEl.classList.add("pass");
    } else if (r.ok) {
      resultEl.textContent = t("TEST_FAIL", { cycles: r.cycles_run });
      resultEl.classList.add("fail");
    } else {
      resultEl.textContent = t("TEST_ERROR", { err: r.error ?? "" });
      resultEl.classList.add("fail");
    }
  } catch (e) {
    resultEl.textContent = t("TEST_ERROR", { err: tErr(String(e)) });
    resultEl.classList.add("fail");
  } finally {
    btn.disabled = !testGameAvailable;
  }
}

async function switchLanguage(newLang: Locale): Promise<void> {
  if (!cfg) return;
  setLocale(newLang);
  applyI18n();
  rerenderDynamic();
  if (cfg.language !== newLang) {
    cfg = { ...cfg, language: newLang };
    try {
      await invoke("set_config", { cfg });
    } catch (e) {
      console.error("failed to persist language:", e);
    }
  }
}

window.addEventListener("DOMContentLoaded", () => {
  $("#save-dir-pick").addEventListener("click", () =>
    pickFolder($<HTMLInputElement>("#save-dir-input"))
  );
  $("#backup-dir-pick").addEventListener("click", () =>
    pickFolder($<HTMLInputElement>("#backup-dir-input"))
  );
  $("#wizard-prev").addEventListener("click", () => showStep(currentStep - 1));
  $("#wizard-next").addEventListener("click", () => showStep(currentStep + 1));
  $("#wizard-finish").addEventListener("click", () => {
    finishWizard().catch((e) => alert(t("OP_FAILED", { err: tErr(String(e)) })));
  });
  $("#reset-config").addEventListener("click", () => {
    resetConfig().catch((e) => alert(t("CONFIG_RESET_FAILED", { err: tErr(String(e)) })));
  });
  $("#create-backup").addEventListener("click", () => {
    doCreate().catch((e) => alert(t("OP_FAILED", { err: tErr(String(e)) })));
  });
  $("#refresh-list").addEventListener("click", () => {
    refreshGameStatus();
    refreshBackupList();
  });
  $("#levels-reload").addEventListener("click", () => {
    loadLevels().catch((e) => alert(t("LEVELS_LOAD_FAILED_ALERT", { err: tErr(String(e)) })));
  });
  $("#levels-discard").addEventListener("click", () => {
    if (pendingCount() === 0) return;
    if (confirm(t("LEVELS_DISCARD_CONFIRM", { n: pendingCount() }))) discardLevelChanges();
  });
  $("#levels-save").addEventListener("click", () => {
    saveLevelChanges().catch((e) => alert(t("OP_FAILED", { err: tErr(String(e)) })));
  });
  $("#levels-search").addEventListener("input", () => {
    levelFilter = $<HTMLInputElement>("#levels-search").value;
    renderLevelRows();
  });
  $("#test-run").addEventListener("click", () => {
    runTest().catch((e) => alert(t("OP_FAILED", { err: tErr(String(e)) })));
  });
  $("#test-level").addEventListener("change", () => {
    const level = $<HTMLSelectElement>("#test-level").value;
    loadTestSchemes(level).catch((e) => console.error("loadTestSchemes failed:", e));
  });
  $("#auto-save").addEventListener("click", () => {
    saveAutoBackup().catch((e) => alert(t("AUTO_SAVE_FAILED", { err: tErr(String(e)) })));
  });
  $("#redetect-game-dir").addEventListener("click", () => {
    redetectGameDir().catch((e) =>
      alert(t("CONFIG_GAME_DIR_REDETECT_FAILED", { err: tErr(String(e)) })),
    );
  });
  $("#pick-game-dir").addEventListener("click", () => {
    pickGameDir().catch((e) =>
      alert(t("CONFIG_GAME_DIR_SET_FAILED", { err: tErr(String(e)) })),
    );
  });
  $("#lang-select").addEventListener("change", () => {
    const v = $<HTMLSelectElement>("#lang-select").value as Locale;
    switchLanguage(v).catch((e) => console.error("switchLanguage failed:", e));
  });
  listen("auto-backup-done", () => {
    refreshBackupList();
  }).catch((e) => console.error("listen failed:", e));
  bindCharacterEvents();
  document.querySelectorAll<HTMLButtonElement>("#main .nav-item").forEach((btn) => {
    btn.addEventListener("click", () => switchView(btn.dataset.view as ViewName));
  });
  // Tauri WebView2 默认拦截 target=_blank，所以 QQ 群链接必须主动调系统浏览器
  $("#sidebar-qq").addEventListener("click", (e) => {
    e.preventDefault();
    const url = $("#sidebar-qq").getAttribute("href") ?? "";
    invoke("open_external_url", { url }).catch((err) =>
      console.error("open_external_url failed:", err)
    );
  });
  init().catch((e) => alert(t("INIT_FAILED", { err: tErr(String(e)) })));
});