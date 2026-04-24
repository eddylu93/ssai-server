# ssai-server

SSAI 快手监控 — 云端中转服务（Rust + Axum）。

## 角色
- OAuth 回调接收 + Token 换取/刷新（密钥不下发客户端）
- 定时拉取快手磁力金牛/磁力引擎报表
- PostgreSQL 权威数据存储
- 向客户端推送增量数据（REST + WebSocket）

## 文档
完整规格见 `docs/CODEX_SPEC.md`（§35-§41 服务端章节）。

## 快速开始
待补充。
