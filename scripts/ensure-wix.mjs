// 确保 Tauri 缓存目录里有 WixTools314，避免每次构建都去 GitHub 下载。
// 首次构建：从项目 src-tauri/WixTools314/ 复制到用户本地缓存。
// 后续构建：检测到缓存已有 candle.exe 则跳过。

import { existsSync, mkdirSync, cpSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { dirname } from "node:path";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const projectRoot = join(__dirname, "..");

const sourceDir = join(projectRoot, "src-tauri", "WixTools314");
const cacheDir = join(
  process.env.LOCALAPPDATA ?? join(process.env.USERPROFILE ?? "", "AppData", "Local"),
  "tauri",
  "WixTools314"
);
const probe = join(cacheDir, "candle.exe");

if (existsSync(probe)) {
  console.log(`[ensure-wix] cache present, skip: ${cacheDir}`);
  process.exit(0);
}

if (!existsSync(sourceDir)) {
  console.error(`[ensure-wix] source missing: ${sourceDir}`);
  console.error(`[ensure-wix] run 'npm run setup:wix' or place WixTools314 at the path above.`);
  process.exit(1);
}

console.log(`[ensure-wix] copying ${sourceDir} -> ${cacheDir}`);
mkdirSync(join(cacheDir, ".."), { recursive: true });
cpSync(sourceDir, cacheDir, { recursive: true });
console.log("[ensure-wix] done");