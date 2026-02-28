# CodexRelay

跨设备传递/备份 Codex CLI session（`.jsonl`）的管理工具（macOS / Windows / Linux）。

技术栈：Tauri v2 + React + TypeScript + SQLite（后端 Rust）。

## Features (MVP)

- Sessions: scan `CODEX_HOME/sessions/**/rollout-*.jsonl`
- Export: create a portable `bundle.zip` (with `manifest.json` + `rollout.jsonl`)
- Import: validate sha256/size + conflict strategy (recommended: import as new when different sha256)
- History: vault + SQLite records, manual delete
- Restore: restore any history version back to `CODEX_HOME` (also recorded)
- Change ID: rewrite `session_meta.payload.id` (does not touch `forked_from_id`)

## Dev

```bash
pnpm install
pnpm tauri dev
```

## Build (Desktop Bundles)

```bash
pnpm install
pnpm tauri build
```

产物目录（不同平台会生成不同格式）：

- `src-tauri/target/release/bundle/**`
  - macOS: `bundle/dmg/*.dmg`
  - Windows: `bundle/nsis/*.exe` / `bundle/msi/*.msi`（需要在 Windows 上构建；或用 GitHub Actions）

GitHub Actions：见 `.github/workflows/build-bundles.yml`（支持手动触发 `workflow_dispatch`；打 tag `v*` 会自动构建并发布 GitHub Release，附带 `SHA256SUMS.txt` 校验文件）。

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
