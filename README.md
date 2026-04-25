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
- `sqlx-cli`

## Run Locally
1. 创建数据库和用户：
   ```sql
   CREATE USER ssai_app WITH PASSWORD 'CHANGEME';
   CREATE DATABASE ssai OWNER ssai_app;
   ```
2. 复制 [server/.env.example](/D:/sanshengITX/KSJKSJ2.0/ssai-server/server/.env.example:1) 为 `server/.env`，填入本地 PostgreSQL、快手开放平台和 bootstrap admin 配置。
3. 执行 migration：
   ```bash
   cd server
   sqlx migrate run
   ```
4. 启动服务并检查健康接口：
   ```bash
   cargo run
   curl http://127.0.0.1:8080/health
   ```

## OAuth Setup
- 在快手开放平台配置回调地址为 `KS_REDIRECT_URI`，默认值是 `https://kuaishou.ssaitool.cn/api/kuaishou/oauth/callback`。
- 设置 `BOOTSTRAP_ADMIN_EMAIL` 和 `BOOTSTRAP_ADMIN_PASSWORD`，服务启动时会把这个单租户管理员 UPSERT 到 `users` 表。
- 设置 `TOKEN_ENC_KEY` 为 base64 编码后的 32 字节密钥；服务端只会把 token 以 AES-256-GCM 密文形式写入 `ks_accounts.access_token_enc` 和 `ks_accounts.refresh_token_enc`。
- 访问 `GET /api/kuaishou/oauth/authorize` 获取授权 URL，完成扫码后由 `GET /api/kuaishou/oauth/callback` 校验 `state`、换 token 并落库。
