import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

interface AppConfig {
  save_dir: string;
  backup_dir: string;
  language: string;
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

async function init(): Promise<void> {
  cfg = await invoke<AppConfig | null>("get_config");
  if (cfg) {
    showMain();
  } else {
    await showWizard();
  }
  const v = await invoke<string>("app_version").catch(() => "?");
  $("#app-version").textContent = v;
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
  await refreshGameStatus();
  await refreshBackupList();
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
  };
  await invoke("set_config", { cfg });
  await showMain();
}

async function resetConfig(): Promise<void> {
  if (!confirm("确认重置配置？存档本身不会被修改，只是回到首次启动向导。")) return;
  cfg = null;
  await showWizard();
}

async function refreshGameStatus(): Promise<void> {
  const el = $("#game-status");
  try {
    const running = await invoke<boolean>("is_game_running");
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
  init().catch((e) => alert(`初始化失败: ${e}`));
});