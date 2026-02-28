# CodexRelay Architecture（更新：2026-02-27）

目标：让用户在 macOS / Windows / Linux 之间，通过“导出包 -> 传输（微信等）-> 导入包”的方式，原生 `codex resume <session_id>` 继续同一个对话；同时每次传递都会在本机留档（vault + SQLite），避免分叉/覆盖导致丢失。

## Data Sources（来自哪里）

- Codex session rollout（必需）：
  - `CODEX_HOME/sessions/YYYY/MM/DD/rollout-...-<SESSION_ID>.jsonl`
- Shell snapshot（可选）：
  - `CODEX_HOME/shell_snapshots/<SESSION_ID>.sh`
  - 注意：可能包含环境变量/路径信息，默认不建议自动打包。

## Storage Layout（落盘结构）

- `CODEX_HOME`
  - 优先使用环境变量 `CODEX_HOME`
  - 否则默认探测：
    - macOS/Linux: `~/.codex`
    - Windows: `%USERPROFILE%\\.codex`
- `AppData`（Tauri `app_data_dir`）
  - 默认使用系统分配的应用数据目录
  - 支持 `CODEXRELAY_APP_DATA_DIR` 覆盖（用于测试/便携模式）
- SQLite
  - `AppData/codexrelay.sqlite3`
- Vault（永久归档）
  - `AppData/vault/<transfer_id>/...`
  - 导出/改ID：会生成一个“可传输 zip”（文件名包含 op/时间/session_id/name）
  - 导入/恢复：会把用户选择的 zip 固定存为 `bundle.zip`，并同时解包出 `manifest.json` / `rollout.jsonl`

## Core Concepts（核心概念）

- Bundle：一次“导出/导入”产生的可传输文件（zip）。
- Vault：CodexRelay 自己的永久归档区，保存每一次导入/导出的原始版本（不自动删除）。
- History：SQLite 记录每次导入/导出/恢复/改 ID 的操作元信息（强制命名）。

## Conflict Strategy（MVP）

同一个 `session_id` 在两台机器同时续写会产生分叉，JSONL 很难自动无损合并。

MVP 策略：
- 导入时如果发现本机已存在同 `session_id` 且内容不同：
  - 先强制备份本机当前版本到 vault
  - 再让用户选择：
    - 覆盖本机（继续使用导入版本）
    - “导入为新会话”（自动更换 session id，保留两条分叉都可 resume）

## Change Session ID（更换会话ID）

“更换 session id”应当：
- 重写 `rollout-*.jsonl` 中 `type=="session_meta"` 且 `payload.id==old_id` 的 `payload.id`
- 生成一个新的 rollout 文件（文件名包含 new_id），写入到 `CODEX_HOME/sessions/YYYY/MM/DD/`（不修改原文件）
- 默认不处理 `shell_snapshots/<id>.sh`（export 时可选打包；import/restore 默认不会写回 CODEX_HOME）

## Security / Guardrails（安全护栏）

- 不打包 `CODEX_HOME/auth.json` / `config.toml` 等敏感信息（bundle 只包含会话相关文件）。
- 导入 zip 时做 sha256/size 校验，并限制最大解包大小，避免损坏包/恶意 zip 造成磁盘耗尽。
- 预览能力限制为白名单根目录（`CODEX_HOME` / AppData / Vault），避免通过 IPC 任意读文件。
