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

let cfg: AppConfig | null = null;
let currentStep = 1;

function $<T extends HTMLElement>(sel: string): T {
  const el = document.querySelector<T>(sel);
  if (!el) throw new Error(`element not found: ${sel}`);
  return el;
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

function showMain(): void {
  $("#wizard").hidden = true;
  $("#main").hidden = false;
  $("#current-save-dir").textContent = cfg!.save_dir;
  $("#current-backup-dir").textContent = cfg!.backup_dir;
  $("#current-language").textContent = cfg!.language;
}

async function loadWizardDefaults(): Promise<void> {
  const detect = await invoke<DetectSaveDir>("detect_save_dir");
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

  const backupDefault =
    detect.default_path.replace(/[\\/]Turing Complete[\\/]?$/, "") +
    "/TuringCompleteManager/backups";
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
  showMain();
}

async function resetConfig(): Promise<void> {
  if (!confirm("确认重置配置？存档本身不会被修改，只是回到首次启动向导。")) return;
  cfg = null;
  await showWizard();
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
  init().catch((e) => alert(`初始化失败: ${e}`));
});