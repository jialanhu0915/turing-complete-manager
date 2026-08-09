# Turing Complete Manager

A lightweight desktop save manager for the game *Turing Complete*. Backup and restore your save directory, toggle level completion flags, and let auto-backups catch your progress. Built with Tauri 2 + vanilla TypeScript.

> Not affiliated with, endorsed by, or sponsored by the makers of *Turing Complete*. All game-related content (level IDs, names, translations) is read from the user's local game installation and belongs to its respective owners.

## Features

- **Whole-directory backup / restore** — snapshots your entire save directory into a single zip, with timestamps.
- **Auto-backup** — configurable interval (1–1440 min) and retention count (1–999). Hot-backup supported; runs while the game is open.
- **Level editor** — flip the completion flag on any level via `levels.txt`. Other fields stay byte-perfect; originals are auto-backed up to `<save_dir>/levels_backups/`.
- **Level names in your language** — names are pulled directly from the game's own `translations/` files (English + Simplified Chinese); no separate translation table to maintain.
- **Bilingual UI** — Simplified Chinese and English, switchable at runtime.
- **First-run wizard** — detects the default save directory (`%APPDATA%\Turing Complete`) and suggests the install-dir `backups` subfolder.

## Tech stack

- [Tauri 2](https://tauri.app/) — native shell, Rust backend
- Vanilla TypeScript + Vite — no UI framework
- Rust crates: `zip`, `walkdir`, `chrono`, `serde`, `serde_json`
- MSI installer via WiX 3.14 (vendored at `src-tauri/WixTools314/`)

## Prerequisites

- Node.js 18+
- Rust stable (`rustup default stable`)
- Windows 10/11 (uses `tasklist.exe` to detect the running game; `reg.exe` to find Steam install)
- *Turing Complete* installed via Steam (for level-name translation; the rest of the app works without it)

## Development

```bash
npm install
npm run tauri dev
```

Runs the app in dev mode with hot-reload.

## Build the installer

```bash
npm run tauri build
```

If you're behind a proxy (e.g., mainland China), set the proxy so the Tauri CLI can fetch from GitHub on first build:

```bash
HTTPS_PROXY=http://127.0.0.1:7897 HTTP_PROXY=http://127.0.0.1:7897 npm run tauri build
```

The output is in `src-tauri/target/release/bundle/msi/`:

- `Turing Complete Manager_0.1.0_x64_zh-CN.msi`
- `Turing Complete Manager_0.1.0_x64_en-US.msi`

## Tests

```bash
cd src-tauri && cargo test --lib   # 10 unit tests
npx tsc --noEmit                   # type check
```

## Configuration

Stored at `%APPDATA%\turing-complete-manager\config.json`. Editable via the UI's **配置** / **Configuration** panel; **重置配置** / **Reset** re-opens the wizard.

| Key | Default | Notes |
|---|---|---|
| `save_dir` | `%APPDATA%\Turing Complete` | Where *Turing Complete* stores saves |
| `backup_dir` | `%APPDATA%\turing-complete-manager\backups` | Where the app writes `.zip` snapshots |
| `language` | `zh-CN` | `zh-CN` or `en-US` |
| `auto_backup_enabled` | `false` | Toggle the background scheduler |
| `auto_backup_interval_min` | `30` | 1–1440 |
| `auto_backup_keep` | `20` | Oldest are pruned when exceeded |
| `game_dir` | `null` (auto-detected on startup) | Game install path, cached after first detection |
| `game_dir_source` | `auto` | `auto` (Steam detection) or `manual` (user-specified, not overwritten by re-detect) |

A separate cached file, `level_names.json`, lives next to `config.json`. It is rebuilt automatically when the source `translations/` directory changes (mtime check).

## Project structure

```
.
├── index.html              # single-page UI
├── src/                    # frontend (TS + CSS)
│   ├── main.ts
│   ├── i18n.ts             # translation tables + t() / tErr()
│   └── styles.css
├── src-tauri/
│   ├── src/
│   │   ├── lib.rs          # Tauri commands, auto-backup loop
│   │   ├── backup.rs       # zip / unzip / list / delete
│   │   ├── levels.rs       # levels.txt parser & editor
│   │   ├── translations.rs # Steam path + translation file parser
│   │   └── config.rs       # AppConfig load / save
│   ├── WixTools314/        # vendored WiX (for offline MSI builds)
│   └── tauri.conf.json
└── package.json
```

## License

[MIT](LICENSE) © 2026 jialanhu0915