import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

interface AppConfig {
  save_dir: string;
  backup_dir: string;
  language: string;
  auto_backup_enabled: boolean;
  auto_backup_interval_min: number;
  auto_backup_keep: number;
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

let cfg: AppConfig | null = null;
let currentStep = 1;

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
  // 已经是本地时间 ISO 字符串，把 T 换成空格更好读
  return iso.replace("T", " ");
}

// 游戏状态短期缓存：避免每次刷新都启动 tasklist 子进程（~200ms 开销）
const GAME_STATUS_TTL_MS = 5000;
let gameStatusCache: { value: boolean; ts: number } | null = null;

async function init(): Promise<void> {
  // 并行获取配置和版本号，少一轮 RTT
  const [configResult, version] = await Promise.all([
    invoke<AppConfig | null>("get_config"),
    invoke<string>("app_version").catch(() => "?"),
  ]);
  cfg = configResult;
  $("#app-version").textContent = version;
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
  fillAutoBackupForm();
  // 并行：游戏状态 + 备份列表
  await Promise.all([refreshGameStatus(), refreshBackupList()]);
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
    alert("间隔必须在 1-1440 分钟之间");
    return null;
  }
  if (!Number.isFinite(keep) || keep < 1 || keep > 999) {
    alert("保留个数必须在 1-999 之间");
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
  status.textContent = "保存中…";
  try {
    await invoke("set_config", { cfg: next });
    cfg = next;
    status.textContent = "✓ 已保存";
    status.classList.remove("warn");
  } catch (e) {
    status.textContent = `✗ ${e}`;
    status.classList.add("warn");
  } finally {
    btn.disabled = false;
  }
}

async function loadWizardDefaults(): Promise<void> {
  const [detect, installDir] = await Promise.all([
    invoke<DetectSaveDir>("detect_save_dir"),
    invoke<string>("detect_install_dir"),
  ]);
  const saveInput = $<HTMLInputElement>("#save-dir-input");
  saveInput.value = detect.default_path;

  const hint = $("#save-dir-hint");
  if (detect.exists) {
    hint.textContent = "✓ 已检测到存档目录";
    hint.classList.remove("warn");
  } else {
    hint.textContent = "⚠ 未在该路径检测到存档目录，请确认游戏至少启动过一次";
    hint.classList.add("warn");
  }

  const backupDefault = installDir
    ? installDir.replace(/[\\/]+$/, "") + "\\backups"
    : "";
  $<HTMLInputElement>("#backup-dir-input").value = backupDefault;
}

function showStep(n: number): void {
  currentStep = n;
  for (let i = 1; i <= 3; i++) {
    $(`#step-${i}`).hidden = i !== n;
  }
  $("#wizard-step").textContent = String(n);
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
    alert("存档目录和备份目录都不能为空");
    return;
  }
  cfg = {
    save_dir: saveDir,
    backup_dir: backupDir,
    language: readSelectedLang(),
    auto_backup_enabled: false,
    auto_backup_interval_min: 30,
    auto_backup_keep: 20,
  };
  await invoke("set_config", { cfg });
  await showMain();
}

async function resetConfig(): Promise<void> {
  if (!confirm("确认重置配置？存档本身不会被修改，只是回到首次启动向导。")) return;
  cfg = null;
  await showWizard();
}

async function refreshGameStatus(force = false): Promise<void> {
  const el = $("#game-status");
  const now = Date.now();
  if (!force && gameStatusCache && now - gameStatusCache.ts < GAME_STATUS_TTL_MS) {
    const running = gameStatusCache.value;
    el.textContent = running ? "游戏：运行中" : "游戏：未运行";
    el.classList.toggle("warn", running);
    return;
  }
  try {
    const running = await invoke<boolean>("is_game_running");
    gameStatusCache = { value: running, ts: now };
    el.textContent = running ? "游戏：运行中" : "游戏：未运行";
    el.classList.toggle("warn", running);
  } catch {
    el.textContent = "游戏状态未知";
  }
}

async function refreshBackupList(): Promise<void> {
  const tbody = $("#backup-rows");
  tbody.innerHTML = `<tr><td colspan="3" class="empty">加载中…</td></tr>`;
  try {
    const list = await invoke<BackupInfo[]>("list_backups");
    if (list.length === 0) {
      tbody.innerHTML = `<tr><td colspan="3" class="empty">暂无备份。点击「立即备份」创建第一个。</td></tr>`;
      return;
    }
    tbody.innerHTML = "";
    for (const b of list) {
      const tr = document.createElement("tr");
      tr.innerHTML = `
        <td>${fmtDate(b.created_at)}</td>
        <td>${fmtBytes(b.size_bytes)}</td>
        <td class="row-actions">
          <button type="button" class="action-restore" data-name="${b.name}">恢复</button>
          <button type="button" class="action-delete" data-name="${b.name}">删除</button>
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
    tbody.innerHTML = `<tr><td colspan="3" class="empty">加载失败：${e}</td></tr>`;
  }
}

async function doCreate(): Promise<void> {
  const btn = $<HTMLButtonElement>("#create-backup");
  btn.disabled = true;
  try {
    await invoke<BackupInfo>("create_backup");
    await refreshBackupList();
  } catch (e) {
    alert(`备份失败：${e}`);
  } finally {
    btn.disabled = false;
  }
}

async function doRestore(name: string): Promise<void> {
  if (!confirm(`确认恢复到备份「${name}」？\n恢复前会自动保存当前状态，可以再回退。`)) return;
  try {
    const autoName = await invoke<string>("restore_backup", { name });
    alert(`恢复完成。\n回退用快照：${autoName}`);
    await refreshBackupList();
  } catch (e) {
    alert(`恢复失败：${e}`);
  }
}

async function doDelete(name: string): Promise<void> {
  if (!confirm(`确认删除备份「${name}」？此操作不可撤销。`)) return;
  try {
    await invoke("delete_backup", { name });
    await refreshBackupList();
  } catch (e) {
    alert(`删除失败：${e}`);
  }
}

// ===== 关卡编辑器 =====

let levelRows: LevelRow[] = [];
// 用户未保存的修改：lineIndex -> completed
let levelPending = new Map<number, boolean>();
let levelFilter = "";

function pendingCount(): number {
  return levelPending.size;
}

function renderLevelRows(): void {
  const tbody = $("#levels-rows");
  const filter = levelFilter.trim().toLowerCase();
  const rows = filter
    ? levelRows.filter((r) => r.id.toLowerCase().includes(filter))
    : levelRows;

  if (levelRows.length === 0) {
    tbody.innerHTML = `<tr><td colspan="4" class="empty">未读取。点击「重新读取」加载。</td></tr>`;
    return;
  }
  if (rows.length === 0) {
    tbody.innerHTML = `<tr><td colspan="4" class="empty">没有匹配的关卡。</td></tr>`;
    return;
  }

  tbody.innerHTML = "";
  for (const r of rows) {
    const pendingVal = levelPending.get(r.line_index);
    const checked = pendingVal !== undefined ? pendingVal : r.completed;
    const tr = document.createElement("tr");
    tr.innerHTML = `
      <td><input type="checkbox" data-line-index="${r.line_index}" ${checked ? "checked" : ""} /></td>
      <td>${escapeHtml(r.id)}</td>
      <td>${escapeHtml(r.solution) || "<span class='muted'>(无)</span>"}</td>
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
          // 恢复成原始值，从 pending 中移除
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
  pendingEl.textContent = n === 0 ? "无修改" : `待保存 ${n} 处`;
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

async function loadLevels(): Promise<void> {
  const tbody = $("#levels-rows");
  tbody.innerHTML = `<tr><td colspan="4" class="empty">加载中…</td></tr>`;
  try {
    levelRows = await invoke<LevelRow[]>("list_levels");
    levelPending.clear();
    renderLevelRows();
    updateLevelPendingUI();
  } catch (e) {
    tbody.innerHTML = `<tr><td colspan="4" class="empty">加载失败：${escapeHtml(String(e))}</td></tr>`;
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
    alert(`保存完成。\n原文件已备份：${backupName}`);
    await loadLevels();
  } catch (e) {
    alert(`保存失败：${e}`);
    saveBtn.disabled = false;
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
    finishWizard().catch((e) => alert(`保存失败: ${e}`));
  });
  $("#reset-config").addEventListener("click", () => {
    resetConfig().catch((e) => alert(`重置失败: ${e}`));
  });
  $("#create-backup").addEventListener("click", () => {
    doCreate().catch((e) => alert(`操作失败: ${e}`));
  });
  $("#refresh-list").addEventListener("click", () => {
    refreshGameStatus();
    refreshBackupList();
  });
  $("#levels-reload").addEventListener("click", () => {
    loadLevels().catch((e) => alert(`读取失败: ${e}`));
  });
  $("#levels-discard").addEventListener("click", () => {
    if (pendingCount() === 0) return;
    if (confirm(`确认放弃 ${pendingCount()} 处未保存修改？`)) discardLevelChanges();
  });
  $("#levels-save").addEventListener("click", () => {
    saveLevelChanges().catch((e) => alert(`操作失败: ${e}`));
  });
  $("#levels-search").addEventListener("input", () => {
    levelFilter = $<HTMLInputElement>("#levels-search").value;
    renderLevelRows();
  });
  $("#auto-save").addEventListener("click", () => {
    saveAutoBackup().catch((e) => alert(`保存失败: ${e}`));
  });
  // 后台自动备份完成后刷新列表
  listen("auto-backup-done", () => {
    refreshBackupList();
  }).catch((e) => console.error("listen failed:", e));
  init().catch((e) => alert(`初始化失败: ${e}`));
});