// 角色替换前端模块
//
// 职责：
//   - 渲染角色列表（预装卡片 + 自建表格）
//   - 渲染状态条
//   - 处理按钮：新建 / 恢复默认 / 应用 / 上传 / 删除 / 复制为我的
//   - canvas 抠图：选背景色 + 阈值滑块实时预览，canvas.toBlob 提交给后端
//
// 不依赖外部图像处理库（image crate 不需要）；图像处理在前端完成。

import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core";
import { applyI18n, t, tErr } from "./i18n";

// ===== 类型（与 Rust 端一一对应） =====

export type CharacterKind = "default" | "custom";

export interface Character {
  id: string;
  name: string;
  kind: CharacterKind;
  has_neutral: boolean;
  has_smile: boolean;
  created_at: string | null;
}

export interface CharacterStatus {
  install_dir: string;
  install_dir_writable: boolean;
  game_available: boolean;
  game_dialogue_dir: string | null;
  snapshot_taken: boolean;
  active_id: string | null;
}

export type Slot = "neutral" | "smile";

// ===== 状态 =====

let characters: Character[] = [];
let status: CharacterStatus | null = null;

interface MattingState {
  characterId: string;
  slot: Slot;
  sourceImage: HTMLImageElement | null;
  /** 当前显示在 canvas 上的处理后图像（用于导出） */
  processedCanvas: HTMLCanvasElement | null;
  bgColor: { r: number; g: number; b: number };
  threshold: number;
  /** 取消时回滚的原始文件路径（用于重复上传） */
  hasExistingFile: boolean;
}
let matting: MattingState | null = null;

// ===== 工具 =====

function $<T extends HTMLElement>(sel: string): T {
  const el = document.querySelector<T>(sel);
  if (!el) throw new Error(`element not found: ${sel}`);
  return el;
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function hexToRgb(hex: string): { r: number; g: number; b: number } {
  const m = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex);
  if (!m) return { r: 255, g: 255, b: 255 };
  return {
    r: parseInt(m[1], 16),
    g: parseInt(m[2], 16),
    b: parseInt(m[3], 16),
  };
}

function rgbToHex(r: number, g: number, b: number): string {
  const h = (n: number) => n.toString(16).padStart(2, "0");
  return `#${h(r)}${h(g)}${h(b)}`;
}

/** 读取图片的 ImageData（用于抠图） */
function getImageData(img: HTMLImageElement): ImageData {
  const c = document.createElement("canvas");
  c.width = img.naturalWidth;
  c.height = img.naturalHeight;
  const ctx = c.getContext("2d", { willReadFrequently: true });
  if (!ctx) throw new Error("canvas 2d context unavailable");
  ctx.drawImage(img, 0, 0);
  return ctx.getImageData(0, 0, c.width, c.height);
}

/** 在副本上应用抠图，返回新的 ImageData */
function applyKeying(
  src: ImageData,
  bg: { r: number; g: number; b: number },
  threshold: number,
): ImageData {
  const out = new ImageData(new Uint8ClampedArray(src.data), src.width, src.height);
  const t2 = threshold * threshold;
  for (let i = 0; i < out.data.length; i += 4) {
    const dr = out.data[i] - bg.r;
    const dg = out.data[i + 1] - bg.g;
    const db = out.data[i + 2] - bg.b;
    if (dr * dr + dg * dg + db * db <= t2) {
      out.data[i + 3] = 0;
    }
  }
  return out;
}

function drawMattingToCanvas(
  imageData: ImageData,
  canvas: HTMLCanvasElement,
): void {
  canvas.width = imageData.width;
  canvas.height = imageData.height;
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  ctx.putImageData(imageData, 0, 0);
}

/** 取原图四个角的平均色作为初始背景色猜测 */
function sampleCornerColor(img: HTMLImageElement): { r: number; g: number; b: number } {
  const data = getImageData(img);
  const w = data.width;
  const h = data.height;
  const points: Array<[number, number]> = [
    [0, 0],
    [w - 1, 0],
    [0, h - 1],
    [w - 1, h - 1],
  ];
  let r = 0, g = 0, b = 0;
  for (const [x, y] of points) {
    const i = (y * w + x) * 4;
    r += data.data[i];
    g += data.data[i + 1];
    b += data.data[i + 2];
  }
  return { r: Math.round(r / 4), g: Math.round(g / 4), b: Math.round(b / 4) };
}

// ===== 状态条渲染 =====

function renderStatus(): void {
  const el = $<HTMLElement>("#character-status");
  const warn = $<HTMLElement>("#character-warn");
  warn.hidden = true;
  warn.textContent = "";
  if (!status) {
    el.textContent = t("CHARACTER_STATUS_LOADING");
    return;
  }
  const lines: string[] = [];
  if (status.game_available && status.game_dialogue_dir) {
    lines.push(t("CHARACTER_GAME_OK", { path: status.game_dialogue_dir }));
  } else {
    lines.push(t("CHARACTER_GAME_MISSING"));
  }
  lines.push(status.snapshot_taken ? t("CHARACTER_SNAPSHOT_OK") : t("CHARACTER_SNAPSHOT_PENDING"));
  el.textContent = lines.join(" · ");

  if (!status.install_dir_writable) {
    warn.textContent = t("CHARACTER_INSTALL_READONLY");
    warn.hidden = false;
  }
}

// ===== 列表渲染 =====

function thumbnailDataUrl(ch: Character, slot: Slot): string | null {
  // 缩略图走 convertFileSrc 拿本地文件 URL。
  // 路径：default → install_dir/default_characters/<id>/<slot>.png
  //       custom → install_dir/characters/<uuid>/<slot>.png
  const base = status?.install_dir;
  if (!base) return null;
  const rel = ch.kind === "default"
    ? `default_characters/${ch.id.replace(/^default:/, "")}/${slot}.png`
    : `characters/${ch.id.replace(/^custom:/, "")}/${slot}.png`;
  // Windows path: 反斜杠变正斜杠
  const fullPath = `${base.replace(/\\/g, "/")}/${rel}`;
  return convertFileSrc(fullPath);
}

function renderDefaultGrid(): void {
  const wrap = $<HTMLElement>("#character-defaults");
  const defaults = characters.filter((c) => c.kind === "default");
  if (defaults.length === 0) {
    wrap.innerHTML = `<div class="character-empty" data-i18n="CHARACTER_DEFAULT_EMPTY">${escapeHtml(t("CHARACTER_DEFAULT_EMPTY"))}</div>`;
    return;
  }
  wrap.innerHTML = defaults
    .map((ch) => {
      const isActive = status?.active_id === ch.id;
      const thumb = thumbnailDataUrl(ch, "neutral");
      const thumbHtml = thumb
        ? `<img class="character-thumb" src="${escapeHtml(thumb)}" alt="" />`
        : `<div class="character-thumb character-thumb-empty">?</div>`;
      const activeBadge = isActive
        ? `<span class="character-active-badge" data-i18n="CHARACTER_ACTIVE_BADGE">${escapeHtml(t("CHARACTER_ACTIVE_BADGE"))}</span>`
        : "";
      const applyDisabled = !status?.install_dir_writable ? "disabled" : "";
      return `
        <div class="character-card ${isActive ? "active" : ""}">
          ${thumbHtml}
          <div class="character-card-name">${escapeHtml(ch.name)} ${activeBadge}</div>
          <div class="character-card-actions">
            <button type="button" data-character-apply="${escapeHtml(ch.id)}" ${applyDisabled} data-i18n="CHARACTER_APPLY">${escapeHtml(t("CHARACTER_APPLY"))}</button>
            <button type="button" data-character-duplicate="${escapeHtml(ch.id)}" data-i18n="CHARACTER_DUPLICATE">${escapeHtml(t("CHARACTER_DUPLICATE"))}</button>
          </div>
        </div>`;
    })
    .join("");
  applyI18n();
}

function renderCustomTable(): void {
  const tbody = $<HTMLElement>("#character-customs");
  const customs = characters.filter((c) => c.kind === "custom");
  if (customs.length === 0) {
    tbody.innerHTML = `<tr><td colspan="4" class="empty" data-i18n="CHARACTER_EMPTY">${escapeHtml(t("CHARACTER_EMPTY"))}</td></tr>`;
    return;
  }
  tbody.innerHTML = customs
    .map((ch) => {
      const isActive = status?.active_id === ch.id;
      const neutralThumb = ch.has_neutral && thumbnailDataUrl(ch, "neutral");
      const smileThumb = ch.has_smile && thumbnailDataUrl(ch, "smile");
      const neutralCell = neutralThumb
        ? `<img class="character-thumb" src="${escapeHtml(neutralThumb)}" alt="" />`
        : `<span class="muted" data-i18n="CHARACTER_SLOT_EMPTY">${escapeHtml(t("CHARACTER_SLOT_EMPTY"))}</span>`;
      const smileCell = smileThumb
        ? `<img class="character-thumb" src="${escapeHtml(smileThumb)}" alt="" />`
        : `<span class="muted" data-i18n="CHARACTER_SLOT_EMPTY">${escapeHtml(t("CHARACTER_SLOT_EMPTY"))}</span>`;
      const activeBadge = isActive
        ? ` <span class="character-active-badge" data-i18n="CHARACTER_ACTIVE_BADGE">${escapeHtml(t("CHARACTER_ACTIVE_BADGE"))}</span>`
        : "";
      const applyDisabled = !status?.install_dir_writable ? "disabled" : "";
      const slots = [
        { slot: "neutral" as Slot, label: t("CHARACTER_UPLOAD") + " " + t("CHARACTER_SLOT_NEUTRAL") },
        { slot: "smile" as Slot, label: t("CHARACTER_UPLOAD") + " " + t("CHARACTER_SLOT_SMILE") },
      ];
      const uploadButtons = slots
        .map(
          (s) =>
            `<button type="button" data-character-upload="${escapeHtml(ch.id)}|${s.slot}" data-i18n="CHARACTER_UPLOAD">${escapeHtml(t("CHARACTER_UPLOAD"))} ${escapeHtml(t(s.slot === "neutral" ? "CHARACTER_SLOT_NEUTRAL" : "CHARACTER_SLOT_SMILE"))}</button>`,
        )
        .join("");
      return `
        <tr class="${isActive ? "active-row" : ""}">
          <td>${neutralCell}</td>
          <td>${smileCell}</td>
          <td>${escapeHtml(ch.name)}${activeBadge}</td>
          <td class="row-actions">
            <button type="button" class="action-restore" data-character-apply="${escapeHtml(ch.id)}" ${applyDisabled} data-i18n="CHARACTER_APPLY">${escapeHtml(t("CHARACTER_APPLY"))}</button>
            ${uploadButtons}
            <button type="button" class="action-delete" data-character-delete="${escapeHtml(ch.id)}" data-i18n="CHARACTER_DELETE">${escapeHtml(t("CHARACTER_DELETE"))}</button>
          </td>
        </tr>`;
    })
    .join("");
  applyI18n();
}

// ===== 数据加载 =====

export async function refreshCharacters(): Promise<void> {
  try {
    const [s, list] = await Promise.all([
      invoke<CharacterStatus>("character_status"),
      invoke<Character[]>("list_characters"),
    ]);
    status = s;
    characters = list;
  } catch (e) {
    status = null;
    characters = [];
    renderStatus();
    renderDefaultGrid();
    renderCustomTable();
    console.error("character refresh failed:", tErr(String(e)));
    return;
  }
  renderStatus();
  renderDefaultGrid();
  renderCustomTable();
}

// ===== 操作 =====

async function doCreate(name: string): Promise<void> {
  try {
    await invoke<Character>("create_character", { name });
    await refreshCharacters();
  } catch (e) {
    alert(t("CHARACTER_CREATE_FAILED", { err: tErr(String(e)) }));
  }
}

async function doApply(id: string): Promise<void> {
  try {
    await invoke("apply_character", { id });
    await refreshCharacters();
  } catch (e) {
    alert(t("CHARACTER_APPLY_FAILED", { err: tErr(String(e)) }));
  }
}

async function doDelete(id: string, name: string): Promise<void> {
  if (!confirm(t("CHARACTER_CONFIRM_DELETE", { name }))) return;
  try {
    await invoke("delete_character", { id });
    await refreshCharacters();
  } catch (e) {
    alert(t("CHARACTER_DELETE_FAILED", { err: tErr(String(e)) }));
  }
}

async function doDuplicate(defaultId: string, newName: string): Promise<void> {
  try {
    await invoke<Character>("duplicate_character", { id: defaultId, newName });
    await refreshCharacters();
  } catch (e) {
    alert(t("CHARACTER_DUPLICATE_FAILED", { err: tErr(String(e)) }));
  }
}

async function doRestoreDefault(): Promise<void> {
  try {
    await invoke("restore_default_character");
    await refreshCharacters();
  } catch (e) {
    alert(t("CHARACTER_RESTORE_FAILED", { err: tErr(String(e)) }));
  }
}

// ===== 抠图弹窗 =====

function resetMattingModal(): void {
  matting = null;
  const canvas = $<HTMLCanvasElement>("#matting-canvas");
  const ctx = canvas.getContext("2d");
  if (ctx) ctx.clearRect(0, 0, canvas.width, canvas.height);
  canvas.width = 0;
  canvas.height = 0;
  $<HTMLElement>("#matting-empty").hidden = false;
  $<HTMLButtonElement>("#matting-apply").disabled = true;
  $<HTMLInputElement>("#matting-bg").value = "#ffffff";
  $<HTMLInputElement>("#matting-threshold").value = "30";
  $<HTMLElement>("#matting-threshold-val").textContent = "30";
}

export function openMattingModal(characterId: string, slot: Slot): void {
  resetMattingModal();
  matting = {
    characterId,
    slot,
    sourceImage: null,
    processedCanvas: null,
    bgColor: { r: 255, g: 255, b: 255 },
    threshold: 30,
    hasExistingFile: false,
  };
  $<HTMLDialogElement>("#dialog-character-matting").showModal();
}

function reRenderMattingPreview(): void {
  if (!matting?.sourceImage) return;
  const src = getImageData(matting.sourceImage);
  const processed = applyKeying(src, matting.bgColor, matting.threshold);
  const canvas = $<HTMLCanvasElement>("#matting-canvas");
  drawMattingToCanvas(processed, canvas);
  matting.processedCanvas = canvas;
}

async function loadMattingImage(file: File): Promise<void> {
  if (!matting) return;
  const url = URL.createObjectURL(file);
  try {
    const img = new Image();
    img.src = url;
    await new Promise<void>((resolve, reject) => {
      img.onload = () => resolve();
      img.onerror = () => reject(new Error("image load failed"));
    });
    matting.sourceImage = img;
    matting.bgColor = sampleCornerColor(img);
    $<HTMLInputElement>("#matting-bg").value = rgbToHex(
      matting.bgColor.r,
      matting.bgColor.g,
      matting.bgColor.b,
    );
    $<HTMLElement>("#matting-empty").hidden = true;
    $<HTMLButtonElement>("#matting-apply").disabled = false;
    reRenderMattingPreview();
  } finally {
    URL.revokeObjectURL(url);
  }
}

async function applyMatting(): Promise<void> {
  if (!matting?.processedCanvas) return;
  const applyBtn = $<HTMLButtonElement>("#matting-apply");
  applyBtn.disabled = true;
  try {
    const blob = await new Promise<Blob | null>((resolve) =>
      matting!.processedCanvas!.toBlob((b) => resolve(b), "image/png"),
    );
    if (!blob) throw new Error("canvas toBlob returned null");
    const buf = await blob.arrayBuffer();
    const b64 = arrayBufferToBase64(buf);
    await invoke("save_character_image", {
      id: matting.characterId,
      slot: matting.slot,
      pngBase64: b64,
    });
    $<HTMLDialogElement>("#dialog-character-matting").close();
    await refreshCharacters();
  } catch (e) {
    applyBtn.disabled = false;
    alert(t("CHARACTER_UPLOAD_FAILED", { err: tErr(String(e)) }));
  }
}

function arrayBufferToBase64(buf: ArrayBuffer): string {
  const bytes = new Uint8Array(buf);
  let s = "";
  for (let i = 0; i < bytes.length; i++) s += String.fromCharCode(bytes[i]);
  return btoa(s);
}

// ===== 事件绑定 =====

export function bindCharacterEvents(): void {
  $<HTMLButtonElement>("#character-create").addEventListener("click", () => {
    const dlg = $<HTMLDialogElement>("#dialog-character-create");
    ($<HTMLInputElement>("#character-name-input")).value = "";
    dlg.showModal();
  });

  $<HTMLButtonElement>("#character-restore-default").addEventListener("click", () => {
    doRestoreDefault().catch((e) => console.error("restore default failed:", e));
  });

  // 新建弹窗：用 form returnValue 区分 confirm/cancel
  const createDlg = $<HTMLDialogElement>("#dialog-character-create");
  createDlg.addEventListener("close", () => {
    if (createDlg.returnValue !== "confirm") return;
    const name = $<HTMLInputElement>("#character-name-input").value.trim();
    if (!name) return;
    doCreate(name).catch((e) => console.error("create character failed:", e));
  });

  // 复制为我的弹窗
  const dupDlg = $<HTMLDialogElement>("#dialog-character-duplicate");
  let pendingDuplicateId: string | null = null;
  // 暴露给事件代理用
  (window as any).__openDuplicateDialog = (id: string) => {
    pendingDuplicateId = id;
    ($<HTMLInputElement>("#character-duplicate-name")).value = "";
    dupDlg.showModal();
  };
  dupDlg.addEventListener("close", () => {
    if (dupDlg.returnValue !== "confirm" || !pendingDuplicateId) return;
    const newName = $<HTMLInputElement>("#character-duplicate-name").value.trim();
    if (!newName) return;
    doDuplicate(pendingDuplicateId, newName).catch((e) =>
      console.error("duplicate failed:", e),
    );
    pendingDuplicateId = null;
  });

  // 抠图弹窗按钮
  $<HTMLButtonElement>("#matting-cancel").addEventListener("click", () => {
    $<HTMLDialogElement>("#dialog-character-matting").close();
  });
  $<HTMLButtonElement>("#matting-apply").addEventListener("click", () => {
    applyMatting().catch((e) => console.error("apply matting failed:", e));
  });
  $<HTMLButtonElement>("#matting-pick-file").addEventListener("click", () => {
    ($<HTMLInputElement>("#matting-file-input")).click();
  });
  $<HTMLInputElement>("#matting-file-input").addEventListener("change", async (e) => {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    try {
      await loadMattingImage(file);
    } catch (err) {
      alert(t("CHARACTER_UPLOAD_FAILED", { err: String(err) }));
    } finally {
      input.value = ""; // 允许重选同一文件
    }
  });
  $<HTMLInputElement>("#matting-bg").addEventListener("input", (e) => {
    if (!matting) return;
    matting.bgColor = hexToRgb((e.target as HTMLInputElement).value);
    reRenderMattingPreview();
  });
  $<HTMLInputElement>("#matting-threshold").addEventListener("input", (e) => {
    if (!matting) return;
    const v = parseInt((e.target as HTMLInputElement).value, 10);
    matting.threshold = v;
    $<HTMLElement>("#matting-threshold-val").textContent = String(v);
    reRenderMattingPreview();
  });

  // 列表内按钮（事件代理）
  document.addEventListener("click", (e) => {
    const target = e.target as HTMLElement;
    const applyId = target.getAttribute("data-character-apply");
    if (applyId) {
      doApply(applyId).catch((err) => console.error("apply failed:", err));
      return;
    }
    const dupId = target.getAttribute("data-character-duplicate");
    if (dupId) {
      (window as any).__openDuplicateDialog?.(dupId);
      return;
    }
    const uploadAttr = target.getAttribute("data-character-upload");
    if (uploadAttr) {
      const [id, slot] = uploadAttr.split("|") as [string, Slot];
      openMattingModal(id, slot);
      return;
    }
    const delId = target.getAttribute("data-character-delete");
    if (delId) {
      const ch = characters.find((c) => c.id === delId);
      const name = ch?.name ?? delId;
      doDelete(delId, name).catch((err) => console.error("delete failed:", err));
      return;
    }
  });
}
