import { invoke } from "@tauri-apps/api/core";

const versionEl = document.querySelector<HTMLSpanElement>("#app-version");

window.addEventListener("DOMContentLoaded", async () => {
  if (!versionEl) return;
  try {
    versionEl.textContent = await invoke("app_version");
  } catch (err) {
    versionEl.textContent = `? (${err})`;
  }
});