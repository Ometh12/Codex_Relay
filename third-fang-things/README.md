# third-fang-things

这个目录用于放置“第三方开源项目/参考实现”，便于阅读与借鉴（不代表本项目直接依赖或复用其源码）。

## Included

- `ccswitch/`
  - Upstream: https://github.com/ksred/ccswitch
  - License: MIT
  - Why: 作为优秀的“会话/工作区管理工具”参考，重点学习其：
    - 统一的数据目录与可配置策略（`~/.ccswitch/...` + `config.yaml`）
    - 安全的清理/删除交互（先列出、再确认）
    - 可维护的错误类型/提示（errors + hint）
    - 发行/发布自动化（GoReleaser、checksums、install.sh）
    - 测试分层（unit / integration / docker）

