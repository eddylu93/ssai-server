# ssai-server

SSAI 快手监控 — 云端中转服务（Rust + Axum）。

## 角色
- OAuth 回调接收 + Token 换取/刷新（密钥不下发客户端）
- 定时拉取快手磁力金牛/磁力引擎报表
- PostgreSQL 权威数据存储
- 向客户端推送增量数据（REST + WebSocket）

## 文档
完整规格见 `docs/CODEX_SPEC.md`（§35-§41 服务端章节）。

## Prerequisites
- Rust 1.75+
- PostgreSQL 16
- `sqlx-cli`（用于执行 migration）

## Run Locally
1. 创建本地数据库和用户：
   ```sql
   CREATE USER ssai_app WITH PASSWORD 'REPLACE_STRONG';
   CREATE DATABASE ssai OWNER ssai_app;
   ```
2. 在 `server/` 下执行 migration：
   ```bash
   cargo install sqlx-cli --no-default-features --features native-tls,postgres
   sqlx migrate run
   ```
3. 复制 `server/.env.example` 为 `server/.env`，填好 `DATABASE_URL` 后启动：
   ```bash
   cargo run
   ```
4. 检查健康接口：
   ```bash
   curl http://127.0.0.1:8080/health
   ```
