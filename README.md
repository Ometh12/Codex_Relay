# CodexRelay

跨设备传递/备份 Codex CLI session（`.jsonl`）的桌面管理工具（macOS / Windows / Linux）。

目标：让你在不同电脑/不同系统上，也能通过 `codex resume <session_id>` 继续同一个会话（通过导出/导入 session 文件实现）。

注：本项目为社区工具，和 OpenAI 无官方从属关系。

技术栈：Tauri v2 + React + TypeScript + SQLite（后端 Rust）。

## Why / 使用场景

- 多平台项目（Win / macOS / Linux）来回测试，需要把同一个 Codex 会话带到另一台机器继续解决兼容问题
- 两台电脑协同：用微信/网盘/邮件/AirDrop 传 zip 包即可迁移会话
- 备份：在本地建立“会话版本库”（vault + 历史记录），防止手滑覆盖/误删

## How It Works / 工作方式

Codex CLI 会把会话写在本地 `CODEX_HOME/sessions/**/rollout-*.jsonl`（默认常见路径：`~/.codex/`）。

CodexRelay 负责：

1) 扫描会话并展示列表
2) 导出：把选中的会话打包为 zip（可合并为一个包或每会话单独包；默认导出到系统 Downloads）
3) 导入：先解析/校验（manifest + size/sha256）并预览，再按冲突策略写入 `CODEX_HOME`
4) 在目标机执行：`codex resume <session_id>`

## Features (MVP)

- Sessions：扫描 `CODEX_HOME/sessions/**/rollout-*.jsonl`
- Export：
  - 生成可传输的 `bundle.zip`（`manifest.json` + `rollout.jsonl`）
  - 支持“合并为一个 zip”或“每会话单独 zip”，默认导出到 Downloads
- Import：
  - 导入前先解析/校验并预览最近消息，再决定是否写入
  - 支持多选 zip
  - 支持导入“合并导出包”（外层 zip 内含 `bundles/*.zip`，可选 `batch_manifest.json`）
  - 冲突策略（recommended）：本机已存在同 `session_id` 且指纹不同 -> “改 ID 导入”（保留两条分叉都可 resume）
- History / Vault：每次导入/导出/改ID/恢复都会存档 + 落库（SQLite）；支持收藏/标签；支持手动清理
- Restore：把任意历史版本恢复回 `CODEX_HOME`（会记录一次恢复动作）
- Change ID：改写 `session_meta.payload.id`（默认不改 `forked_from_id`）
- Utilities：从 md/txt/“带噪声文本”中自动提取会话 ID（UUID）并去重

相关格式说明：`docs/BUNDLE_FORMAT.md`

## Quick Start / 快速上手

1) 在机器 A：勾选会话 -> 导出 zip（默认在 Downloads）
2) 把 zip 传到机器 B（微信/网盘/邮件/AirDrop 均可）
3) 在机器 B：导入 -> 预览/确认 -> 写入
4) 在机器 B：`codex resume <session_id>`

## Download / 下载

- macOS：优先用 Homebrew（见下方 Install/Update），或从 GitHub Releases 下载 DMG
- Windows / Linux：从 GitHub Releases 下载安装包

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

- 推荐一条命令搞定（同时兼容“之前手动拖拽 DMG 安装 / Homebrew 没有接管 / 升级失败导致 cask 记录丢失”等情况）：

```bash
brew tap star-alp/tap-codexrelay && brew update && (brew upgrade --cask codexrelay || brew install --cask --force codexrelay)
```

- 如果你是通过 Homebrew 安装（`brew list --cask codexrelay` 能查到）：
  - `brew update && brew upgrade --cask codexrelay`
- 如果你看到 `Error: Cask 'codexrelay' is not installed.`，说明你之前不是通过 Homebrew 安装（例如手动 DMG 拖拽）：
  - 继续用 DMG 更新：下载最新 Release 的 `.dmg`，将 App 拖到 `/Applications` 覆盖即可（如遇 Gatekeeper，按上文 `xattr` 放行）。
  - 或让 Homebrew “接管”管理（会覆盖 `/Applications/CodexRelay.app`，不影响应用数据/存档库）：
    - `brew tap star-alp/tap-codexrelay && brew install --cask --force codexrelay`
- 应用内：`设置 -> 更新 -> 检查更新`（会提示最新版本并提供打开 Release 的按钮）。

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## Keywords

codex, codex-cli, session, resume, jsonl, backup, export, import, cross-device, cross-platform, tauri, rust, react, typescript, sqlite, homebrew, cask, macos, windows, linux
