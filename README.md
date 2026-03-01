# CodexRelay

跨设备传递/备份 Codex CLI session（`.jsonl`）的管理工具（macOS / Windows / Linux）。

技术栈：Tauri v2 + React + TypeScript + SQLite（后端 Rust）。

## Features (MVP)

- Sessions: scan `CODEX_HOME/sessions/**/rollout-*.jsonl`
- Export: create a portable `bundle.zip` (with `manifest.json` + `rollout.jsonl`), 默认导出到系统 Downloads；支持“合并为一个 zip”或“每会话单独 zip”
- Import: 支持多选 zip 导入；支持导入“合并导出包”（外层 zip 内含 `bundles/*.zip`）；校验 sha256/size + 冲突策略（recommended: 有冲突则改ID导入）
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
  - Windows: `bundle/nsis/*-setup.exe`（需要在 Windows 上构建；或用 GitHub Actions）
  - Linux(Ubuntu): `bundle/deb/*.deb`

注：为避免 Linux 产物过大（AppImage 往往会打包大量依赖导致体积显著增大），当前默认只构建 `deb`；如需 AppImage / rpm，可在 `src-tauri/tauri.conf.json` 的 `bundle.targets` 里补回。

## macOS “已损坏，无法打开”

结论：如果不做 Apple Developer ID 签名 + 公证（notarization），从浏览器下载的 DMG/App 很可能会被 macOS Gatekeeper 拦截并提示“已损坏，无法打开”（这是系统安全机制，并非一定是包真的坏了）。

免费分发场景下，通常只能给用户提供“放行”方式：

1) 右键（或按住 Control 点击）App -> “打开”（会出现额外的允许打开选项）

如果你是自己使用（信任该来源），可以在把 App 拖到 `/Applications` 后执行：

```bash
sudo xattr -dr com.apple.quarantine /Applications/CodexRelay.app
```

GitHub Actions：见 `.github/workflows/build-bundles.yml`（支持手动触发 `workflow_dispatch`；打 tag `v*` 会自动构建并发布 GitHub Release，附带 `SHA256SUMS.txt` 校验文件）。

## Install (macOS via Homebrew)

提供一个 Homebrew Tap（由另一个账号维护）：`star-alp/homebrew-tap-CodexRelay`

```bash
brew tap star-alp/tap-codexrelay
brew install --cask codexrelay
```

注：Homebrew 的 `--no-quarantine` 参数已 deprecated（未来可能移除）。如遇 Gatekeeper 拦截，请按上文右键“打开”或执行 `xattr` 放行即可。

## Update (macOS)

- 如果你是通过 Homebrew 安装（`brew list --cask codexrelay` 能查到）：
  - `brew update && brew upgrade --cask codexrelay`
- 如果你看到 `Error: Cask 'codexrelay' is not installed.`，说明你之前不是通过 Homebrew 安装（例如手动 DMG 拖拽）：
  - 继续用 DMG 更新：下载最新 Release 的 `.dmg`，将 App 拖到 `/Applications` 覆盖即可（如遇 Gatekeeper，按上文 `xattr` 放行）。
  - 或让 Homebrew “接管”管理（会覆盖 `/Applications/CodexRelay.app`，不影响应用数据/存档库）：
    - `brew tap star-alp/tap-codexrelay && brew install --cask --force codexrelay`
- 应用内：`设置 -> 更新 -> 检查更新`（会提示最新版本并提供打开 Release 的按钮）。

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
