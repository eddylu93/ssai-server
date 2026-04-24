# CODEX_SPEC — 开发实现规格书

> 本文档为 AI Coding Agent（Codex / Cursor / Copilot Workspace）优化的技术规格书。
所有类型定义、数据结构、IPC 接口契约均可直接复制到代码中使用。
源 PRD：SSAI-快手监控
> 

---

## 0. 项目元信息

```json
{
  "name": "ssai-ksdesktop",
  "displayName": "SSAI-快手监控",
  "version": "1.0.0",
  "description": "纯数据监控 + 智能预警桌面工具，聚合快手磁力金牛/磁力引擎广告账户数据，只读不操作",
  "framework": "Tauri 2.x",
  "frontend": "React 18 + TypeScript 5 + Vite",
  "backend": "Rust (stable >= 1.75)",
  "database": "SQLite (WAL mode, rusqlite)",
  "stateManagement": "Zustand",
  "uiLibrary": "Ant Design 5.x",
  "charts": "ECharts 5.x (echarts-for-react)",
  "table": "@tanstack/react-table v8",
  "platforms": ["windows-x86_64", "macos-aarch64", "macos-x86_64"],
  "license": "PRIVATE",
  "repository": "https://github.com/eddylu93/ssai-ksdesktop"
}
```

---

## 1. 产品边界（硬约束）

```yaml
RULES:
  - NEVER call any Kuaishou write/mutation API (create, update, delete ads)
  - ALL ad operations are done by human in Kuaishou's official dashboard
  - This tool ONLY: pulls data → detects anomalies → alerts via IM → gives suggestions
  - Hybrid architecture: Tauri desktop client + cloud relay server (see §35)
  - Authoritative data: server PostgreSQL; client SQLite = 7-day cache only
  - App Secret & OAuth tokens ONLY stored on server (encrypted), NEVER in client binary
  - Primary target: 磁力金牛 (ESP); 磁力引擎 (DSP) compatible via shared MAPI
  - Single-tenant (small team) auth via server-issued JWT bearer tokens
```

---

## 2. 技术栈依赖清单

```json
// package.json (key dependencies)
{
  "dependencies": {
    "react": "^18.3.0",
    "react-dom": "^18.3.0",
    "react-router-dom": "^6.20.0",
    "zustand": "^4.5.0",
    "antd": "^5.12.0",
    "echarts": "^5.5.0",
    "echarts-for-react": "^3.0.2",
    "@tanstack/react-table": "^8.11.0",
    "@tauri-apps/api": "^2.0.0",
    "dayjs": "^1.11.10"
  },
  "devDependencies": {
    "typescript": "^5.3.0",
    "vite": "^5.0.0",
    "@vitejs/plugin-react": "^4.2.0",
    "vitest": "^1.1.0",
    "eslint": "^8.56.0",
    "prettier": "^3.2.0",
    "husky": "^9.0.0",
    "lint-staged": "^15.2.0"
  }
}
```

```toml
# Cargo.toml (key dependencies)
[dependencies]
tauri = { version = "2", features = ["updater"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
rusqlite = { version = "0.31", features = ["bundled"] }
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
keyring = "2"
tokio-cron-scheduler = "0.10"
tracing = "0.1"
tracing-subscriber = "0.3"
tracing-appender = "0.2"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4"] }
```

---

## 3. 数据模型（TypeScript 接口 — 前端）

```tsx
// === src/types/account.ts ===
export interface Account {
  id: number;
  accountId: string;         // 快手账户 ID
  name: string;
  groupTag: string | null;   // 分组标签: "主账号" | "代投-品牌A" | ...
  authorizedAt: string;      // ISO-8601
  tokenStatus: TokenStatus;
  todayCost: number;
  balance: number;
  status: AccountStatus;
  createdAt: string;
}

export type AccountStatus = 'normal' | 'abnormal' | 'paused';
export type TokenStatus = 'valid' | 'expiring' | 'expired' | 'refreshing';

// === src/types/metric.ts ===
export interface MetricSnapshot {
  id: number;
  level: MetricLevel;
  refId: string;             // accountId | campaignId | unitId | creativeId
  snapshotAt: string;        // ISO-8601
  cost: number;              // 消耗(元)
  impressions: number;       // 曝光量
  clicks: number;            // 点击量
  conversions: number;       // 转化数
  roi: number;               // ROI = 成交额 / 消耗
  ctr: number;               // 点击率 = clicks / impressions
  cvr: number;               // 转化率 = conversions / clicks
  cpa: number;               // CPA = cost / conversions
}

export type MetricLevel = 'account' | 'campaign' | 'unit' | 'creative';

// === src/types/dashboard.ts ===
export interface DashboardMetricCard {
  label: string;             // "今日消耗(元)" | "曝光量" | "点击量" | "转化率(%)" | "ROI"
  value: number;
  previousValue: number;     // 昨日同期值
  changePercent: number;     // (value - previousValue) / previousValue * 100
  changeDirection: 'up' | 'down' | 'flat';
  sparklineData: number[];   // 最近24个数据点(小时级)
  color: string;             // Design Token color variable
}

export interface ChannelDistribution {
  channel: string;           // "快手主站" | "快手极速版" | "快手联盟" | "快手搜索" | "其他"
  cost: number;
  percent: number;
  color: string;
}

// === src/types/campaign.ts ===
export interface Campaign {
  campaignId: string;
  accountId: string;
  name: string;
  dailyBudget: number;
  status: CampaignStatus;
  bidType: string;
  cost: number;
  impressions: number;
  clicks: number;
  conversions: number;
  roi: number;
  ctr: number;
  cvr: number;
  cpa: number;
  healthScore: number;       // 0-100, AI 计划健康度
  updatedAt: string;
}

export type CampaignStatus = 'active' | 'paused' | 'budget_depleted' | 'review_rejected' | 'expired';

// === src/types/alert.ts ===
export interface AlertRule {
  id: number;
  type: AlertRuleType;
  scopeLevel: 'global' | 'account' | 'campaign';
  scopeRef: string | null;   // null = global
  threshold: AlertThreshold;
  enabled: boolean;
  quietHours: string | null; // "00:00-08:00" or null
  minIntervalSec: number;    // 推送最小间隔(秒), default 1800
}

export type AlertRuleType =
  | 'budget_overrun'         // 预算超支
  | 'ctr_drop'               // 点击率异常下降
  | 'roi_drop'               // ROI 下降
  | 'balance_low'            // 账户余额不足
  | 'cpa_spike'              // 转化成本飙升
  | 'creative_rejected'      // 创意审核未通过
  | 'token_expired';         // 授权失效

export interface AlertThreshold {
  // 不同 type 使用不同字段
  percentDrop?: number;      // ctr_drop: 30 => 下降30%触发
  absoluteValue?: number;    // roi_drop: 1.0 => ROI<1.0触发; balance_low: 500 => 余额<500触发
  budgetPercent?: number;    // budget_overrun: 100 => 消耗>=100%日预算
  minCost?: number;          // 最低消耗阈值，过滤低消耗噪音
  lookbackDays?: number;     // 对比基准天数, default 7
}

export interface AlertEvent {
  id: number;
  ruleId: number;
  ruleType: AlertRuleType;
  triggeredAt: string;       // ISO-8601
  level: AlertLevel;
  subject: string;           // 告警标题
  detail: AlertDetail;
  handled: boolean;
  handledAt: string | null;
}

export type AlertLevel = 'critical' | 'warning' | 'info';

export interface AlertDetail {
  accountId: string;
  accountName: string;
  campaignId?: string;
  campaignName?: string;
  currentValue: number;
  thresholdValue: number;
  suggestion?: string;       // AI 生成的建议文字
}

// === src/types/push.ts ===
export interface PushChannel {
  id: number;
  provider: IMProvider;
  name: string;              // 用户自定义名称 "投放群" | "管理层群"
  webhookConfigured: boolean;
  signSecretConfigured: boolean;
  enabled: boolean;
}

export type IMProvider = 'wecom' | 'dingtalk' | 'feishu' | 'slack';

export interface PushSettings {
  frequency: PushFrequency;
  alertTypes: AlertRuleType[];        // 勾选的告警类型
  receivers: PushReceiver[];
}

export type PushFrequency = 'realtime' | 'hourly' | 'daily';

export interface PushReceiver {
  name: string;
  role: string;              // "运营经理" | "投放专员" | ...
  identifier: string;        // userId(飞书/企微) | 手机号(钉钉)
}

// === src/types/ai.ts ===
export interface AISuggestion {
  id: string;
  type: SuggestionType;
  campaignId?: string;
  campaignName?: string;
  accountId: string;
  title: string;
  description: string;       // 建议详细内容
  reasons: string[];         // 归因列表 (最多3条)
  actionUrl?: string;        // 跳转快手后台的 URL
  confidence: number;        // 0-1 置信度
  createdAt: string;
}

export type SuggestionType =
  | 'anomaly_diagnosis'      // 异常归因
  | 'budget_suggestion'      // 预算建议
  | 'bid_suggestion'         // 出价建议
  | 'creative_suggestion'    // 素材建议
  | 'health_warning';        // 健康度预警
```

---

## 4. 数据模型（Rust 结构体 — 后端）

```rust
// === src-tauri/src/models/account.rs ===
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: i64,
    pub account_id: String,
    pub name: String,
    pub group_tag: Option<String>,
    pub authorized_at: Option<DateTime<Utc>>,
    pub token_ref: Option<String>,     // Keychain alias
    pub status: AccountStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountStatus {
    Normal,
    Abnormal,
    Paused,
}

// === src-tauri/src/models/metric.rs ===
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSnapshot {
    pub id: i64,
    pub level: String,          // "account" | "campaign" | "unit" | "creative"
    pub ref_id: String,
    pub snapshot_at: DateTime<Utc>,
    pub cost: f64,
    pub impressions: i64,
    pub clicks: i64,
    pub conversions: i64,
    pub roi: f64,
}

// === src-tauri/src/models/alert.rs ===
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    pub id: i64,
    pub rule_type: String,
    pub scope_level: String,
    pub scope_ref: Option<String>,
    pub threshold_json: String,      // serde_json::Value
    pub enabled: bool,
    pub quiet_hours: Option<String>,
    pub min_interval_sec: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertEvent {
    pub id: i64,
    pub rule_id: i64,
    pub triggered_at: DateTime<Utc>,
    pub level: String,               // "critical" | "warning" | "info"
    pub subject: String,
    pub detail_json: String,
    pub handled: bool,
}

// === src-tauri/src/models/push.rs ===
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushChannel {
    pub id: i64,
    pub provider: String,            // "wecom" | "dingtalk" | "feishu" | "slack"
    pub name: String,
    pub webhook_ref: Option<String>, // Keychain alias for webhook URL
    pub sign_secret_ref: Option<String>,
    pub enabled: bool,
}
```

---

## 5. SQLite 建表语句（完整版）

```sql
-- === 数据库初始化 + 迁移 ===
-- 文件: src-tauri/src/db/schema.rs
-- 执行时机: 应用首次启动或版本升级时

PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

-- v1.0.0 基础表 --

CREATE TABLE IF NOT EXISTS accounts (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id      TEXT UNIQUE NOT NULL,
    name            TEXT NOT NULL,
    group_tag       TEXT,
    authorized_at   TEXT,          -- ISO-8601
    token_ref       TEXT,          -- Keychain alias: "ks_token_{account_id}"
    status          TEXT NOT NULL DEFAULT 'normal',  -- normal | abnormal | paused
    balance         REAL DEFAULT 0,
    daily_budget    REAL DEFAULT 0,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS campaigns (
    campaign_id     TEXT PRIMARY KEY,
    account_id      TEXT NOT NULL REFERENCES accounts(account_id),
    name            TEXT NOT NULL,
    daily_budget    REAL,
    bid_type        TEXT,          -- "OCPM" | "CPC" | "CPM" | ...
    status          TEXT,          -- active | paused | budget_depleted | review_rejected | expired
    updated_at      TEXT
);
CREATE INDEX IF NOT EXISTS idx_campaigns_account ON campaigns(account_id);

CREATE TABLE IF NOT EXISTS units (
    unit_id         TEXT PRIMARY KEY,
    campaign_id     TEXT NOT NULL REFERENCES campaigns(campaign_id),
    account_id      TEXT NOT NULL,
    name            TEXT NOT NULL,
    status          TEXT,
    updated_at      TEXT
);
CREATE INDEX IF NOT EXISTS idx_units_campaign ON units(campaign_id);

CREATE TABLE IF NOT EXISTS creatives (
    creative_id     TEXT PRIMARY KEY,
    unit_id         TEXT NOT NULL REFERENCES units(unit_id),
    account_id      TEXT NOT NULL,
    name            TEXT,
    review_status   TEXT,          -- pending | approved | rejected
    reject_reason   TEXT,
    cover_url       TEXT,
    video_url       TEXT,
    updated_at      TEXT
);
CREATE INDEX IF NOT EXISTS idx_creatives_unit ON creatives(unit_id);

CREATE TABLE IF NOT EXISTS metrics_snapshots (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    level           TEXT NOT NULL,  -- account | campaign | unit | creative
    ref_id          TEXT NOT NULL,
    snapshot_at     TEXT NOT NULL,  -- ISO-8601
    cost            REAL DEFAULT 0,
    impressions     INTEGER DEFAULT 0,
    clicks          INTEGER DEFAULT 0,
    conversions     INTEGER DEFAULT 0,
    roi             REAL DEFAULT 0,
    ctr             REAL DEFAULT 0,
    cvr             REAL DEFAULT 0,
    cpa             REAL DEFAULT 0,
    revenue         REAL DEFAULT 0  -- 成交金额
);
CREATE INDEX IF NOT EXISTS idx_metrics_ref ON metrics_snapshots(level, ref_id, snapshot_at);
CREATE INDEX IF NOT EXISTS idx_metrics_time ON metrics_snapshots(snapshot_at);

CREATE TABLE IF NOT EXISTS alert_rules (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    type            TEXT NOT NULL,
    scope_level     TEXT NOT NULL DEFAULT 'global',
    scope_ref       TEXT,
    threshold_json  TEXT NOT NULL,
    enabled         INTEGER NOT NULL DEFAULT 1,
    quiet_hours     TEXT,
    min_interval_sec INTEGER NOT NULL DEFAULT 1800,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS alert_events (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    rule_id         INTEGER NOT NULL REFERENCES alert_rules(id),
    triggered_at    TEXT NOT NULL,
    level           TEXT NOT NULL,    -- critical | warning | info
    subject         TEXT NOT NULL,
    detail_json     TEXT,
    handled         INTEGER NOT NULL DEFAULT 0,
    handled_at      TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_alerts_time ON alert_events(triggered_at);
CREATE INDEX IF NOT EXISTS idx_alerts_level ON alert_events(level, handled);

CREATE TABLE IF NOT EXISTS push_channels (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    provider        TEXT NOT NULL,    -- wecom | dingtalk | feishu | slack
    name            TEXT NOT NULL,
    webhook_ref     TEXT,             -- Keychain alias
    sign_secret_ref TEXT,
    enabled         INTEGER NOT NULL DEFAULT 1,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS push_logs (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    channel_id      INTEGER NOT NULL REFERENCES push_channels(id),
    alert_event_id  INTEGER REFERENCES alert_events(id),
    payload_json    TEXT,
    status          TEXT NOT NULL,    -- sent | failed | pending
    error_message   TEXT,
    sent_at         TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_push_logs_time ON push_logs(sent_at);

CREATE TABLE IF NOT EXISTS ai_suggestions (
    id              TEXT PRIMARY KEY, -- UUID v4
    type            TEXT NOT NULL,
    account_id      TEXT NOT NULL,
    campaign_id     TEXT,
    title           TEXT NOT NULL,
    description     TEXT NOT NULL,
    reasons_json    TEXT,              -- JSON array of strings
    action_url      TEXT,
    confidence      REAL DEFAULT 0,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS app_settings (
    key             TEXT PRIMARY KEY,
    value_json      TEXT NOT NULL,
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

-- === 默认告警规则(种子数据) ===
INSERT OR IGNORE INTO alert_rules (id, type, scope_level, threshold_json, enabled, min_interval_sec) VALUES
(1, 'budget_overrun',    'global', '{"budgetPercent": 100}',                         1, 1800),
(2, 'ctr_drop',          'global', '{"percentDrop": 30, "lookbackDays": 7}',         1, 1800),
(3, 'roi_drop',          'global', '{"absoluteValue": 1.0, "minCost": 500}',         1, 1800),
(4, 'balance_low',       'global', '{"absoluteValue": 500}',                         1, 3600),
(5, 'cpa_spike',         'global', '{"percentDrop": 45, "lookbackDays": 7}',         1, 1800),
(6, 'creative_rejected', 'global', '{}',                                              1, 0),
(7, 'token_expired',     'global', '{}',                                              1, 0);
```

---

## 6. IPC 命令契约（Tauri Commands）

```tsx
// === src/services/ipc.ts ===
// 所有 Tauri IPC invoke 调用的类型定义
// Rust 端每个 command 对应 src-tauri/src/commands/*.rs 中的同名函数

import { invoke } from '@tauri-apps/api/core';

// --- 账户模块 ---
export const accountCommands = {
  /** 获取 OAuth 授权 URL，前端打开浏览器跳转 */
  getOAuthUrl: () =>
    invoke<{ url: string; state: string }>('get_oauth_url'),

  /** OAuth 回调处理，传入 code 换取 token */
  handleOAuthCallback: (code: string, state: string) =>
    invoke<Account>('handle_oauth_callback', { code, state }),

  /** 获取所有已绑定账户 */
  listAccounts: () =>
    invoke<Account[]>('list_accounts'),

  /** 手动刷新某账户 token */
  refreshToken: (accountId: string) =>
    invoke<{ success: boolean; message: string }>('refresh_token', { accountId }),

  /** 解绑账户 */
  unbindAccount: (accountId: string) =>
    invoke<void>('unbind_account', { accountId }),

  /** 更新账户分组标签 */
  updateAccountTag: (accountId: string, groupTag: string | null) =>
    invoke<void>('update_account_tag', { accountId, groupTag }),
};

// --- 数据查询模块 ---
export const metricsCommands = {
  /** Dashboard 核心指标卡数据 */
  getDashboardCards: () =>
    invoke<DashboardMetricCard[]>('get_dashboard_cards'),

  /** 投放趋势图数据 */
  getTrendData: (range: 'hour' | 'day' | '7d' | '30d') =>
    invoke<{ timestamps: string[]; costSeries: number[]; ctrSeries: number[] }>(
      'get_trend_data', { range }
    ),

  /** 渠道消耗分布 */
  getChannelDistribution: () =>
    invoke<ChannelDistribution[]>('get_channel_distribution'),

  /** 账户状态列表(概览页右侧) */
  getAccountStatusList: () =>
    invoke<{ accountId: string; name: string; cost: number; status: AccountStatus }[]>(
      'get_account_status_list'
    ),

  /** 投放效果散点图数据 */
  getEffectScatter: (type: 'cost' | 'conversion' | 'roi') =>
    invoke<{ campaignId: string; name: string; x: number; y: number; size: number }[]>(
      'get_effect_scatter', { type }
    ),

  /** 计划列表(投放监控页) */
  getCampaigns: (filters: {
    accountId?: string;
    status?: CampaignStatus;
    roiMin?: number;
    roiMax?: number;
    costMin?: number;
  }) => invoke<Campaign[]>('get_campaigns', { filters }),

  /** 单元列表(展开计划时延迟加载) */
  getUnits: (campaignId: string) =>
    invoke<Unit[]>('get_units', { campaignId }),

  /** 创意列表 */
  getCreatives: (unitId: string) =>
    invoke<Creative[]>('get_creatives', { unitId }),
};

// --- 告警模块 ---
export const alertCommands = {
  /** 获取告警规则列表 */
  listAlertRules: () =>
    invoke<AlertRule[]>('list_alert_rules'),

  /** 创建/更新告警规则 */
  upsertAlertRule: (rule: Omit<AlertRule, 'id'> & { id?: number }) =>
    invoke<AlertRule>('upsert_alert_rule', { rule }),

  /** 删除告警规则 */
  deleteAlertRule: (ruleId: number) =>
    invoke<void>('delete_alert_rule', { ruleId }),

  /** 获取告警事件列表(支持分页+筛选) */
  listAlertEvents: (params: {
    level?: AlertLevel;
    handled?: boolean;
    startDate?: string;
    endDate?: string;
    offset: number;
    limit: number;
  }) => invoke<{ events: AlertEvent[]; total: number }>('list_alert_events', { params }),

  /** 标记告警为已处理 */
  handleAlert: (eventId: number) =>
    invoke<void>('handle_alert', { eventId }),

  /** 获取概览页实时告警流(最近N条) */
  getRecentAlerts: (limit: number) =>
    invoke<AlertEvent[]>('get_recent_alerts', { limit }),
};

// --- 推送模块 ---
export const pushCommands = {
  /** 获取已配置的推送渠道 */
  listPushChannels: () =>
    invoke<PushChannel[]>('list_push_channels'),

  /** 添加/更新推送渠道 */
  upsertPushChannel: (channel: {
    id?: number;
    provider: IMProvider;
    name: string;
    webhookUrl: string;        // 明文传入，Rust 端存入 Keychain
    signSecret?: string;
  }) => invoke<PushChannel>('upsert_push_channel', { channel }),

  /** 删除推送渠道 */
  deletePushChannel: (channelId: number) =>
    invoke<void>('delete_push_channel', { channelId }),

  /** 测试推送(发送一条样例消息) */
  testPush: (channelId: number) =>
    invoke<{ success: boolean; message: string }>('test_push', { channelId }),

  /** 获取推送记录 */
  listPushLogs: (params: {
    channelId?: number;
    status?: 'sent' | 'failed';
    offset: number;
    limit: number;
  }) => invoke<{ logs: PushLog[]; total: number }>('list_push_logs', { params }),

  /** 保存推送设置(频率、告警类型、接收人) */
  savePushSettings: (settings: PushSettings) =>
    invoke<void>('save_push_settings', { settings }),

  /** 获取推送设置 */
  getPushSettings: () =>
    invoke<PushSettings>('get_push_settings'),
};

// --- AI 模块 ---
export const aiCommands = {
  /** 获取 AI 建议列表 */
  listSuggestions: (params: {
    type?: SuggestionType;
    accountId?: string;
    offset: number;
    limit: number;
  }) => invoke<{ suggestions: AISuggestion[]; total: number }>(
    'list_suggestions', { params }
  ),

  /** 获取计划健康度评分 */
  getHealthScores: (accountId?: string) =>
    invoke<{ campaignId: string; name: string; score: number; factors: string[] }[]>(
      'get_health_scores', { accountId }
    ),

  /** 手动触发异常归因分析 */
  diagnoseAnomaly: (alertEventId: number) =>
    invoke<AISuggestion>('diagnose_anomaly', { alertEventId }),
};

// --- 系统设置模块 ---
export const settingsCommands = {
  /** 获取所有设置 */
  getSettings: () =>
    invoke<AppSettings>('get_settings'),

  /** 更新设置 */
  updateSettings: (updates: Partial<AppSettings>) =>
    invoke<AppSettings>('update_settings', { updates }),

  /** 导出诊断日志(脱敏) */
  exportDiagnosticLogs: (days: number) =>
    invoke<{ filePath: string }>('export_diagnostic_logs', { days }),

  /** 手动触发数据库备份 */
  triggerBackup: () =>
    invoke<{ filePath: string }>('trigger_backup'),

  /** 保存 LLM API Key(存入 Keychain) */
  saveLlmConfig: (config: { apiKey: string; baseUrl: string; model: string }) =>
    invoke<void>('save_llm_config', { config }),
};

// --- AppSettings 类型 ---
export interface AppSettings {
  theme: 'dark' | 'light' | 'system';
  dataDir: string;
  backupDir: string;
  backupRetentionDays: number;       // default 30
  refreshIntervalForeground: number; // default 30 (seconds)
  refreshIntervalBackground: number; // default 300 (seconds)
  language: 'zh-CN';                 // v1 仅中文
}
```

---

## 7. Tauri 事件通道（后端 → 前端）

```tsx
// === src/hooks/useTauriEvent.ts ===
// 后端通过 Tauri Event 向前端推送实时数据
// 前端在对应 Store 中监听这些事件

export const TAURI_EVENTS = {
  /** 数据拉取完成，payload: { accountId: string; timestamp: string } */
  DATA_UPDATED: 'data_updated',

  /** 新告警产生，payload: AlertEvent */
  NEW_ALERT: 'new_alert',

  /** Token 状态变化，payload: { accountId: string; status: TokenStatus } */
  TOKEN_STATUS_CHANGED: 'token_status_changed',

  /** 推送结果回调，payload: { channelId: number; success: boolean; error?: string } */
  PUSH_RESULT: 'push_result',

  /** 调度器状态，payload: { running: boolean; nextRunAt: string } */
  SCHEDULER_STATUS: 'scheduler_status',

  /** AI 建议生成完成，payload: AISuggestion */
  AI_SUGGESTION_READY: 'ai_suggestion_ready',
} as const;

// Rust 端发送事件示例:
// app_handle.emit("new_alert", &alert_event)?;
```

---

## 8. 快手 API 适配层（字段映射）

<aside>
📌

本项目走磁力金牛 ESP 流程，不同报表端点字段差异较大。
映射按「报表类型」分组：通用（unit_report / ecpm_report）+ ESP 专属（商品 / 直播 / 短视频 ROI / 粉丝 / 订单）。
官方字段口径以开放平台「接口文档/字段说明」为准，本文件仅定义 Raw → 内部统一模型的映射。

</aside>

```json
// === public/field-mapping.json ===
// 按「报表类型」分组；FieldMapper 根据 report_type 选择对应 section
// 当快手 API 字段调整时只改本文件
{
  "$schema_version": "2.0",
  "$note": "2.0 = 磁力金牛 ESP 适配版；Raw→内部字段映射",

  "_shared": {
    "advertiser_id":    "accountId",
    "advertiser_name":  "name",
    "stat_date":        "statDate",
    "stat_hour":        "statHour",
    "stat_minute":      "statMinute",
    "request_id":       "requestId"
  },

  "unit_report": {
    "_endpoint": "/rest/openapi/v1/report/unit_report",
    "charge":                "cost",
    "photo_show":            "impressions",
    "photo_click":           "clicks",
    "photo_click_ratio":     "ctr",
    "aclick":                "conversions",
    "conversion_rate":       "cvr",
    "conversion_price":      "cpa",
    "impression_1k_cost":    "cpm",
    "click_cost":            "cpc"
  },

  "ecpm_report": {
    "_endpoint": "/rest/openapi/gw/dsp/v1/report/ecpm_report",
    "ecpm":                  "ecpm",
    "charge":                "cost",
    "show_cnt":              "impressions",
    "click_cnt":             "clicks",
    "conversion_cnt":        "conversions"
  },

  "esp_goods_report": {
    "_endpoint": "/rest/openapi/v2/esp/goods/report",
    "_note": "商品维度 —— 磁力金牛核心看板",
    "item_id":               "itemId",
    "item_title":            "itemTitle",
    "item_cover":            "itemCover",
    "charge":                "cost",
    "pay_amount":            "revenue",
    "pay_order_cnt":         "paidOrders",
    "pay_order_user_cnt":    "paidBuyers",
    "roi":                   "roi",
    "cost_per_order":        "costPerOrder",
    "aov":                   "aov",
    "click_cnt":             "clicks",
    "show_cnt":              "impressions",
    "ctr":                   "ctr",
    "order_ctr":             "orderCtr"
  },

  "esp_live_report": {
    "_endpoint": "/rest/openapi/v2/esp/live/report",
    "_note": "直播间维度",
    "live_stream_id":        "liveStreamId",
    "live_title":            "liveTitle",
    "start_time":            "liveStartAt",
    "end_time":              "liveEndAt",
    "charge":                "cost",
    "watch_cnt":             "viewers",
    "watch_time":            "watchSeconds",
    "new_follow_cnt":        "newFollows",
    "pay_amount":            "revenue",
    "pay_order_cnt":         "paidOrders",
    "roi":                   "roi",
    "in_live_pay_amount":    "liveRoomRevenue"
  },

  "esp_photo_roi_report": {
    "_endpoint": "/rest/openapi/v2/esp/photo/roi_report",
    "_note": "短视频 ROI —— 单条短视频维度",
    "photo_id":              "photoId",
    "photo_title":           "photoTitle",
    "photo_cover":           "photoCover",
    "publish_time":          "publishedAt",
    "charge":                "cost",
    "show_cnt":              "impressions",
    "click_cnt":             "clicks",
    "like_cnt":              "likes",
    "comment_cnt":           "comments",
    "share_cnt":             "shares",
    "pay_amount":            "revenue",
    "pay_order_cnt":         "paidOrders",
    "roi":                   "roi"
  },

  "esp_fans_report": {
    "_endpoint": "/rest/openapi/v2/esp/fans/report",
    "_note": "粉丝维度",
    "new_fans_cnt":          "newFans",
    "lost_fans_cnt":         "lostFans",
    "net_fans_cnt":          "netFans",
    "total_fans_cnt":        "totalFans",
    "fans_pay_amount":       "fansRevenue",
    "fans_pay_order_cnt":    "fansOrders"
  },

  "esp_shop_order": {
    "_endpoint": "/rest/openapi/v2/esp/shop/order_list",
    "_note": "小店订单明细（按订单粒度）",
    "order_id":              "orderId",
    "item_id":               "itemId",
    "buyer_id":              "buyerId",
    "pay_amount":            "orderAmount",
    "pay_time":              "paidAt",
    "order_status":          "orderStatus",
    "refund_amount":         "refundAmount",
    "source_photo_id":       "sourcePhotoId",
    "source_live_stream_id": "sourceLiveStreamId"
  },

  "advertiser_info": {
    "_endpoint": "/v1/advertiser/list + /v1/advertiser/fund/get",
    "advertiser_id":         "accountId",
    "advertiser_name":       "name",
    "recharge_balance":      "balance",
    "contract_rebate":       "contractRebate",
    "direct_rebate":         "directRebate",
    "daily_budget":          "dailyBudget",
    "status":                "rawStatus"
  },

  "campaign_info": {
    "_endpoint": "/rest/openapi/v1/campaign/list",
    "campaign_id":           "campaignId",
    "campaign_name":         "name",
    "day_budget":            "dailyBudget",
    "put_status":            "status",
    "bid_type":              "bidType"
  },

  "status_mapping": {
    "account_status": {
      "1": "normal",
      "2": "paused",
      "3": "limited",
      "4": "abnormal"
    },
    "campaign_status": {
      "1": "active",
      "2": "paused",
      "3": "budget_depleted",
      "4": "review_rejected",
      "5": "expired"
    },
    "order_status": {
      "1": "pending_pay",
      "2": "paid",
      "3": "shipped",
      "4": "completed",
      "5": "refunded",
      "6": "closed"
    }
  }
}
```

```rust
// === src-tauri/src/services/field_mapper.rs (客户端) ===
// 服务端 server/src/services/mapper.rs 共享同一份 field-mapping.json

use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum ReportType {
    UnitReport,
    EcpmReport,
    EspGoodsReport,
    EspLiveReport,
    EspPhotoRoiReport,
    EspFansReport,
    EspShopOrder,
    AdvertiserInfo,
    CampaignInfo,
}

pub struct FieldMapper {
    pub mappings: HashMap<ReportType, HashMap<String, String>>,
    pub status_mapping: HashMap<String, HashMap<String, String>>,
    pub shared: HashMap<String, String>,
}

impl FieldMapper {
    /// 从 field-mapping.json 加载配置（客户端在打包资源内，服务端在 /opt/ssai/server/config 下）
    pub fn from_config(config_path: &str) -> Result<Self, anyhow::Error> { todo!() }

    /// 通用映射入口：由调用方指定 report_type
    pub fn map_metric(&self, report_type: ReportType, raw: &Value)
        -> Result<MetricSnapshot, anyhow::Error> { todo!() }

    /// 映射商品维度（/v2/esp/goods/report）
    pub fn map_goods_row(&self, raw: &Value) -> Result<GoodsMetric, anyhow::Error> { todo!() }

    /// 映射直播维度（/v2/esp/live/report）
    pub fn map_live_row(&self, raw: &Value) -> Result<LiveMetric, anyhow::Error> { todo!() }

    /// 映射短视频 ROI（/v2/esp/photo/roi_report）
    pub fn map_photo_roi_row(&self, raw: &Value) -> Result<PhotoRoiMetric, anyhow::Error> { todo!() }

    /// 映射订单明细（/v2/esp/shop/order_list）
    pub fn map_shop_order(&self, raw: &Value) -> Result<ShopOrder, anyhow::Error> { todo!() }

    /// 状态码映射：kind ∈ { account_status, campaign_status, order_status }
    pub fn map_status(&self, kind: &str, raw: &str) -> String { todo!() }
}
```

<aside>
⚠️

字段存在同名异义情况（如 charge / pay_amount 在通用报表与 ESP 报表中语义边界不同）。
FieldMapper 调用前必须强制传入 report_type，严禁「按字段名猜」。字段变更时先改本 JSON，再跑 `cargo test field_mapper` 的 golden 快照测试。

</aside>

---

## 9. 快手 API 调用规范（✅ 已对照官方文档校正）

<aside>
📌

以下内容已根据磁力引擎开放平台官方文档（2025-12 更新）校正。
官方文档入口：
• 磁力引擎 DSP: [https://developers.e.kuaishou.com/docs?docType=DSP](https://developers.e.kuaishou.com/docs?docType=DSP)
• 磁力金牛 ESP: [https://developers.e.kuaishou.com/docs?docType=ESP](https://developers.e.kuaishou.com/docs?docType=ESP)

</aside>

```yaml
# ============================================================
# OAuth 2.0 授权流程（磁力引擎 MAPI）
# 官方文档: MAPI注册授权流程说明
# ============================================================
auth:
  type: authorization_code
  protocol: OAuth 2.0

  # --- 步骤 1: 引导用户授权 ---
  # 拼接授权 URL 在浏览器中打开
  authorize_url_template: >
    https://developers.e.kuaishou.com/oauth/authorize?
    app_id={app_id}&
    scope={scope}&
    redirect_uri={redirect_uri}&
    state={state}&
    oauth_type=advertiser

  # oauth_type 可选值:
  #   advertiser — 广告主/服务商（本项目使用此值）
  #   agent      — 代理商
  #   ad_social  — 聚星

  # --- 步骤 2: 获取 Token ---
  # 用户授权后回调返回 auth_code，用 auth_code 换 token
  token_url: "https://ad.e.kuaishou.com/rest/openapi/oauth2/authorize/access_token"
  token_method: POST
  token_content_type: application/json
  token_request_body:
    app_id: "{app_id}"
    secret: "{app_secret}"
    auth_code: "{auth_code}"
  token_response_fields:
    - access_token       # 访问令牌
    - refresh_token      # 刷新令牌
    - expires_in         # access_token 剩余有效秒数
    - refresh_token_expires_in  # refresh_token 剩余有效秒数
    # ⚠️ 官方已于 2024 年下线 advertiser_ids / advertiser_id 字段，不再随 token 返回
    # 获取广告主列表请在 token 换取后另行调用: POST /rest/openapi/v1/advertiser/list

  # --- 步骤 3: 刷新 Token ---
  refresh_url: "https://ad.e.kuaishou.com/rest/openapi/oauth2/authorize/refresh_token"
  refresh_method: POST
  refresh_request_body:
    app_id: "{app_id}"
    secret: "{app_secret}"
    refresh_token: "{refresh_token}"

  # --- Token 有效期（磁力引擎 MAPI 官方确认）---
  auth_code_ttl: 10m          # auth_code 有效 10 分钟，仅可使用一次
  access_token_ttl: 24h       # access_token 有效 1 天
  refresh_token_ttl: 30d      # refresh_token 有效 30 天
  # ⚠️ 每次 refresh 会生成全新的 access_token + refresh_token
  # ⚠️ 旧 access_token 和 refresh_token 立即失效，务必保存新值

  # --- 本项目配置 ---
  redirect_uri: "https://kuaishou.ssaitool.cn/api/kuaishou/oauth/callback"
  # ⚠️ 必须是公网 HTTPS 回调（快手平台要求 + 混合架构要求）
  # 回调由云端中转服务器接收 auth_code 并完成 token 换取，详见 §35 时序图
  scopes:
    - report_service      # 报表（磁力金牛商品/直播/短视频ROI/粉丝报表均需此 scope）
    - account_service     # 账户信息 + 余额（recharge_balance, contract_rebate, direct_rebate）
    # 磁力金牛专属报表接口清单见 §36
    # ⚠️ 不申请 ad_query / ad_manage —— 本项目严格只读监控
  oauth_type: advertiser  # 磁力金牛走 advertiser 流程（广告主/服务商），非 agent/ad_social
  token_storage: OS_KEYCHAIN
  token_refresh_strategy: >
    access_token 过期前 1 小时主动刷新；
    如刷新失败，每 5 分钟重试，最多 3 次；
    refresh_token 过期前 3 天弹窗提醒用户重新授权。
  max_refresh_retries: 3

# ============================================================
# API 基础配置
# ============================================================
api:
  base_url: "https://ad.e.kuaishou.com"   # 官方 API 域名
  path_prefix: "/rest/openapi"             # 所有接口路径前缀
  # 完整接口 URL = base_url + path_prefix + 具体路径
  # 例: https://ad.e.kuaishou.com/rest/openapi/oauth2/authorize/access_token

  # 请求规范（官方要求）
  method: POST                             # 所有数据接口均为 POST
  content_type: application/json
  auth_header: "Access-Token"              # ⚠️ 不是 Authorization: Bearer
  # 示例: curl -H "Access-Token:d198a850da67..." -H "Content-Type:application/json"

  timeout_ms: 10000
  max_retries: 3
  retry_strategy: exponential_backoff      # min(2^n * 1s + random(0,500ms), 60s)

  # 通用响应结构
  response_structure:
    code: 0          # 0 = 成功，非0 = 失败
    message: "OK"
    data: {}         # 业务数据
    request_id: ""   # 请求唯一标识（用于排查问题）

  # 常见错误码
  error_codes:
    0:     "success"
    40001: "access_token expired → 触发自动 refresh"
    40029: "api freq out of limit → 限流降速"
    50000: "internal server error → 重试"

# ============================================================
# 限流配置
# 官方文档: 频控和创建限制
# ============================================================
rate_limiting:
  algorithm: token_bucket
  # ⚠️ 快手 MAPI 存在两种限流:
  #   1. QPS 限流（每秒请求数）
  #   2. 日调用量限流（每日总调用次数，超限报错 "该接口今天调用量已经超限"）
  # 具体限额因接口和开发者等级不同，需在实际环境中确认
  global_qps: "80%_of_official_limit"      # 预留 20% 安全余量
  per_account_qps: "global_qps / active_accounts"
  daily_quota_buffer: "90%"                 # 日调用量使用 90% 即触发降频
  priority_order:
    - realtime_report    # 告警数据源，最高优先级
    - dashboard_data     # 概览页
    - analysis_data      # 效果分析
    - historical_archive # 历史归档
  request_merging: true  # 同账户多指标合并为单次请求
  on_rate_limit: >
    收到 code=40029 时：暂停该接口 60s → 指数退避重试；
    收到 "今天调用量已超限" 时：该接口当日不再调用，次日 00:00 恢复。

# ============================================================
# 数据拉取调度
# ============================================================
scheduler:
  realtime_report:
    foreground_interval: 30s
    background_interval: 300s
  account_info:
    interval: 300s  # 5分钟
  campaign_list:
    interval: 300s
  daily_report:
    cron: "0 2 * * *"  # 每天凌晨2点
  token_refresh:
    check_interval: 60s
    # access_token 1天有效，提前1小时刷新
    preemptive_refresh_before: 3600s
  db_backup:
    cron: "0 3 * * *"  # 每天凌晨3点
    retention_days: 30
```

---

## 10. 告警引擎伪代码

```rust
// === src-tauri/src/services/alert_engine.rs ===
// 核心告警匹配逻辑

pub async fn evaluate_alerts(
    db: &Database,
    snapshot: &MetricSnapshot,
    rules: &\[AlertRule\],
    push_service: &PushService,
    app_handle: &AppHandle,
) -> Result<Vec<AlertEvent>> {
    let mut triggered = Vec::new();
    let now = Utc::now();

    for rule in rules.iter().filter(|r| r.enabled) {
        // 1. 作用域匹配
        if !matches_scope(rule, snapshot) { continue; }

        // 2. 静默时段检查
        if is_in_quiet_hours(rule, &now) { continue; }

        // 3. 重复推送间隔检查
        if let Some(last) = db.get_last_alert_for_rule(rule.id).await? {
            let elapsed = (now - last.triggered_at).num_seconds();
            if elapsed < rule.min_interval_sec { continue; }
        }

        // 4. 阈值判定
        let threshold: AlertThreshold = serde_json::from_str(&rule.threshold_json)?;
        let is_triggered = match rule.rule_type.as_str() {
            "budget_overrun" => {
                let account = db.get_account(&snapshot.ref_id).await?;
                let pct = snapshot.cost / account.daily_budget \* 100.0;
                pct >= threshold.budget_percent.unwrap_or(100.0)
            }
            "ctr_drop" => {
                let avg = db.get_avg_ctr(
                    &snapshot.ref_id,
                    threshold.lookback_days.unwrap_or(7)
                ).await?;
                let drop_pct = (avg - snapshot.ctr) / avg \* 100.0;
                drop_pct >= threshold.percent_drop.unwrap_or(30.0)
            }
            "roi_drop" => {
                let min_cost = threshold.min_cost.unwrap_or(0.0);
                snapshot.cost > min_cost
                    && snapshot.roi < threshold.absolute_value.unwrap_or(1.0)
            }
            "balance_low" => {
                let account = db.get_account(&snapshot.ref_id).await?;
                account.balance <= threshold.absolute_value.unwrap_or(500.0)
            }
            "cpa_spike" => {
                let avg = db.get_avg_cpa(
                    &snapshot.ref_id,
                    threshold.lookback_days.unwrap_or(7)
                ).await?;
                let spike_pct = (snapshot.cpa - avg) / avg \* 100.0;
                spike_pct >= threshold.percent_drop.unwrap_or(45.0)
            }
            "creative_rejected" => false, // 由 creative 同步逻辑单独触发
            "token_expired" => false,     // 由 token 刷新逻辑单独触发
            _ => false,
        };

        if is_triggered {
            // 5. 生成告警事件
            let event = AlertEvent {
                rule_id: rule.id,
                level: determine_level(&rule.rule_type),
                subject: format_subject(&rule.rule_type, snapshot),
                detail_json: build_detail(snapshot, &threshold),
                triggered_at: now,
                handled: false,
                ..Default::default()
            };
            let event = db.insert_alert_event(&event).await?;

            // 6. IM 推送
            push_service.send_alert(&event).await?;

            // 7. 通知前端
            app_handle.emit("new_alert", &event)?;

            triggered.push(event);
        }
    }
    Ok(triggered)
}

fn determine_level(rule_type: &str) -> &str {
    match rule_type {
        "budget_overrun" | "roi_drop" | "balance_low" | "token_expired" => "critical",
        "ctr_drop" | "cpa_spike" => "warning",
        "creative_rejected" => "info",
        _ => "info",
    }
}
```

---

## 11. IM Webhook 推送格式

```tsx
// === 各 IM 平台 Webhook payload 格式 ===

// --- 企业微信 (WeCom) ---
interface WeComPayload {
  msgtype: 'markdown';
  markdown: {
    content: string;  // 支持 markdown 子集
  };
}
// 模板:
// "### 🚨 SSAI告警: \${subject}\n"
// "> 账户: <font color=\\"warning\\">\${accountName}</font>\n"
// "> 当前值: \${currentValue} | 阈值: \${thresholdValue}\n"
// "> 时间: \${triggeredAt}\n"
// "> 建议: \${suggestion}\n"
// "\[去快手后台处理\](\${actionUrl})"

// --- 钉钉 (DingTalk) ---
interface DingTalkPayload {
  msgtype: 'markdown';
  markdown: {
    title: string;    // 通知标题(移动端显示)
    text: string;     // markdown 内容
  };
  at?: {
    atMobiles?: string\[\];  // @指定手机号
    isAtAll?: boolean;
  };
}
// 签名: HmacSHA256(timestamp + "\n" + secret)

// --- 飞书 (Feishu/Lark) ---
interface FeishuPayload {
  msg_type: 'interactive';
  card: {
    header: {
      title: { tag: 'plain_text'; content: string };
      template: 'red' | 'orange' | 'grey';
    };
    elements: Array<{
      tag: 'div';
      text: { tag: 'lark_md'; content: string };
    }>;
  };
}

// --- Slack ---
interface SlackPayload {
  text: string;       // fallback 纯文本
  blocks: Array<{
    type: 'section';
    text: { type: 'mrkdwn'; text: string };
  }>;
}
```

---

## 12. Design Token（CSS 变量）

```css
/\* === src/styles/variables.css === \*/
/\* 深色主题 (默认) — 对照效果图 \*/

:root {
  /\* --- 背景色 --- \*/
  --bg-primary:        #0D0D0D;
  --bg-card:           #1A1A2E;
  --bg-sidebar:        #111122;
  --bg-topbar:         #0D0D1A;
  --bg-modal:          #1E1E30;
  --bg-input:          #252540;
  --bg-hover:          #2A2A45;

  /\* --- 品牌色 --- \*/
  --brand-primary:     #FF6600;
  --brand-hover:       #FF8533;
  --brand-active:      #E65C00;
  --brand-alpha-20:    rgba(255, 102, 0, 0.2);

  /\* --- 语义色 --- \*/
  --color-up:          #00D68F;
  --color-down:        #FF3B3B;
  --color-warning:     #FFB800;
  --color-info:        #4A90D9;

  /\* --- 文字色 --- \*/
  --text-primary:      #E0E0E0;
  --text-secondary:    #888888;
  --text-disabled:     #555555;
  --text-inverse:      #FFFFFF;

  /\* --- 边框 --- \*/
  --border:            #2A2A3E;
  --border-active:     rgba(255, 102, 0, 0.2);

  /\* --- 圆角 --- \*/
  --radius-card:       8px;
  --radius-button:     6px;
  --radius-badge:      50%;
  --radius-modal:      12px;

  /\* --- 阴影 --- \*/
  --shadow-modal:      0 8px 32px rgba(0, 0, 0, 0.5);
  --shadow-dropdown:   0 4px 16px rgba(0, 0, 0, 0.4);

  /\* --- 间距 --- \*/
  --spacing-xs:  4px;  --spacing-sm:  8px;
  --spacing-md:  12px; --spacing-lg:  16px;
  --spacing-xl:  24px; --spacing-2xl: 32px;

  /\* --- 布局 --- \*/
  --sidebar-width:     220px;
  --sidebar-collapsed: 60px;
  --topbar-height:     56px;
  --card-height:       110px;
  --max-content-width: 1600px;

  /\* --- 字体 --- \*/
  --font-data: 'DIN Alternate', 'DIN Pro', 'Roboto Mono', monospace;
  --font-body: 'PingFang SC', 'Microsoft YaHei', -apple-system, sans-serif;
  --font-size-data-xl: 40px;
  --font-size-title:   15px;
  --font-size-body:    14px;
  --font-size-label:   13px;
  --font-size-small:   12px;

  /\* --- 动画 --- \*/
  --transition-fast:   0.15s ease;
  --transition-normal: 0.25s ease;
  --transition-slow:   0.35s ease-out;
}
```

---

## 13. 前端路由与组件树

```tsx
// === src/App.tsx ===
const routes = \[
  { path: '/',           component: 'Dashboard',     icon: '📊', label: '数据概览'  },
  { path: '/accounts',   component: 'Accounts',      icon: '👤', label: '广告账户'  },
  { path: '/monitor',    component: 'Monitor',       icon: '🎯', label: '投放监控'  },
  { path: '/analysis',   component: 'Analysis',      icon: '📈', label: '效果分析'  },
  { path: '/alerts',     component: 'Alerts',        icon: '🚨', label: '告警中心', badge: 'alertCount' },
  { path: '/ai',         component: 'AIOptimize',    icon: '🧠', label: '智能优化', tag: 'AI' },
  { path: '/reports',    component: 'Reports',       icon: '📑', label: '报表中心'  },
  { path: '/materials',  component: 'Materials',     icon: '🖼️', label: '素材管理'  },
  { path: '/settings',   component: 'Settings',      icon: '⚙️', label: '系统设置'  },
  { path: '/push',       component: 'PushSettings',  icon: '🔔', label: '推送设置', tag: '新' },
\] as const;
```

```
Dashboard 页面组件树:
├── MetricCards (5张卡片, grid 5列)
│   └── MetricCard × 5
│       ├── label (13px gray)
│       ├── value (40px DIN Pro, colored)
│       ├── Sparkline (30px mini chart)
│       └── ChangeTag (+12.5%↑ green / -3.2%↓ red)
├── Row2 (3栏)
│   ├── TrendChart (col 1-5, 280px, ECharts dual-axis line)
│   ├── ChannelPie (col 6-8, 280px, ECharts doughnut)
│   └── AccountStatus (col 9-12, 280px, mini table)
└── Row3 (2栏)
    ├── AlertFeed (col 1-5, 220px, filterable list)
    └── EffectScatter (col 6-12, 220px, scatter + summary cards)
```

---

## 14. Zustand Store 结构

```tsx
// === src/stores/accountStore.ts ===
interface AccountStore {
  accounts: Account\[\];
  loading: boolean;
  fetchAccounts: () => Promise<void>;
  updateTag: (accountId: string, tag: string | null) => Promise<void>;
  removeAccount: (accountId: string) => Promise<void>;
}

// === src/stores/metricsStore.ts ===
interface MetricsStore {
  dashboardCards: DashboardMetricCard\[\];
  trendData: { timestamps: string\[\]; costSeries: number\[\]; ctrSeries: number\[\] };
  channelDist: ChannelDistribution\[\];
  lastUpdated: string | null;
  loading: boolean;
  fetchDashboard: () => Promise<void>;
  fetchTrend: (range: 'hour' | 'day' | '7d' | '30d') => Promise<void>;
}

// === src/stores/alertStore.ts ===
interface AlertStore {
  recentAlerts: AlertEvent\[\];
  allAlerts: { events: AlertEvent\[\]; total: number };
  rules: AlertRule\[\];
  unhandledCount: number;      // TopBar badge
  loading: boolean;
  fetchRecent: (limit?: number) => Promise<void>;
  fetchAll: (params: ListAlertParams) => Promise<void>;
  handleAlert: (eventId: number) => Promise<void>;
}

// === src/stores/settingsStore.ts ===
interface SettingsStore {
  settings: AppSettings;
  loading: boolean;
  fetch: () => Promise<void>;
  update: (updates: Partial<AppSettings>) => Promise<void>;
}
```

---

## 15. 数据保留与清理策略

```yaml
data_retention:
  metrics_snapshots:
    minute_level: { retention: 90d,  cleanup: "aggregate to hourly", cron: "0 4 * * *" }
    hourly_level: { retention: 365d, cleanup: "aggregate to daily",  cron: "0 4 1 * *" }
    daily_level:  { retention: forever }
  alert_events:   { retention: forever }
  push_logs:      { retention: 180d }
  ai_suggestions: { retention: 365d }
  db_backup:      { retention_days: 30 }

db_maintenance:
  wal_checkpoint: "PRAGMA wal_checkpoint(TRUNCATE)"  # 每周一次
  vacuum: "VACUUM"                                    # 每月一次
  analyze: "ANALYZE"                                  # 每周一次
```

---

## 16. 错误码枚举

```tsx
// === src/types/errors.ts ===
export enum ErrorCode {
  // 网络
  NETWORK_TIMEOUT       = 'E1001',
  NETWORK_UNREACHABLE   = 'E1002',
  // 快手 API
  KS_RATE_LIMITED       = 'E2001',
  KS_UNAUTHORIZED       = 'E2002',
  KS_SERVER_ERROR       = 'E2003',
  KS_FIELD_MISSING      = 'E2004',
  // Token
  TOKEN_REFRESH_FAILED  = 'E3001',
  TOKEN_EXPIRED         = 'E3002',
  TOKEN_KEYCHAIN_ERROR  = 'E3003',
  // 推送
  PUSH_WEBHOOK_FAILED   = 'E4001',
  PUSH_SIGN_ERROR       = 'E4002',
  // 数据库
  DB_WRITE_FAILED       = 'E5001',
  DB_MIGRATION_FAILED   = 'E5002',
  // AI
  LLM_API_FAILED        = 'E6001',
  LLM_KEY_MISSING       = 'E6002',
  // 通用
  UNKNOWN               = 'E9999',
}

export interface AppError {
  code: ErrorCode;
  message: string;
  details?: Record<string, unknown>;
  timestamp: string;
}
```

---

## 17. ECharts 深色主题配置

```tsx
// === src/styles/echarts-theme.ts ===
export const darkTheme = {
  color: \['#FF6600', '#4A90D9', '#00D68F', '#FFB800', '#FF3B3B', '#888888'\],
  backgroundColor: 'transparent',
  textStyle: { fontFamily: 'PingFang SC, Microsoft YaHei, sans-serif', color: '#E0E0E0' },
  tooltip: {
    backgroundColor: '#1A1A2E',
    borderColor: '#2A2A3E',
    textStyle: { color: '#E0E0E0', fontSize: 13 },
  },
  xAxis: {
    axisLine: { lineStyle: { color: '#2A2A3E' } },
    axisLabel: { color: '#888888' },
    splitLine: { lineStyle: { color: '#1A1A2E' } },
  },
  yAxis: {
    axisLine: { lineStyle: { color: '#2A2A3E' } },
    axisLabel: { color: '#888888' },
    splitLine: { lineStyle: { color: '#1A1A2E', type: 'dashed' } },
  },
};
// 注册: echarts.registerTheme('ssai-dark', darkTheme)
```

---

## 18. Tauri 配置要点

```json
// === src-tauri/tauri.conf.json (关键字段) ===
{
  "productName": "SSAI-快手监控",
  "identifier": "com.ssai.ks-monitor",
  "version": "1.0.0",
  "app": {
    "windows": \[{
      "title": "SSAI-快手监控",
      "width": 1440, "height": 900,
      "minWidth": 1280, "minHeight": 800,
      "decorations": true, "resizable": true
    }\],
    "security": {
      "csp": "default-src 'self'; connect-src 'self' https://kuaishou.ssaitool.cn wss://kuaishou.ssaitool.cn https://ad.e.kuaishou.com https://open.kuaishou.com https://oapi.dingtalk.com https://open.feishu.cn https://qyapi.weixin.qq.com https://hooks.slack.com"
    }
  },
  "plugins": {
    "updater": {
      "endpoints": \["https://your-cdn.com/ssai-ksdesktop/update.json"\],
      "pubkey": "YOUR_PUBLIC_KEY_HERE"
    }
  }
}
```

---

## 19. 开发命令速查

```bash
# 日常开发
pnpm install                    # 安装前端依赖
pnpm tauri dev                  # 启动开发模式 (HMR + Rust 热重载)
pnpm dev                        # 仅启动前端 (Vite dev server)

# 代码质量
pnpm lint                       # ESLint 检查
pnpm format                     # Prettier 格式化
pnpm typecheck                  # TypeScript 类型检查
cd src-tauri && cargo clippy     # Rust lint
cd src-tauri && cargo fmt        # Rust 格式化

# 测试
pnpm test                       # vitest 前端单元测试
pnpm test:e2e                   # Playwright E2E 测试
cd src-tauri && cargo test       # Rust 测试

# 构建
pnpm tauri build                # 生产构建 (当前平台)
```

---

## 20. 项目目录结构（完整 + 章节交叉引用）

```
ssai-ksdesktop/
├── src-tauri/                        # Rust 后端
│   ├── Cargo.toml                    # (见§2)
│   ├── tauri.conf.json               # (见§18)
│   ├── icons/
│   └── src/
│       ├── main.rs
│       ├── commands/                 # IPC 命令 (见§6)
│       │   ├── mod.rs
│       │   ├── accounts.rs
│       │   ├── metrics.rs
│       │   ├── alerts.rs
│       │   ├── push.rs
│       │   ├── ai.rs
│       │   └── settings.rs
│       ├── services/                 # 业务逻辑
│       │   ├── scheduler.rs          # (见§9)
│       │   ├── kuaishou_api.rs
│       │   ├── field_mapper.rs       # (见§8)
│       │   ├── rate_limiter.rs       # (见§9)
│       │   ├── alert_engine.rs       # (见§10)
│       │   ├── push_service.rs       # (见§11)
│       │   ├── ai_engine.rs
│       │   └── backup.rs
│       ├── db/
│       │   ├── schema.rs             # DDL (见§5)
│       │   ├── accounts_repo.rs
│       │   ├── metrics_repo.rs
│       │   ├── alerts_repo.rs
│       │   └── push_repo.rs
│       ├── models/                   # (见§4)
│       └── utils/
│           ├── keychain.rs
│           ├── logger.rs
│           └── crypto.rs
├── src/                              # React 前端
│   ├── main.tsx
│   ├── App.tsx                       # 路由 (见§13)
│   ├── types/                        # TS 类型 (见§3)
│   ├── services/ipc.ts               # IPC 契约 (见§6)
│   ├── layouts/
│   ├── pages/                        # (见§13 组件树)
│   ├── components/
│   ├── stores/                       # Zustand (见§14)
│   ├── hooks/                        # (见§7)
│   ├── styles/                       # (见§12, §17)
│   └── utils/
├── public/field-mapping.json         # (见§8)
├── tests/
├── scripts/
├── .github/workflows/
├── package.json                      # (见§2)
├── vite.config.ts
└── .env.example
```

---

## 21. 实现优先级（Codex 任务队列）

```yaml
# 建议 Codex 按以下顺序实现，每个 task 可独立提交

Phase_1_Scaffold:
  - task: "初始化 Tauri + React 项目脚手架"
  - task: "创建所有 TypeScript 类型文件 (src/types/*)"
  - task: "创建 CSS 变量 + 全局样式 + ECharts 主题"
  - task: "创建 MainLayout + Sidebar + TopBar"
  - task: "配置路由"

Phase_2_Backend_Core:
  - task: "SQLite schema + 迁移"
  - task: "Rust models"
  - task: "Keychain 工具"
  - task: "快手 API 客户端 + 字段映射 + 限流器"
  - task: "OAuth 流程 (authorize + callback + token refresh)"

Phase_3_Data_Pipeline:
  - task: "定时调度器"
  - task: "数据入库 repo 层"
  - task: "IPC 命令层 - metrics"
  - task: "前端 Zustand stores"
  - task: "Tauri Event hook"

Phase_4_Dashboard:
  - task: "MetricCards 组件"
  - task: "TrendChart (ECharts dual-axis)"
  - task: "ChannelPie (ECharts doughnut)"
  - task: "AccountStatus 迷你表格"
  - task: "AlertFeed 列表"
  - task: "EffectScatter 散点图"

Phase_5_Alert_System:
  - task: "告警引擎 (规则匹配)"
  - task: "IM 推送服务 (4平台 Webhook)"
  - task: "告警中心页 UI"
  - task: "推送设置页 + IM弹窗"

Phase_6_AI_and_Polish:
  - task: "AI 建议引擎 (规则+LLM)"
  - task: "智能优化页 UI"
  - task: "系统设置页"
  - task: "广告账户管理页"
  - task: "投放监控页 (三级联动表格)"
```

---

## 22. v1.0 待补充清单

<aside>
✅

本清单按 v1.0 最新进度刷新：
✅ = 已在下方对应章节补齐；🔄 = 规格已定，等待真机/上线验证；⏳ = 仍待补充。
表尾新增「服务端」分组对应 §35-§41 的混合架构落地项。

</aside>

| 类别 | 待补充项 | 优先级 | 状态 |
| --- | --- | --- | --- |
| API 验证 | OAuth 全流程沙盒跑通（auth_code→access_token→refresh） | P0 | 🔄 规格见 §9 / §35.3；需 APP_ID=165926585 真机走通 |
| API 验证 | 磁力金牛 ESP 报表实际延迟与最小粒度 | P0 | 🔄 §36 已列端点；待上线实测 /v2/esp/*/report |
| API 验证 | 多账户 QPS 限额（共享 vs 独立） | P0 | 🔄 §9 rate_limiting 已预留降速策略，需实测校准 |
| API 验证 | /v1/advertiser/list 返回字段确认（2024 后迁移） | P0 | 🔄 §9 已标注旧字段下线 |
| 安全 | app_secret 分发方案 | P0 | ✅ §23 + §40 服务端集中保管 |
| 安全 | 日志脱敏规则（10 条正则） | P1 | ✅ §24 |
| 安全 | 密钥轮换 SOP | P1 | ✅ §40.4 |
| 安全 | fail2ban / SSH 硬化 | P1 | ✅ §40.5 |
| 数据库 | SQLite 客户端缓存迁移运行器 | P1 | ✅ §25 |
| 数据库 | PostgreSQL 服务端权威 DDL | P0 | ✅ §37.4 |
| 数据库 | 快照聚合 SQL（分→时→日） | P1 | ✅ §26 |
| 数据库 | PG → SQLite 增量同步规则 | P1 | ✅ §39.4 sync_cursor |
| 数据库 | pg_dump 自动备份 + 保留 30d | P1 | ✅ §41.1 |
| AI | 健康度评分 5 维度权重 | P1 | ✅ §27 |
| AI | LLM Prompt 模板（4 份） | P1 | ✅ §28 |
| AI | 无 LLM 降级建议库 | P1 | ✅ §29 覆盖 7 类告警 |
| UI | 深色主题 Design Token | P1 | ✅ §12 |
| UI | 浅色主题 Design Token | P2 | ✅ §43 |
| UI | 空状态插画与文案规范 | P2 | ✅ §44 |
| UI | 全局 Loading / Error 方案 | P1 | ✅ §32 |
| 推送 | IM 推送模板（4 平台） | P1 | ✅ §30 |
| 推送 | 推送失败重试队列（5 次指数退避） | P1 | ✅ §31 |
| 测试 | Mock Server 数据集（7 场景） | P1 | ✅ §33 |
| 测试 | E2E 测试用例清单 | P2 | ✅ §45 |
| 发布 | macOS 签名证书步骤 | P2 | ✅ §46.1 |
| 发布 | Windows 代码签名步骤 | P2 | ✅ §46.2 |
| 发布 | [CHANGELOG.md](http://CHANGELOG.md) 模板 | P2 | ✅ §47 |
| 发布 | GitHub Actions 发版流水线 | P1 | ✅ §41.4 |
| 文档 | [README.md](http://README.md) 初版（安装/截图/FAQ） | P1 | ⏳ 各章节已就位，可由 Codex 汇总 |
| 服务端 | Rust Axum 骨架 + 目录结构 | P0 | ✅ §37 |
| 服务端 | 阿里云 Ubuntu 24.04 部署全流程 | P0 | ✅ §38（DNS/安全组/[init.sh/Nginx/Certbot/systemd）](http://init.sh/Nginx/Certbot/systemd）) |
| 服务端 | 客户端↔服务端 REST + WS 协议 | P0 | ✅ §39 |
| 服务端 | .env 模板 + 密钥分层 | P0 | ✅ §40 |
| 服务端 | systemd + 健康检查 + 日志轮转 | P1 | ✅ §38.5 / §41.2 / §41.3 |
| 服务端 | Prometheus 指标上报 | P2 | ⏳ §41.3 标注 deferred，v1.1 再做 |

---

## 23. app_secret 分发方案（P0 架构决策）

<aside>
🔐

架构决策（已更新为混合架构，详见§35）：App Secret **仅存储于服务端** <code>.env</code> 或阿里云 KMS。
客户端二进制内 **绝不包含 Secret**，也不需要用户输入。
用户只需在客户端用本地账号登录→打开授权链接→授权后自动回跳服务端完成 Token 换取。
本节原「用户自行输入」方案已废弃；保留 IPC 契约用于客户端设置页展示当前授权状态。

</aside>

```yaml
# 方案对比
Option_A_Embed_In_Binary:
  pros: "用户无感知，开箱即用"
  cons:
    - "反编译可提取 secret → 严重安全风险"
    - "快手开放平台可能因此封禁应用"
    - "更换 secret 需发布新版本"
  verdict: "❌ 不推荐"

Option_B_User_Input:  # ✅ 推荐方案
  pros:
    - "secret 永不出现在代码/二进制中"
    - "每个用户可用自己的开发者账号"
    - "更换 secret 无需发版"
  cons: "首次使用多一步配置"
  verdict: "✅ 推荐"

# 实现细节
implementation:
  first_launch_flow:
    1: "应用启动 → 检测 Keychain 中无 app_id/app_secret"
    2: "弹出『快手开放平台配置』引导弹窗"
    3: "用户输入 app_id + app_secret"
    4: "前端调用 IPC: save_app_credentials(app_id, app_secret)"
    5: "Rust 端存入 Keychain: ks_app_id / ks_app_secret"
    6: "跳转 OAuth 授权流程"

  keychain_keys:
    app_id: "ssai-ksdesktop.app_id"
    app_secret: "ssai-ksdesktop.app_secret"
    token_per_account: "ssai-ksdesktop.token.{account_id}"
    refresh_per_account: "ssai-ksdesktop.refresh.{account_id}"
    webhook_per_channel: "ssai-ksdesktop.webhook.{channel_id}"
    llm_api_key: "ssai-ksdesktop.llm_api_key"

  settings_page:
    section: "快手开放平台"
    fields:
      - { label: "App ID", type: "text", masked: false }
      - { label: "App Secret", type: "password", masked: true, show_toggle: true }
    actions:
      - "保存" # 存入 Keychain
      - "验证连接" # 调用 /oauth2/token 验证凭据有效性
```

```tsx
// === 新增 IPC 命令 (追加到 src/services/ipc.ts) ===
export const credentialCommands = {
  /** 检查是否已配置 app credentials */
  hasAppCredentials: () =>
    invoke<boolean>('has_app_credentials'),

  /** 保存 app_id + app_secret 到 Keychain */
  saveAppCredentials: (appId: string, appSecret: string) =>
    invoke<void>('save_app_credentials', { appId, appSecret }),

  /** 验证 app credentials 是否有效 */
  verifyAppCredentials: () =>
    invoke<{ valid: boolean; message: string }>('verify_app_credentials'),
};
```

---

## 24. 日志脱敏规则（✅ P1）

```rust
// === src-tauri/src/utils/logger.rs ===
// 日志脱敏处理器：所有日志在写入前经过此层过滤

use regex::Regex;
use once_cell::sync::Lazy;

/// 脱敏规则列表
static SANITIZE_RULES: Lazy<Vec<SanitizeRule>> = Lazy::new(|| vec!\[
    // 1. OAuth access_token (32-128位字符串)
    SanitizeRule {
        name: "access_token",
        pattern: Regex::new(
            r#"(?i)(access_token|accesstoken)["':\s=]+["']?([a-zA-Z0-9._\-]{20,128})["']?"#
        ).unwrap(),
        replacement: "$1=\"***REDACTED***\"",
    },
    // 2. OAuth refresh_token
    SanitizeRule {
        name: "refresh_token",
        pattern: Regex::new(
            r#"(?i)(refresh_token|refreshtoken)["':\s=]+["']?([a-zA-Z0-9._\-]{20,128})["']?"#
        ).unwrap(),
        replacement: "$1=\"***REDACTED***\"",
    },
    // 3. app_secret
    SanitizeRule {
        name: "app_secret",
        pattern: Regex::new(
            r#"(?i)(app_secret|appsecret|client_secret)["':\s=]+["']?([a-zA-Z0-9]{16,64})["']?"#
        ).unwrap(),
        replacement: "$1=\"***REDACTED***\"",
    },
    // 4. Webhook URL (企微/钉钉/飞书/Slack)
    SanitizeRule {
        name: "webhook_url",
        pattern: Regex::new(
            r#"(https://(?:qyapi\.weixin\.qq\.com|oapi\.dingtalk\.com|open\.feishu\.cn|hooks\.slack\.com)/[^\s"']{10,})"#
        ).unwrap(),
        replacement: "***WEBHOOK_REDACTED***",
    },
    // 5. 手机号 (中国大陆1xx)
    SanitizeRule {
        name: "phone",
        pattern: Regex::new(r"1[3-9]\d{9}").unwrap(),
        replacement: "1**\*\*\*\*\*\*\*\*",
    },
    // 6. 邮箱地址
    SanitizeRule {
        name: "email",
        pattern: Regex::new(
            r"[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}"
        ).unwrap(),
        replacement: "***@***.***",
    },
    // 7. 快手账户 ID (只保留前2后2位)
    SanitizeRule {
        name: "account_id",
        pattern: Regex::new(
            r#"(?i)(account_id|advertiser_id)["':\s=]+["']?(\d{2})(\d+)(\d{2})["']?"#
        ).unwrap(),
        replacement: "$1=\"$2***$4\"",
    },
    // 8. Authorization header
    SanitizeRule {
        name: "auth_header",
        pattern: Regex::new(
            r#"(?i)(authorization)["':\s=]+["']?(Bearer\s+)([a-zA-Z0-9._\-]+)["']?"#
        ).unwrap(),
        replacement: "$1: \"Bearer ***REDACTED***\"",
    },
    // 9. LLM API Key (sk-xxx / key-xxx)
    SanitizeRule {
        name: "llm_key",
        pattern: Regex::new(
            r"(?i)(sk-|key-|api[_-]?key[=:\s]+)[a-zA-Z0-9]{16,}"
        ).unwrap(),
        replacement: "***LLM_KEY_REDACTED***",
    },
    // 10. HmacSHA256 签名值
    SanitizeRule {
        name: "sign",
        pattern: Regex::new(
            r#"(?i)(sign|signature|secret)["':\s=]+["']?([a-fA-F0-9]{32,128})["']?"#
        ).unwrap(),
        replacement: "$1=\"***SIGN_REDACTED***\"",
    },
\]);

pub struct SanitizeRule {
    pub name: &'static str,
    pub pattern: Regex,
    pub replacement: &'static str,
}

/// 对日志消息执行全部脱敏规则
pub fn sanitize(input: &str) -> String {
    let mut output = input.to_string();
    for rule in SANITIZE_RULES.iter() {
        output = rule.pattern.replace_all(&output, rule.replacement).to_string();
    }
    output
}

// 集成方式: 自定义 tracing Layer，在 format 层调用 sanitize()
```

---

## 25. Schema 版本迁移策略（✅ P1）

```rust
// === src-tauri/src/db/migration.rs ===
// 简单的线性迁移运行器，适合单用户桌面应用

use rusqlite::Connection;

/// 迁移列表：每个元素 = (version, description, sql)
/// 新版本只需在末尾追加，永不修改已有条目
const MIGRATIONS: &\[(i64, &str, &str)\] = &\[
    (1, "v1.0.0 base schema", include_str!("sql/v1_0_0_base.sql")),
    // 未来版本在此追加:
    // (2, "v1.1.0 add revenue column", "ALTER TABLE metrics_snapshots ADD COLUMN revenue REAL DEFAULT 0;"),
    // (3, "v1.1.0 add push_logs index", "CREATE INDEX IF NOT EXISTS idx_push_logs_channel ON push_logs(channel_id);"),
\];

pub fn run_migrations(conn: &Connection) -> Result<(), rusqlite::Error> {
    // 1. 创建迁移跟踪表
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS _migrations (
            version     INTEGER PRIMARY KEY,
            description TEXT NOT NULL,
            applied_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );
    ")?;

    // 2. 获取当前已应用的最高版本
    let current_version: i64 = conn
        .query_row("SELECT COALESCE(MAX(version), 0) FROM _migrations", \[\], |r| r.get(0))?;

    // 3. 顺序执行未应用的迁移
    for &(version, description, sql) in MIGRATIONS {
        if version > current_version {
            tracing::info!("Applying migration v{}: {}", version, description);
            conn.execute_batch(sql)?;
            conn.execute(
                "INSERT INTO _migrations (version, description) VALUES (?1, ?2)",
                \[&version.to_string(), &description.to_string()\],
            )?;
        }
    }

    tracing::info!("Database schema is up to date (version {})",
        MIGRATIONS.last().map(|m| m.0).unwrap_or(0));
    Ok(())
}
```

```
文件结构:
src-tauri/src/db/
├── mod.rs
├── migration.rs          # 迁移运行器 (上述代码)
├── sql/
│   ├── v1_0_0_base.sql   # §5 的完整建表语句
│   └── v1_1_0_xxx.sql    # 未来版本追加
├── accounts_repo.rs
├── metrics_repo.rs
├── alerts_repo.rs
└── push_repo.rs

调用时机: main.rs 中 Tauri setup 钩子内，在所有服务初始化前执行
```

---

## 26. 快照聚合 SQL（✅ P1）

```sql
-- === 分钟级 → 小时级聚合 ===
-- 调度: 每天 04:00 执行
-- 保留: 分钟级数据保留 90 天

-- Step 1: 插入小时级聚合数据
INSERT INTO metrics_snapshots (level, ref_id, snapshot_at, cost, impressions, clicks, conversions, roi, ctr, cvr, cpa, revenue)
SELECT
    level,
    ref_id,
    -- 截断到小时: "2024-05-20T14:00:00Z"
    strftime('%Y-%m-%dT%H:00:00Z', snapshot_at) AS hour_bucket,
    -- 消耗/曝光/点击/转化/成交额: 用最后一次快照的累计值(快手报表为当日累计)
    MAX(cost) AS cost,
    MAX(impressions) AS impressions,
    MAX(clicks) AS clicks,
    MAX(conversions) AS conversions,
    -- 比率指标: 用该小时内最后一次快照的值
    (SELECT m2.roi FROM metrics_snapshots m2
     WHERE m2.level = metrics_snapshots.level
       AND m2.ref_id = metrics_snapshots.ref_id
       AND strftime('%Y-%m-%dT%H:00:00Z', m2.snapshot_at) = strftime('%Y-%m-%dT%H:00:00Z', metrics_snapshots.snapshot_at)
     ORDER BY m2.snapshot_at DESC LIMIT 1) AS roi,
    (SELECT m2.ctr FROM metrics_snapshots m2
     WHERE m2.level = metrics_snapshots.level
       AND m2.ref_id = metrics_snapshots.ref_id
       AND strftime('%Y-%m-%dT%H:00:00Z', m2.snapshot_at) = strftime('%Y-%m-%dT%H:00:00Z', metrics_snapshots.snapshot_at)
     ORDER BY m2.snapshot_at DESC LIMIT 1) AS ctr,
    (SELECT m2.cvr FROM metrics_snapshots m2
     WHERE m2.level = metrics_snapshots.level
       AND m2.ref_id = metrics_snapshots.ref_id
       AND strftime('%Y-%m-%dT%H:00:00Z', m2.snapshot_at) = strftime('%Y-%m-%dT%H:00:00Z', metrics_snapshots.snapshot_at)
     ORDER BY m2.snapshot_at DESC LIMIT 1) AS cvr,
    (SELECT m2.cpa FROM metrics_snapshots m2
     WHERE m2.level = metrics_snapshots.level
       AND m2.ref_id = metrics_snapshots.ref_id
       AND strftime('%Y-%m-%dT%H:00:00Z', m2.snapshot_at) = strftime('%Y-%m-%dT%H:00:00Z', metrics_snapshots.snapshot_at)
     ORDER BY m2.snapshot_at DESC LIMIT 1) AS cpa,
    MAX(revenue) AS revenue
FROM metrics_snapshots
WHERE snapshot_at < datetime('now', '-90 days')
  AND snapshot_at NOT LIKE '%:00:00Z'  -- 排除已经是整点的小时级数据
GROUP BY level, ref_id, hour_bucket;

-- Step 2: 删除过期的分钟级原始数据
DELETE FROM metrics_snapshots
WHERE snapshot_at < datetime('now', '-90 days')
  AND snapshot_at NOT LIKE '%:00:00Z';

-- === 小时级 → 日级聚合 ===
-- 调度: 每月 1 号 04:00 执行
-- 保留: 小时级数据保留 365 天

INSERT INTO metrics_snapshots (level, ref_id, snapshot_at, cost, impressions, clicks, conversions, roi, ctr, cvr, cpa, revenue)
SELECT
    level,
    ref_id,
    strftime('%Y-%m-%dT00:00:00Z', snapshot_at) AS day_bucket,
    MAX(cost) AS cost,            -- 当日最终累计消耗
    MAX(impressions) AS impressions,
    MAX(clicks) AS clicks,
    MAX(conversions) AS conversions,
    -- 日级比率: 取当天最后一次快照
    (SELECT m2.roi FROM metrics_snapshots m2
     WHERE m2.level = metrics_snapshots.level AND m2.ref_id = metrics_snapshots.ref_id
       AND date(m2.snapshot_at) = date(metrics_snapshots.snapshot_at)
     ORDER BY m2.snapshot_at DESC LIMIT 1) AS roi,
    (SELECT m2.ctr FROM metrics_snapshots m2
     WHERE m2.level = metrics_snapshots.level AND m2.ref_id = metrics_snapshots.ref_id
       AND date(m2.snapshot_at) = date(metrics_snapshots.snapshot_at)
     ORDER BY m2.snapshot_at DESC LIMIT 1) AS ctr,
    (SELECT m2.cvr FROM metrics_snapshots m2
     WHERE m2.level = metrics_snapshots.level AND m2.ref_id = metrics_snapshots.ref_id
       AND date(m2.snapshot_at) = date(metrics_snapshots.snapshot_at)
     ORDER BY m2.snapshot_at DESC LIMIT 1) AS cvr,
    (SELECT m2.cpa FROM metrics_snapshots m2
     WHERE m2.level = metrics_snapshots.level AND m2.ref_id = metrics_snapshots.ref_id
       AND date(m2.snapshot_at) = date(metrics_snapshots.snapshot_at)
     ORDER BY m2.snapshot_at DESC LIMIT 1) AS cpa,
    MAX(revenue) AS revenue
FROM metrics_snapshots
WHERE snapshot_at < datetime('now', '-365 days')
  AND snapshot_at LIKE '%:00:00Z'     -- 只聚合小时级数据
  AND snapshot_at NOT LIKE '%T00:00:00Z' -- 排除已是日级的
GROUP BY level, ref_id, day_bucket;

DELETE FROM metrics_snapshots
WHERE snapshot_at < datetime('now', '-365 days')
  AND snapshot_at LIKE '%:00:00Z'
  AND snapshot_at NOT LIKE '%T00:00:00Z';

-- === 聚合后执行维护 ===
PRAGMA wal_checkpoint(TRUNCATE);
ANALYZE metrics_snapshots;
```

```yaml
# 聚合规则说明
aggregation_rules:
  cumulative_fields:  # 快手报表返回的是当日累计值→用 MAX
    - cost
    - impressions
    - clicks
    - conversions
    - revenue
  rate_fields:        # 比率指标→取时间窗口内最后一次快照
    - roi
    - ctr
    - cvr
    - cpa
  reason: "快手报表的消耗/曝光/点击等是当日累计值，不能用 AVG，否则会低估。比率指标取最后快照即可。"
```

---

## 27. AI 健康度评分算法（✅ P1）

```tsx
// === src-tauri 对应 Rust 实现，此处用 TS 伪代码描述算法 ===
// 文件: src-tauri/src/services/ai_engine.rs → compute_health_score()

/**
 * 计划健康度评分 (0-100)
 * 
 * 核心思路：
 * 1. 5个维度分别评分 (0-100)
 * 2. 加权汇总得到总分
 * 3. 存在致命因素时直接降档
 */

// === 权重配置 ===
const WEIGHTS = {
  roi:           0.30,  // ROI 占 30%
  cpa:           0.25,  // CPA 占 25%
  ctr:           0.20,  // CTR 占 20%
  budgetPace:    0.15,  // 预算消耗速度 占 15%
  stability:     0.10,  // 指标稳定性 占 10%
};

// === 各维度评分函数 ===

/** ROI 维度 (越高越好) */
function scoreROI(current: number, target: number): number {
  // target 默认 = 过去7天均值 × 0.8
  if (current >= target * 1.5) return 100;  // 远超目标
  if (current >= target)       return 80;   // 达标
  if (current >= target * 0.7) return 50;   // 接近目标
  if (current >= target * 0.5) return 25;   // 明显低于目标
  return 5;                                  // 严重不达标
}

/** CPA 维度 (越低越好) */
function scoreCPA(current: number, avg7d: number): number {
  const ratio = current / avg7d;
  if (ratio <= 0.7)  return 100;  // 远低于均值
  if (ratio <= 1.0)  return 85;
  if (ratio <= 1.3)  return 55;
  if (ratio <= 1.8)  return 25;
  return 5;                        // CPA 飙升
}

/** CTR 维度 (越高越好) */
function scoreCTR(current: number, avg7d: number): number {
  const ratio = current / avg7d;
  if (ratio >= 1.3)  return 100;
  if (ratio >= 0.9)  return 80;
  if (ratio >= 0.7)  return 50;
  if (ratio >= 0.5)  return 25;
  return 5;
}

/** 预算消耗速度 (理想 = 当前时间占比 ± 15%) */
function scoreBudgetPace(spent: number, budget: number, hourOfDay: number): number {
  const idealPace = hourOfDay / 24;  // 0.0 ~ 1.0
  const actualPace = spent / budget;
  const deviation = Math.abs(actualPace - idealPace);
  if (deviation <= 0.05) return 100;  // 非常均匀
  if (deviation <= 0.15) return 80;   // 正常
  if (deviation <= 0.30) return 50;   // 偏快/偏慢
  if (deviation <= 0.50) return 25;   // 明显异常
  return 5;                            // 极端异常 (撞线/花不出去)
}

/** 指标稳定性 (过去24h变异系数) */
function scoreStability(recentValues: number[]): number {
  const mean = recentValues.reduce((a, b) => a + b, 0) / recentValues.length;
  const std = Math.sqrt(recentValues.reduce((s, v) => s + (v - mean) ** 2, 0) / recentValues.length);
  const cv = mean > 0 ? std / mean : 0;  // 变异系数
  if (cv <= 0.10) return 100;  // 非常稳定
  if (cv <= 0.25) return 75;
  if (cv <= 0.50) return 40;
  return 10;                    // 剧烈波动
}

/** 汇总 */
function computeHealthScore(campaign: CampaignMetrics): {
  score: number;
  level: 'excellent' | 'good' | 'warning' | 'critical';
  factors: string[];
} {
  const roiScore    = scoreROI(campaign.roi, campaign.roiTarget);
  const cpaScore    = scoreCPA(campaign.cpa, campaign.cpaAvg7d);
  const ctrScore    = scoreCTR(campaign.ctr, campaign.ctrAvg7d);
  const paceScore   = scoreBudgetPace(campaign.todayCost, campaign.dailyBudget, campaign.hourOfDay);
  const stabScore   = scoreStability(campaign.recentROIs);

  let score = Math.round(
    roiScore    * WEIGHTS.roi +
    cpaScore    * WEIGHTS.cpa +
    ctrScore    * WEIGHTS.ctr +
    paceScore   * WEIGHTS.budgetPace +
    stabScore   * WEIGHTS.stability
  );

  // === 致命因素降档 ===
  const factors: string[] = [];
  if (campaign.status === 'review_rejected') {
    score = Math.min(score, 10);
    factors.push('创意审核被拒');
  }
  if (campaign.status === 'budget_depleted') {
    score = Math.min(score, 30);
    factors.push('预算已耗尽');
  }
  if (campaign.roi < 0.5) factors.push('ROI 严重偏低');
  if (cpaScore <= 25)     factors.push('CPA 飙升');
  if (ctrScore <= 25)     factors.push('CTR 异常下降');
  if (paceScore <= 25)    factors.push('消耗速度异常');

  const level = score >= 80 ? 'excellent'
              : score >= 60 ? 'good'
              : score >= 40 ? 'warning'
              : 'critical';

  return { score, level, factors };
}
```

```yaml
# 健康度等级对应 UI 展示
health_levels:
  excellent: { range: "80-100", color: "var(--color-up)",    badge: "优秀", icon: "✅" }
  good:      { range: "60-79",  color: "var(--color-info)",  badge: "良好", icon: "👍" }
  warning:   { range: "40-59",  color: "var(--color-warning)", badge: "注意", icon: "⚠️" }
  critical:  { range: "0-39",   color: "var(--color-down)",  badge: "危险", icon: "🚨" }
```

---

## 28. LLM Prompt 模板（✅ P1）

```tsx
// === src-tauri/src/services/prompts/ ===
// 每个 prompt 独立文件，便于迭代

// --- prompts/system.txt ---
// 所有 LLM 调用共享的 system prompt
const SYSTEM_PROMPT = `<br>你是 SSAI 智能投放助手，专门分析快手广告投放数据。<br><br>## 身份约束<br>- 你只提供数据分析和操作建议，绝不直接操作广告。<br>- 用户需要在快手后台自行执行你建议的操作。<br>- 始终基于数据说话，给出具体数值和对比。<br><br>## 输出格式<br>- 使用简洁的中文<br>- 建议不超过 3 条<br>- 每条建议包含：原因 + 具体操作 + 预期效果<br>- 涉及数值时标注单位和对比基准<br>`;

// --- prompts/anomaly_diagnosis.txt ---
const ANOMALY_DIAGNOSIS_PROMPT = `<br>## 任务<br>分析以下广告计划的异常指标，给出归因诊断。<br><br>## 计划数据<br>- 计划名称: campaignName<br>- 账户: accountName<br>- 当前指标: 消耗=cost元, ROI=roi, CTR=ctr%, CPA=cpa元<br>- 过去7天均值: ROI=roiAvg7d, CTR=ctrAvg7d%, CPA=cpaAvg7d元<br>- 今日预算: dailyBudget元, 已消耗: todayCost元<br>- 触发告警: alertType — alertSubject<br><br>## 要求<br>1. 分析最可能的 1-3 个原因（从竞争环境、素材衰退、受众饱和、出价策略、时段因素等角度）<br>2. 给出具体的调整建议<br>3. 评估紧急程度: critical / warning / info<br><br>以 JSON 格式输出:<br>{<br>  "reasons": ["原因1", "原因2"],<br>  "suggestions": [<br>    { "action": "具体操作", "expected_effect": "预期效果", "urgency": "critical|warning|info" }<br>  ],<br>  "confidence": 0.0-1.0<br>}<br>`;

// --- prompts/budget_suggestion.txt ---
const BUDGET_SUGGESTION_PROMPT = `<br>## 任务<br>根据投放数据，给出预算优化建议。<br><br>## 账户概况<br>- 账户: accountName<br>- 账户余额: balance元<br>- 活跃计划数: activeCampaigns<br><br>## 各计划数据<br>#each campaigns<br>- name: 日预算=dailyBudget元, 今日消耗=todayCost元, ROI=roi, CPA=cpa元, 健康度=healthScore<br>/each<br><br>## 要求<br>1. 识别预算分配不合理的计划（ROI高但预算低、ROI低但预算高）<br>2. 给出具体的预算调整建议（加到多少/减到多少）<br>3. 计算调整后的预期整体 ROI 变化<br><br>以 JSON 格式输出:<br>{<br>  "analysis": "整体分析",<br>  "adjustments": [<br>    { "campaignName": "...", "currentBudget": 0, "suggestedBudget": 0, "reason": "..." }<br>  ],<br>  "expectedROIChange": "+X%",<br>  "confidence": 0.0-1.0<br>}<br>`;

// --- prompts/bid_suggestion.txt ---
const BID_SUGGESTION_PROMPT = `<br>## 任务<br>分析出价策略，给出优化建议。<br><br>## 计划数据<br>- 计划: campaignName, 出价方式: bidType<br>- 当前出价: currentBid元<br>- 今日: 消耗=cost元, 转化=conversions个, CPA=cpa元<br>- 7日均值: CPA=cpaAvg7d元, CVR=cvrAvg7d%<br>- 同行业参考 CPA 区间: industryCpaRange<br><br>## 要求<br>1. 判断当前出价是否合理<br>2. 如需调整，给出具体出价值和调整幅度<br>3. 说明调整风险（可能导致跑量下降等）<br><br>以 JSON 格式输出:<br>{<br>  "assessment": "当前出价评估",<br>  "suggestion": { "newBid": 0, "changePercent": "+/-X%", "reason": "..." },<br>  "risks": ["风险1"],<br>  "confidence": 0.0-1.0<br>}<br>`;

// --- prompts/creative_suggestion.txt ---
const CREATIVE_SUGGESTION_PROMPT = `<br>## 任务<br>分析素材表现，给出优化建议。<br><br>## 计划下各创意数据<br>#each creatives<br>- name: 曝光=impressions, CTR=ctr%, CVR=cvr%, 审核状态=reviewStatus<br>/each<br><br>## 要求<br>1. 识别表现最好和最差的创意<br>2. 分析 CTR/CVR 差异原因<br>3. 给出新素材方向建议<br><br>以 JSON 格式输出:<br>{<br>  "topCreative": { "name": "...", "strengths": ["..."] },<br>  "weakCreatives": [{ "name": "...", "issues": ["..."] }],<br>  "newDirections": ["建议方向1", "建议方向2"],<br>  "confidence": 0.0-1.0<br>}<br>`;
```

```yaml
# LLM 调用配置
llm_config:
  default_model: "gpt-4o-mini"  # 成本低，速度快
  fallback_model: "gpt-3.5-turbo"
  temperature: 0.3              # 低随机性，保证一致性
  max_tokens: 1024
  timeout_ms: 15000
  retry: 2
  # 无 API Key 时自动降级到 §29 规则引擎
  fallback_to_rules: true
```

---

## 29. 纯规则降级建议库（✅ P1·无 LLM 时使用）

```tsx
// === src-tauri/src/services/rule_suggestions.rs 对应 ===
// 当用户未配置 LLM API Key 时，使用纯规则引擎生成建议

export const RULE_SUGGESTIONS: Record<AlertRuleType, (detail: AlertDetail) => AISuggestion> = {

  budget_overrun: (d) => ({
    type: 'budget_suggestion',
    title: `计划「${d.campaignName}」预算已耗尽`,
    description: `该计划今日消耗已达预算上限 ${d.thresholdValue} 元。`,
    reasons: [
      '当日预算设置偏低，跑量速度超出预期',
      '建议检查是否需要追加预算或调整投放时段',
    ],
    confidence: 0.7,
  }),

  ctr_drop: (d) => ({
    type: 'anomaly_diagnosis',
    title: `计划「${d.campaignName}」点击率下降 ${Math.abs(d.currentValue - d.thresholdValue).toFixed(1)}%`,
    description: `当前 CTR ${d.currentValue.toFixed(2)}%，低于过去7天均值 ${d.thresholdValue.toFixed(2)}%。`,
    reasons: [
      '素材可能出现疲劳衰退，建议更换封面图或视频',
      '竞品可能在相同时段加大投放，挤压曝光',
      '检查定向人群是否过窄导致重复触达',
    ],
    confidence: 0.6,
  }),

  roi_drop: (d) => ({
    type: 'anomaly_diagnosis',
    title: `计划「${d.campaignName}」ROI 低于阈值`,
    description: `当前 ROI ${d.currentValue.toFixed(2)}，低于设定阈值 ${d.thresholdValue.toFixed(2)}。`,
    reasons: [
      'ROI 下降通常与 CPA 上升或客单价下降有关',
      '建议在快手后台检查转化漏斗各环节',
      '如持续低迷，考虑降低出价或暂停该计划',
    ],
    confidence: 0.6,
  }),

  balance_low: (d) => ({
    type: 'budget_suggestion',
    title: `账户「${d.accountName}」余额不足`,
    description: `当前余额 ${d.currentValue.toFixed(0)} 元，低于预警阈值 ${d.thresholdValue.toFixed(0)} 元。`,
    reasons: [
      '余额不足将导致所有计划停投',
      '建议立即在快手后台充值',
    ],
    confidence: 0.9,
  }),

  cpa_spike: (d) => ({
    type: 'bid_suggestion',
    title: `计划「${d.campaignName}」转化成本飙升`,
    description: `当前 CPA ${d.currentValue.toFixed(2)} 元，较7日均值上涨 ${((d.currentValue / d.thresholdValue - 1) * 100).toFixed(0)}%。`,
    reasons: [
      '可能遇到竞价高峰期，同行出价上涨',
      '转化率下降导致单次转化成本增加',
      '建议在快手后台适当降低出价 10-20% 或收窄定向',
    ],
    confidence: 0.6,
  }),

  creative_rejected: (d) => ({
    type: 'creative_suggestion',
    title: `创意审核未通过`,
    description: `账户「${d.accountName}」有创意被拒审。`,
    reasons: [
      '请在快手后台查看具体拒审原因',
      '常见原因：素材含违规内容、夸大宣传、落地页不合规',
      '建议修改素材后重新提交审核',
    ],
    confidence: 0.8,
  }),

  token_expired: (d) => ({
    type: 'health_warning',
    title: `账户「${d.accountName}」授权已失效`,
    description: '无法获取该账户的最新数据，请重新授权。',
    reasons: [
      '快手 OAuth Token 已过期且自动续期失败',
      '请在「广告账户」页面点击「重新授权」',
    ],
    confidence: 1.0,
  }),
};
```

---

## 30. IM 推送完整消息模板（✅ P1）

```rust
// === src-tauri/src/services/push_templates.rs ===
// 各平台推送消息模板，变量用 {var} 占位

/// 告警级别对应 emoji
fn level_emoji(level: &str) -> &str {
    match level {
        "critical" => "🔴",
        "warning"  => "🟡",
        "info"     => "🔵",
        _          => "⚪",
    }
}

// ============================================================
// 企业微信 (Markdown)
// ============================================================
pub fn wecom_alert(e: &AlertEvent, d: &AlertDetail) -> String {
    format!(r#"{{
  "msgtype": "markdown",
  "markdown": {{
    "content": "### {emoji} SSAI告警: {subject}\n> **级别**: {level}\n> **账户**: <font color=\"warning\">{account}</font>\n> **计划**: {campaign}\n> **当前值**: {current} | **阈值**: {threshold}\n> **时间**: {time}\n> ---\n> 💡 **建议**: {suggestion}\n\n[👉 去快手后台处理]({action_url})"
  }}
}}"#,
        emoji = level_emoji(&e.level),
        subject = e.subject,
        level = e.level,
        account = d.account_name,
        campaign = d.campaign_name.as_deref().unwrap_or("-"),
        current = d.current_value,
        threshold = d.threshold_value,
        time = e.triggered_at,
        suggestion = d.suggestion.as_deref().unwrap_or("暂无自动建议"),
        action_url = d.action_url.as_deref().unwrap_or("https://jst.kuaishou.com"),
    )
}

// ============================================================
// 钉钉 (Markdown + @指定人)
// ============================================================
pub fn dingtalk_alert(e: &AlertEvent, d: &AlertDetail, at_mobiles: &[String]) -> String {
    let at_section = if at_mobiles.is_empty() {
        String::new()
    } else {
        format!("\n\n{}", at_mobiles.iter().map(|m| format!("@{}", m)).collect::<Vec<_>>().join(" "))
    };
    format!(r#"{{
  "msgtype": "markdown",
  "markdown": {{
    "title": "{emoji} {subject}",
    "text": "### {emoji} SSAI告警\n\n**{subject}**\n\n| 项目 | 值 |\n|---|---|\n| 级别 | {level} |\n| 账户 | {account} |\n| 计划 | {campaign} |\n| 当前值 | {current} |\n| 阈值 | {threshold} |\n| 时间 | {time} |\n\n---\n\n💡 **建议**: {suggestion}\n\n[👉 去快手后台处理]({action_url}){at}"
  }},
  "at": {{
    "atMobiles": {at_json},
    "isAtAll": false
  }}
}}"#,
        emoji = level_emoji(&e.level),
        subject = e.subject,
        level = e.level,
        account = d.account_name,
        campaign = d.campaign_name.as_deref().unwrap_or("-"),
        current = d.current_value,
        threshold = d.threshold_value,
        time = e.triggered_at,
        suggestion = d.suggestion.as_deref().unwrap_or("暂无自动建议"),
        action_url = d.action_url.as_deref().unwrap_or("https://jst.kuaishou.com"),
        at = at_section,
        at_json = serde_json::to_string(at_mobiles).unwrap_or_default(),
    )
}

// ============================================================
// 飞书 (Interactive Card)
// ============================================================
pub fn feishu_alert(e: &AlertEvent, d: &AlertDetail) -> String {
    let template = match e.level.as_str() {
        "critical" => "red",
        "warning"  => "orange",
        _          => "grey",
    };
    format!(r#"{{
  "msg_type": "interactive",
  "card": {{
    "header": {{
      "title": {{ "tag": "plain_text", "content": "{emoji} SSAI告警: {subject}" }},
      "template": "{template}"
    }},
    "elements": [
      {{ "tag": "div", "text": {{ "tag": "lark_md", "content": "**级别**: {level}\n**账户**: {account}\n**计划**: {campaign}" }} }},
      {{ "tag": "div", "text": {{ "tag": "lark_md", "content": "**当前值**: {current}  |  **阈值**: {threshold}\n**时间**: {time}" }} }},
       "tag": "hr" ,
      {{ "tag": "div", "text": {{ "tag": "lark_md", "content": "💡 **建议**: {suggestion}" }} }},
      {{ "tag": "action", "actions": [{{ "tag": "button", "text":  "tag": "plain_text", "content": "去快手后台处理" , "url": "{action_url}", "type": "primary" }}] }}
    ]
  }}
}}"#,
        emoji = level_emoji(&e.level),
        subject = e.subject,
        template = template,
        level = e.level,
        account = d.account_name,
        campaign = d.campaign_name.as_deref().unwrap_or("-"),
        current = d.current_value,
        threshold = d.threshold_value,
        time = e.triggered_at,
        suggestion = d.suggestion.as_deref().unwrap_or("暂无自动建议"),
        action_url = d.action_url.as_deref().unwrap_or("https://jst.kuaishou.com"),
    )
}

// ============================================================
// Slack (Block Kit)
// ============================================================
pub fn slack_alert(e: &AlertEvent, d: &AlertDetail) -> String {
    format!(r#"{{
  "text": "{emoji} SSAI Alert: {subject}",
  "blocks": [
    {{ "type": "header", "text": {{ "type": "plain_text", "text": "{emoji} {subject}" }} }},
    {{ "type": "section", "fields": [
      {{ "type": "mrkdwn", "text": "*Level:* {level}" }},
      {{ "type": "mrkdwn", "text": "*Account:* {account}" }},
      {{ "type": "mrkdwn", "text": "*Campaign:* {campaign}" }},
      {{ "type": "mrkdwn", "text": "*Current:* {current} | *Threshold:* {threshold}" }}
    ] }},
    {{ "type": "section", "text": {{ "type": "mrkdwn", "text": "💡 *Suggestion:* {suggestion}" }} }},
    {{ "type": "actions", "elements": [{{ "type": "button", "text":  "type": "plain_text", "text": "Open Kuaishou" , "url": "{action_url}", "style": "primary" }}] }}
  ]
}}"#,
        emoji = level_emoji(&e.level),
        subject = e.subject,
        level = e.level,
        account = d.account_name,
        campaign = d.campaign_name.as_deref().unwrap_or("-"),
        current = d.current_value,
        threshold = d.threshold_value,
        suggestion = d.suggestion.as_deref().unwrap_or("No suggestion"),
        action_url = d.action_url.as_deref().unwrap_or("https://jst.kuaishou.com"),
    )
}
```

---

## 31. 推送重试队列（✅ P1）

```rust
// === src-tauri/src/services/push_retry.rs ===
// 策略: SQLite 持久化队列 + 内存定时器
// 选择持久化队列是因为桌面应用可能随时被关闭

use std::time::Duration;
use tokio::time;

/// 重试配置
const MAX_RETRIES: u32 = 5;
const RETRY_DELAYS: [Duration; 5] = [
    Duration::from_secs(10),      // 第1次: 10秒后
    Duration::from_secs(30),      // 第2次: 30秒后
    Duration::from_secs(120),     // 第3次: 2分钟后
    Duration::from_secs(600),     // 第4次: 10分钟后
    Duration::from_secs(3600),    // 第5次: 1小时后
];

/// push_logs 表 status 流转:
/// pending → sent (成功)
/// pending → failed (所有重试耗尽)
/// 中间状态通过 retry_count 和 next_retry_at 追踪

pub struct PushRetryQueue {
    db: Database,
    push_service: PushService,
}

impl PushRetryQueue {
    /// 应用启动时调用: 恢复所有 pending 状态的推送任务
    pub async fn recover_pending(&self) -> Result<()> {
        let pending = self.db.query("
            SELECT id, channel_id, payload_json, retry_count
            FROM push_logs
            WHERE status = 'pending'
              AND next_retry_at <= datetime('now')
            ORDER BY next_retry_at ASC
            LIMIT 50
        ").await?;

        for task in pending {
            self.process_retry(task).await;
        }
        Ok(())
    }

    /// 定时轮询 (每30秒检查一次)
    pub async fn start_poll_loop(&self) {
        let mut interval = time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            if let Err(e) = self.recover_pending().await {
                tracing::error!("Push retry poll error: {}", e);
            }
        }
    }

    /// 处理单条重试
    async fn process_retry(&self, task: PushTask) {
        match self.push_service.send_raw(task.channel_id, &task.payload_json).await {
            Ok(_) => {
                self.db.execute("
                    UPDATE push_logs SET status = 'sent', sent_at = datetime('now')
                    WHERE id = ?1
                ", &[task.id]).await.ok();
            }
            Err(e) => {
                let next_count = task.retry_count + 1;
                if next_count >= MAX_RETRIES as i64 {
                    // 最终失败
                    self.db.execute("
                        UPDATE push_logs
                        SET status = 'failed', error_message = ?2
                        WHERE id = ?1
                    ", &[task.id, &e.to_string()]).await.ok();
                    tracing::warn!("Push permanently failed after {} retries: {}", MAX_RETRIES, e);
                } else {
                    // 计算下次重试时间
                    let delay = RETRY_DELAYS[next_count as usize];
                    self.db.execute("
                        UPDATE push_logs
                        SET retry_count = ?2,
                            next_retry_at = datetime('now', '+' || ?3 || ' seconds'),
                            error_message = ?4
                        WHERE id = ?1
                    ", &[task.id, next_count, delay.as_secs(), &e.to_string()]).await.ok();
                }
            }
        }
    }
}
```

```sql
-- push_logs 表需要新增两列 (加入 v1.0.0 schema)
ALTER TABLE push_logs ADD COLUMN retry_count   INTEGER NOT NULL DEFAULT 0;
ALTER TABLE push_logs ADD COLUMN next_retry_at TEXT;
CREATE INDEX IF NOT EXISTS idx_push_retry ON push_logs(status, next_retry_at);
```

---

## 32. 全局 Loading / Error 处理方案（✅ P1）

```tsx
// === src/components/GlobalErrorBoundary.tsx ===
import { ErrorBoundary, FallbackProps } from 'react-error-boundary';
import { notification } from 'antd';

// --- 1. React Error Boundary (捕获渲染异常) ---
function ErrorFallback({ error, resetErrorBoundary }: FallbackProps) {
  return (
    <div className="error-fallback">
      <div className="error-icon">💥</div>
      <h2>页面渲染出错</h2>
      <p className="error-message">{error.message}</p>
      <button onClick={resetErrorBoundary}>重试</button>
    </div>
  );
}

export function AppErrorBoundary({ children }: { children: React.ReactNode }) {
  return (
    <ErrorBoundary
      FallbackComponent={ErrorFallback}
      onError={(error) => {
        console.error('[ErrorBoundary]', error);
        // 可选: 写入本地日志
      }}
    >
      {children}
    </ErrorBoundary>
  );
}

// --- 2. IPC 错误统一处理 (包装所有 invoke 调用) ---
import { AppError, ErrorCode } from '../types/errors';

const ERROR_MESSAGES: Record<ErrorCode, string> = {
  E1001: '网络请求超时，请检查网络连接',
  E1002: '无法连接到快手服务器',
  E2001: 'API 请求过于频繁，已自动降速',
  E2002: '账户授权已失效，请重新授权',
  E2003: '快手服务器异常，稍后自动重试',
  E2004: 'API 返回数据格式异常',
  E3001: 'Token 刷新失败，请重新授权',
  E3002: 'Token 已过期',
  E3003: '系统密钥链访问失败',
  E4001: 'IM 推送失败，请检查 Webhook 配置',
  E4002: '推送签名验证失败',
  E5001: '数据库写入失败',
  E5002: '数据库迁移失败，请联系开发者',
  E6001: 'AI 服务调用失败',
  E6002: '未配置 LLM API Key，已降级为规则建议',
  E9999: '未知错误',
};

export function handleIPCError(error: unknown): void {
  const appError = parseError(error);
  const message = ERROR_MESSAGES[appError.code] || appError.message;

  // 按严重程度选择通知方式
  switch (appError.code) {
    case ErrorCode.KS_RATE_LIMITED:
    case ErrorCode.LLM_KEY_MISSING:
      // 静默处理，仅在 console 记录
      console.warn(`[${appError.code}] ${message}`);
      break;

    case ErrorCode.TOKEN_EXPIRED:
    case ErrorCode.TOKEN_REFRESH_FAILED:
      // 右上角持久通知 + 跳转
      notification.warning({
        message: '授权失效',
        description: message,
        duration: 0,  // 不自动关闭
        btn: <Button onClick={() => navigate('/accounts')}>去重新授权</Button>,
      });
      break;

    case ErrorCode.DB_MIGRATION_FAILED:
      // 模态弹窗，阻断操作
      Modal.error({
        title: '数据库异常',
        content: message,
        okText: '导出日志',
        onOk: () => settingsCommands.exportDiagnosticLogs(7),
      });
      break;

    default:
      // 右上角 Toast，5秒自动消失
      notification.error({
        message: '操作失败',
        description: message,
        duration: 5,
      });
  }
}

// --- 3. 全局 Loading 状态 ---
// Zustand 全局 store
interface GlobalUIStore {
  /** 全局加载遮罩 (仅用于首次数据加载) */
  initialLoading: boolean;
  setInitialLoading: (v: boolean) => void;

  /** TopBar 进度条 (数据刷新中) */
  refreshing: boolean;
  setRefreshing: (v: boolean) => void;

  /** 当前活跃的 toast 通知数 */
  activeToasts: number;
}

// 使用规范:
// - initialLoading: 仅在应用首次启动、数据库初始化完成前显示全屏骨架屏
// - refreshing: 每次后台数据刷新时，TopBar 显示细进度条 (类似 NProgress)
// - 各页面组件使用自己 Store 的 loading 状态显示局部骨架屏
// - 按钮提交使用 antd Button 的 loading 属性
```

```yaml
# Loading 状态分级
loading_hierarchy:
  level_1_global:
    trigger: "应用启动 / 数据库初始化"
    ui: "全屏骨架屏 + Logo"
    component: "<AppSkeleton />"

  level_2_topbar:
    trigger: "后台数据刷新中"
    ui: "TopBar 下方 2px 橙色进度条"
    component: "<RefreshIndicator />"

  level_3_page:
    trigger: "页面首次加载数据"
    ui: "页面区域内骨架屏"
    component: "antd <Skeleton />"

  level_4_component:
    trigger: "表格/图表加载中"
    ui: "组件内 Spin"
    component: "antd <Spin />"

  level_5_button:
    trigger: "表单提交 / 测试推送"
    ui: "按钮 loading 状态"
    component: "antd <Button loading />"
```

---

## 33. Mock 数据集（✅ P1）

```tsx
// === tests/mocks/kuaishou-api-responses.ts ===
// 开发阶段的 Mock 数据，可配合 MSW (Mock Service Worker) 使用

// --- 场景 1: 正常响应 ---
export const MOCK_REPORT_SUCCESS = {
  code: 0,
  message: 'OK',
  data: {
    list: [
      {
        advertiser_id: '1234567890',
        date: '2024-05-20',
        charge: 5280.50,
        photo_show: 128000,
        photo_click: 4200,
        aclick: 180,
        photo_click_ratio: 0.0328,
        conversion_rate: 0.0429,
        pay_amount: 12650.00,
        roi: 2.40,
      },
    ],
  },
};

export const MOCK_ACCOUNT_INFO = {
  code: 0,
  message: 'OK',
  data: {
    advertiser_id: '1234567890',
    advertiser_name: '测试电商账户A',
    balance: 15680.00,
    daily_budget: 10000.00,
    status: 1,
  },
};

export const MOCK_CAMPAIGN_LIST = {
  code: 0,
  message: 'OK',
  data: {
    list: [
      {
        campaign_id: 'C001',
        campaign_name: '618大促-女装爆款',
        day_budget: 3000.00,
        put_status: 1,
        bid_type: 'OCPM',
      },
      {
        campaign_id: 'C002',
        campaign_name: '日常测品-新款T恤',
        day_budget: 500.00,
        put_status: 1,
        bid_type: 'OCPM',
      },
    ],
  },
};

// --- 场景 2: Token 过期 ---
export const MOCK_TOKEN_EXPIRED = {
  code: 40001,
  message: 'access_token expired',
  data: null,
};

// --- 场景 3: 限流 ---
export const MOCK_RATE_LIMITED = {
  code: 40029,
  message: 'api freq out of limit',
  data: null,
};

// --- 场景 4: 服务端错误 ---
export const MOCK_SERVER_ERROR = {
  code: 50000,
  message: 'internal server error',
  data: null,
};

// --- 场景 5: 字段缺失 (用于测试 FieldMapper 容错) ---
export const MOCK_PARTIAL_RESPONSE = {
  code: 0,
  message: 'OK',
  data: {
    list: [
      {
        advertiser_id: '1234567890',
        date: '2024-05-20',
        charge: 1000.00,
        // 缺少 photo_show, photo_click 等字段
        roi: 1.5,
      },
    ],
  },
};

// --- 场景 6: OAuth 回调 ---
export const MOCK_OAUTH_TOKEN_SUCCESS = {
  access_token: 'mock_access_token_abc123',
  refresh_token: 'mock_refresh_token_xyz789',
  expires_in: 7200,
  refresh_token_expires_in: 2592000,
  token_type: 'Bearer',
};

export const MOCK_OAUTH_TOKEN_INVALID_CODE = {
  error: 'invalid_grant',
  error_description: 'authorization code is expired or invalid',
};

// --- 场景 7: Webhook 推送响应 ---
export const MOCK_WECOM_SUCCESS  = { errcode: 0, errmsg: 'ok' };
export const MOCK_WECOM_FAIL     = { errcode: 93000, errmsg: 'invalid webhook url' };
export const MOCK_DINGTALK_SUCCESS = { errcode: 0, errmsg: 'ok' };
export const MOCK_FEISHU_SUCCESS   = { code: 0, msg: 'success' };
export const MOCK_SLACK_SUCCESS    = { ok: true };
export const MOCK_SLACK_FAIL       = { ok: false, error: 'channel_not_found' };
```

```yaml
# Mock 使用方式
mock_setup:
  dev_mode:
    tool: "MSW (Mock Service Worker)"
    config: "src/mocks/browser.ts"
    enable: "VITE_MOCK=true pnpm dev"
    description: "前端开发时拦截 Tauri IPC，返回 Mock 数据"

  rust_tests:
    tool: "mockall crate"
    trait: "KuaishouApiClient (trait object)"
    description: "Rust 单元测试中 mock API 客户端"

  e2e_tests:
    tool: "Playwright + MSW"
    description: "E2E 测试中用 Mock 数据驱动完整流程"
```

---

## 34. [README.md](http://README.md) 初版（✅ P1）

```markdown
# 📊 SSAI-快手监控

> 纯数据监控 + 智能预警桌面工具。聚合快手磁力金牛/磁力引擎广告数据，只读不操作。

## ✨ 核心功能

- 📈 **数据概览** — 5大核心指标卡 + 趋势图 + 渠道分布 + 散点图
- 🎯 **投放监控** — 计划/单元/创意三级联动表格，实时刷新
- 🚨 **智能告警** — 7种预设规则 + 自定义阈值，实时检测
- 🔔 **IM 推送** — 企业微信/钉钉/飞书/Slack 一键配置
- 🧠 **AI 建议** — 异常归因 + 预算/出价/素材优化建议
- 🔒 **混合架构** — 桌面客户端 + 云端中转服务端；App Secret 仅在服务端 `.env`；客户端本地 SQLite 仅缓存 7 天数据
```

---

## 35. 云端中转架构总览（Hybrid Architecture）

<aside>
🏗️

架构决策已定稿：方案 A（桌面+云端中转）+ 方案 B（服务端承担 OAuth + 定时拉数据 + IM 推送）+ 方案 A（PG 权威源，客户端 SQLite 7 天缓存）。

</aside>

### 35.1 组件拓扑

```mermaid
graph LR
  A["桌面客户端<br/>Tauri Win/macOS"] -->|HTTPS/WSS| B["Nginx<br/>kuaishou.ssaitool.cn"]
  B --> C["SSAI Server<br/>Rust Axum (systemd)"]
  C --> D[("PostgreSQL 16<br/>权威数据 + 加密Token")]
  C -->|REST POST| E["快手磁力金牛<br/>ad.e.kuaishou.com"]
  C -->|Webhook| F["企微/钉钉/飞书/Slack"]
  A -.->|本地| G[("SQLite<br/>7d 缓存 + 本地告警")]
```

### 35.2 职责划分

| 职责 | 客户端 | 服务端 |
| --- | --- | --- |
| App Secret 保管 | ❌ | ✅ (.env/KMS) |
| Token 加密存储 | ❌ | ✅ (AES-GCM + PG) |
| 定时拉快手 API | — | ✅ (tokio-cron) |
| IM Webhook 推送 | — | ✅ (失败持久化重试) |
| 本地缓存 7 天 | ✅ SQLite | — |
| 推送配置管理 | UI 编辑 | ✅ 持久化 |

### 35.3 OAuth 授权时序

```mermaid
sequenceDiagram
  participant U as 用户
  participant C as 客户端
  participant S as SSAI Server
  participant K as 快手磁力金牛

  U->>C: 点击「绑定账户」
  C->>S: POST /api/auth/begin-oauth (Bearer JWT)
  S->>S: 生成 state + nonce, 写 oauth_state 表 (TTL 10min)
  S-->>C: { authorize_url }
  C->>U: 浏览器打开 authorize_url
  U->>K: 登录 + 授权
  K->>S: GET /api/kuaishou/oauth/callback?code=&state=
  S->>S: 校验 state 未过期且未使用
  S->>K: POST /rest/openapi/oauth2/authorize/access_token
  K-->>S: { access_token, refresh_token, expires_in }
  S->>K: POST /rest/openapi/v1/advertiser/list
  K-->>S: { advertiser_id[], advertiser_name[] }
  S->>S: AES-GCM 加密 token + 落盘 ks_accounts
  S-->>U: HTML「授权成功，可关闭此页」
  S->>C: WS push: { type: "accounts_updated" }
  C->>S: GET /api/accounts
  S-->>C: Account[]
```

### 35.4 数据同步策略

- **服务端拉取**：按 §9 调度器 POST 磁力金牛 API → 字段映射 → 写 PG → 广播 WS
- **客户端订阅**：WSS `/api/ws` 长连接；收到 `metric_updated` / `new_alert` 后更新本地 SQLite
- **按需查询**：历史超 7 天时 `GET /api/metrics?range=30d` 从服务端拉，不缓存
- **断线兜底**：WS 断开后 30s 指数回退重连；重连后 `GET /api/sync?since={last_ts}` 补齐
- **告警双端**：服务端触发 IM 推送（始终在线）；客户端开启时本地规则并行跑，用于 UI 即时提示
- **客户端缓存清理**：每次启动 + 每天 04:00 删除 `metric_snapshots WHERE snapshot_at < now() - 7 days`

---

## 36. 磁力金牛 (ESP) API 接口清单（服务端专用）

<aside>
📡

调用规范：POST application/json，Header <code>Access-Token: {token}</code>。Base URL <code>[https://ad.e.kuaishou.com/rest/openapi</code>。所有路径均为服务端调用，客户端不直接访问快手](https://ad.e.kuaishou.com/rest/openapi</code>。所有路径均为服务端调用，客户端不直接访问快手) API。

</aside>

### 36.1 基础认证接口（磁力金牛 / 磁力引擎 共用）

| 用途 | 路径 | 关键参数 | 核心响应字段 |
| --- | --- | --- | --- |
| 刷新 Token | /oauth2/authorize/refresh_token | app_id, secret, refresh_token | 同上（全新一对） |
| 账户余额 | /v1/advertiser/fund/get | advertiser_id | balance, recharge_balance, contract_rebate, direct_rebate |

### 36.2 磁力金牛专属报表（ESP 独有）

| 用途 | 路径（待实测校准） | 关键参数 | 聚合维度 |
| --- | --- | --- | --- |
| 直播报表 | /v2/esp/live/report | advertiser_id, live_id?, date_range | live_id 分场 |
| 粉丝报表 | /v2/esp/fans/report | advertiser_id, date_range | fans_add, fans_total, cost_per_fan |
| 选品推荐 | /v2/esp/goods/recommend | advertiser_id | 推荐商品列表 |

### 36.3 通用广告报表（DSP/ESP 均支持）

| 用途 | 路径 | 聚合粒度 | 单元报表 | /v1/report/unit_report | unit_id / day / hour |
| --- | --- | --- | --- | --- | --- |
| 创意报表 | /v1/report/creative_report | creative_id | eCPM 报表 | /gw/dsp/v1/report/ecpm_report | account / unit |

<aside>
⚠️

§36.2 的路径是根据磁力金牛开放平台公开文档和第三方集成资料整理的最常见命名。
开发第一步请用 APP_ID=165926585 在 <a href="[https://developers.e.kuaishou.com/tools/api-debug">开放平台](https://developers.e.kuaishou.com/tools/api-debug">开放平台) API 调试工具</a> 逐一试跑，拿到真实请求/响应样本后：
1. 修正路径（如平台用 v3 或别名）
2. 填入 <code>public/field-mapping.json</code>（§8）
3. 回填本章「待实测校准」列。

</aside>

### 36.4 服务端 API 客户端（Rust 示例）

```rust
// server/src/kuaishou/client.rs
use reqwest::Client;
use serde::{Deserialize, de::DeserializeOwned};

#[derive(Deserialize)]
struct ApiEnvelope<T> {
    code: i64,
    message: String,
    data: Option<T>,
    request_id: Option<String>,
}

pub struct KuaishouClient {
    http: Client,
    base_url: String, // https://ad.e.kuaishou.com/rest/openapi
}

impl KuaishouClient {
    pub async fn call<T, B>(&self, path: &str, token: &str, body: &B) -> Result<T, KsError>
    where T: DeserializeOwned, B: serde::Serialize + ?Sized {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.http
            .post(&url)
            .header("Access-Token", token)
            .header("Content-Type", "application/json")
            .json(body)
            .send().await?
            .error_for_status()?;
        let env: ApiEnvelope<T> = resp.json().await?;
        match env.code {
            0 => env.data.ok_or(KsError::EmptyData),
            40001 => Err(KsError::TokenExpired),
            40029 => Err(KsError::RateLimited),
            50000 => Err(KsError::UpstreamServer(env.message)),
            c => Err(KsError::Business(c, env.message)),
        }
    }
}
```

---

## 37. 服务端技术栈 + PostgreSQL Schema

### 37.1 技术选型

<aside>
🦀

推荐 Rust Axum：与客户端 Tauri 同语言、单二进制部署、冷启 ~150MB 内存，2 核 2G 安全运行。
备选 Node.js Fastify：开发速度快 ~1.5×，但多一个 Node 运行时 ~120MB 常驻内存。
v1 默认 Rust；若团队 JS 栈更熟可切 Node，本文 DDL 与 API 契约皆语言无关。

</aside>

### 37.2 Cargo.toml（核心依赖）

```toml
# server/Cargo.toml
[package]
name = "ssai-server"
version = "1.0.0"
edition = "2021"
rust-version = "1.75"

[dependencies]
axum = { version = "0.7", features = ["ws", "macros"] }
tokio = { version = "1", features = ["full"] }
tower = "0.5"
tower-http = { version = "0.5", features = ["cors", "trace", "compression-gzip", "timeout"] }
hyper = "1"
sqlx = { version = "0.8", features = ["runtime-tokio-rustls", "postgres", "chrono", "uuid", "json", "macros"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
jsonwebtoken = "9"
argon2 = "0.5"
aes-gcm = "0.10"
rand = "0.8"
hex = "0.4"
tokio-cron-scheduler = "0.10"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4", "serde"] }
thiserror = "1"
anyhow = "1"
dotenvy = "0.15"

[profile.release]
lto = true
codegen-units = 1
strip = true
```

### 37.3 目录结构

```
server/
├── Cargo.toml
├── .env.example                    # 见 §40.1
├── migrations/                     # sqlx migrate add
│   ├── 20260401000001_init.sql     # §37.4 内容
│   └── ...
└── src/
    ├── main.rs                     # Axum app 构建 + systemd-ready
    ├── config.rs                   # env 加载与校验
    ├── state.rs                    # AppState (PgPool, aead, jwt_cfg, ks_client)
    ├── error.rs                    # IntoResponse 统一错误 (§39.3 信封)
    ├── auth/
    │   ├── mod.rs
    │   ├── login.rs                # argon2 验证 + JWT 签发
    │   ├── jwt.rs
    │   └── middleware.rs           # Bearer 校验中间件
    ├── kuaishou/
    │   ├── mod.rs
    │   ├── client.rs               # §36.4 KuaishouClient
    │   ├── oauth.rs                # begin_oauth + callback
    │   ├── endpoints.rs            # 各报表 typed 封装
    │   └── token_store.rs          # 加密 Token CRUD
    ├── scheduler/
    │   ├── mod.rs                  # tokio-cron-scheduler bootstrap
    │   ├── realtime_poll.rs        # 30s/300s 轮询
    │   ├── daily_report.rs         # 02:00 日报
    │   ├── token_refresh.rs        # 每 60s，提前 1h 刷
    │   └── aggregate.rs            # 分钟→小时→日 聚合（§26 PG 版本）
    ├── alert/
    │   ├── engine.rs               # 复用 §10 规则
    │   └── history.rs
    ├── push/
    │   ├── mod.rs
    │   ├── templates.rs            # 复用 §30
    │   ├── wecom.rs | dingtalk.rs | feishu.rs | slack.rs
    │   └── retry_queue.rs          # PG 持久化队列
    ├── api/
    │   ├── mod.rs                  # Router::new() 汇总
    │   ├── auth_api.rs             # /api/auth/*
    │   ├── accounts_api.rs         # /api/accounts*
    │   ├── metrics_api.rs          # /api/metrics/*
    │   ├── alerts_api.rs           # /api/alerts* + /api/alert-rules*
    │   ├── push_api.rs             # /api/push/*
    │   ├── oauth_api.rs            # /api/auth/begin-oauth + callback
    │   ├── sync_api.rs             # /api/sync
    │   └── health_api.rs           # /health, /ready, /metrics(prom)
    ├── ws/
    │   ├── hub.rs                  # 广播中心 (broadcast channel)
    │   └── connection.rs           # 单连接状态机
    ├── crypto/
    │   └── aead.rs                 # §40.3 AES-256-GCM
    └── util/
        ├── sanitize.rs             # §24 脱敏复用
        └── time.rs
```

### 37.4 PostgreSQL 建表（完整 migration）

```sql
-- migrations/20260401000001_init.sql
CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- === 1. 系统用户 ===
CREATE TABLE users (
  id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  email         TEXT UNIQUE NOT NULL,
  password_hash TEXT NOT NULL,                 -- argon2id
  totp_secret   TEXT,                          -- 可选 2FA，Base32 加密
  role          TEXT NOT NULL DEFAULT 'user',  -- user | admin
  disabled      BOOLEAN NOT NULL DEFAULT false,
  created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
  last_login_at TIMESTAMPTZ
);

-- === 2. OAuth state 防 CSRF ===
CREATE TABLE oauth_state (
  state       TEXT PRIMARY KEY,
  user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  oauth_type  TEXT NOT NULL DEFAULT 'advertiser',
  created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
  used_at     TIMESTAMPTZ,
  expires_at  TIMESTAMPTZ NOT NULL DEFAULT (now() + interval '10 minute')
);
CREATE INDEX ON oauth_state(expires_at);

-- === 3. 快手广告主账户 + 加密 Token ===
CREATE TABLE ks_accounts (
  id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id             UUID NOT NULL REFERENCES users(id),
  advertiser_id       TEXT NOT NULL,
  advertiser_name     TEXT NOT NULL,
  group_tag           TEXT,
  access_token_enc    BYTEA NOT NULL,          -- [nonce(12) || AES-GCM ciphertext]
  refresh_token_enc   BYTEA NOT NULL,
  access_expires_at   TIMESTAMPTZ NOT NULL,
  refresh_expires_at  TIMESTAMPTZ NOT NULL,
  balance             NUMERIC(14,2) DEFAULT 0,
  daily_budget        NUMERIC(14,2) DEFAULT 0,
  status              TEXT NOT NULL DEFAULT 'normal',
  last_refreshed_at   TIMESTAMPTZ,
  created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (user_id, advertiser_id)
);
CREATE INDEX ON ks_accounts(access_expires_at);
CREATE INDEX ON ks_accounts(user_id);

-- === 4. 指标快照（时序）===
CREATE TABLE metric_snapshots (
  id           BIGSERIAL PRIMARY KEY,
  account_id   UUID NOT NULL REFERENCES ks_accounts(id) ON DELETE CASCADE,
  level        TEXT NOT NULL,                   -- account|campaign|unit|creative|goods|live|photo
  ref_id       TEXT NOT NULL,
  snapshot_at  TIMESTAMPTZ NOT NULL,
  cost         NUMERIC(14,2) DEFAULT 0,
  impressions  BIGINT DEFAULT 0,
  clicks       BIGINT DEFAULT 0,
  conversions  BIGINT DEFAULT 0,
  revenue      NUMERIC(14,2) DEFAULT 0,
  roi          NUMERIC(8,4) DEFAULT 0,
  ctr          NUMERIC(6,4) DEFAULT 0,
  cvr          NUMERIC(6,4) DEFAULT 0,
  cpa          NUMERIC(10,2) DEFAULT 0,
  raw          JSONB                            -- 保留快手原始响应，便于排错
);
CREATE INDEX ON metric_snapshots(account_id, level, ref_id, snapshot_at DESC);
CREATE INDEX ON metric_snapshots(snapshot_at);
-- 可选：按月分区 CREATE TABLE metric_snapshots_2026_04 PARTITION OF ...

-- === 5. 告警规则 / 事件 ===
CREATE TABLE alert_rules (
  id                BIGSERIAL PRIMARY KEY,
  user_id           UUID NOT NULL REFERENCES users(id),
  rule_type         TEXT NOT NULL,
  scope_level       TEXT NOT NULL DEFAULT 'global',
  scope_ref         TEXT,
  threshold         JSONB NOT NULL,
  enabled           BOOLEAN NOT NULL DEFAULT true,
  quiet_hours       TEXT,
  min_interval_sec  INT NOT NULL DEFAULT 1800,
  created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX ON alert_rules(user_id) WHERE enabled = true;

CREATE TABLE alert_events (
  id            BIGSERIAL PRIMARY KEY,
  rule_id       BIGINT NOT NULL REFERENCES alert_rules(id) ON DELETE CASCADE,
  user_id       UUID NOT NULL REFERENCES users(id),
  account_id    UUID REFERENCES ks_accounts(id) ON DELETE SET NULL,
  level         TEXT NOT NULL,                   -- critical|warning|info
  subject       TEXT NOT NULL,
  detail        JSONB NOT NULL,
  triggered_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
  handled       BOOLEAN NOT NULL DEFAULT false,
  handled_at    TIMESTAMPTZ
);
CREATE INDEX ON alert_events(user_id, triggered_at DESC);
CREATE INDEX ON alert_events(user_id, handled) WHERE handled = false;

-- === 6. 推送渠道 + 日志（含重试队列）===
CREATE TABLE push_channels (
  id                 BIGSERIAL PRIMARY KEY,
  user_id            UUID NOT NULL REFERENCES users(id),
  provider           TEXT NOT NULL,             -- wecom|dingtalk|feishu|slack
  name               TEXT NOT NULL,
  webhook_url_enc    BYTEA NOT NULL,
  sign_secret_enc    BYTEA,
  at_mobiles         JSONB,                     -- 钉钉 @
  enabled            BOOLEAN NOT NULL DEFAULT true,
  created_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE push_logs (
  id              BIGSERIAL PRIMARY KEY,
  channel_id      BIGINT NOT NULL REFERENCES push_channels(id) ON DELETE CASCADE,
  alert_event_id  BIGINT REFERENCES alert_events(id) ON DELETE SET NULL,
  payload         JSONB NOT NULL,
  status          TEXT NOT NULL,                -- pending|sent|failed
  retry_count     INT NOT NULL DEFAULT 0,
  next_retry_at   TIMESTAMPTZ,
  error_message   TEXT,
  sent_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX ON push_logs(status, next_retry_at) WHERE status = 'pending';
CREATE INDEX ON push_logs(channel_id, sent_at DESC);

-- === 7. 数据保留 ===
-- metric_snapshots 按 §15 规则：日级永久；
-- 分钟级 > 90 天 聚合到小时级 → 删除源；
-- 小时级 > 365 天 聚合到日级 → 删除源。
-- 由 server/src/scheduler/aggregate.rs 每日 04:00 执行。
--
-- push_logs 保留 180 天：
-- DELETE FROM push_logs WHERE sent_at < now() - interval '180 days';
--
-- oauth_state 过期立即删：
-- DELETE FROM oauth_state WHERE expires_at < now();
```

<!-- MARKER:END-OF-§37 -->

---

## 38. Ubuntu 24.04 部署方案（阿里云 2核 2G）

<aside>
🚀

公网 IP：8.162.4.113 · 根域名：[ssaitool.cn](http://ssaitool.cn) · 回调子域名：[kuaishou.ssaitool.cn](http://kuaishou.ssaitool.cn)
目标：Nginx 反代 + Let's Encrypt HTTPS + systemd 守护 + PostgreSQL 16 + 每日自动备份

</aside>

### 38.1 域名 DNS 解析

在域名服务商控制台添加 A 记录：

| 记录类型 | 主机记录 | 解析值 | TTL | A | kuaishou | 8.162.4.113 | 600 |
| --- | --- | --- | --- | --- | --- | --- | --- |

执行 `dig kuaishou.ssaitool.cn +short` 确认解析生效（一般 1-10 分钟）。

### 38.2 阿里云安全组入方向规则

| 协议 | 端口 | 授权对象 | 用途 | TCP | 22 | 你的公网 IP/32 | SSH（只对你开放，不要 0.0.0.0/0） |
| --- | --- | --- | --- | --- | --- | --- | --- |
| TCP | 80 | 0.0.0.0/0 | HTTP（Certbot ACME 验证 + 跳转 HTTPS） | TCP | 443 | 0.0.0.0/0 | HTTPS |
| TCP | 5432 | 关闭 | PostgreSQL 仅 127.0.0.1 本机访问 | TCP | 8080 | 关闭 | ssai-server 仅 127.0.0.1 绑定，对外通过 Nginx |

### 38.3 一次性系统初始化

```bash
# 用 root 或 sudo 用户执行

# === 01. 系统更新与基础包 ===
apt update && apt upgrade -y
apt install -y curl wget git build-essential pkg-config libssl-dev ca-certificates \
               nginx certbot python3-certbot-nginx ufw \
               postgresql-16 postgresql-contrib \
               fail2ban unattended-upgrades logrotate jq

# === 02. 时区与 NTP ===
timedatectl set-timezone Asia/Shanghai
systemctl enable --now systemd-timesyncd

# === 03. 防火墙（阿里云安全组是第一层，ufw 是第二层、系统级防护）===
ufw allow 22/tcp
ufw allow 80/tcp
ufw allow 443/tcp
ufw --force enable
ufw status verbose

# === 04. fail2ban（SSH 和登录防暴破）===
systemctl enable --now fail2ban

# === 05. 专用服务用户与目录 ===
useradd -r -m -d /opt/ssai -s /bin/bash ssai
mkdir -p /opt/ssai/{bin,data,logs,config,backup}
chown -R ssai:ssai /opt/ssai
chmod 750 /opt/ssai

# === 06. PostgreSQL（只听 127.0.0.1）===
sudo -u postgres psql <<'EOF'
-- 请将 REPLACE_STRONG 改为强随机密码 (openssl rand -base64 24)
CREATE USER ssai_app WITH PASSWORD 'REPLACE_STRONG';
CREATE DATABASE ssai OWNER ssai_app;
\c ssai
CREATE EXTENSION IF NOT EXISTS pgcrypto;
EOF
# 确认 /etc/postgresql/16/main/postgresql.conf 中：listen_addresses = 'localhost'
sed -i "s/^#\?listen_addresses.*/listen_addresses = 'localhost'/" /etc/postgresql/16/main/postgresql.conf
systemctl enable --now postgresql
systemctl restart postgresql

# === 07. 自动安全更新 ===
dpkg-reconfigure -plow unattended-upgrades

# === 08. 系统 swap（2G 内存建议加 1G swap）===
fallocate -l 1G /swapfile
chmod 600 /swapfile
mkswap /swapfile
swapon /swapfile
echo '/swapfile none swap sw 0 0' >> /etc/fstab
```

### 38.4 Nginx 反向代理配置

```
# /etc/nginx/sites-available/ssai
server {
    listen 80;
    server_name kuaishou.ssaitool.cn;
    # Certbot ACME 验证
    location /.well-known/acme-challenge/ { root /var/www/certbot; }
    # 其余跳转 HTTPS
    location / { return 301 https://$host$request_uri; }
}

server {
    listen 443 ssl http2;
    server_name kuaishou.ssaitool.cn;

    ssl_certificate     /etc/letsencrypt/live/kuaishou.ssaitool.cn/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/kuaishou.ssaitool.cn/privkey.pem;
    ssl_protocols       TLSv1.2 TLSv1.3;
    ssl_ciphers         HIGH:!aNULL:!MD5;
    ssl_session_cache   shared:SSL:10m;
    ssl_session_timeout 10m;

    # 安全响应头
    add_header Strict-Transport-Security "max-age=63072000; includeSubDomains" always;
    add_header X-Frame-Options DENY always;
    add_header X-Content-Type-Options nosniff always;
    add_header Referrer-Policy no-referrer always;
    add_header Permissions-Policy "geolocation=(),microphone=(),camera=()" always;

    client_max_body_size 10m;
    gzip on;
    gzip_types application/json text/plain text/css application/javascript;

    # 健康检查 (外部监控用)
    location = /health {
        proxy_pass http://127.0.0.1:8080/health;
        access_log off;
    }

    # WebSocket（客户端订阅）
    location /api/ws {
        proxy_pass http://127.0.0.1:8080;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_read_timeout 3600s;
        proxy_send_timeout 3600s;
    }

    # REST API + OAuth callback
    location /api/ {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto https;
        proxy_read_timeout 60s;
    }

    # OAuth 回调后的成功/失败 HTML 页（可选：静态托管给服务端）
    location / { proxy_pass http://127.0.0.1:8080; }
}
```

```bash
ln -s /etc/nginx/sites-available/ssai /etc/nginx/sites-enabled/
rm -f /etc/nginx/sites-enabled/default
nginx -t && systemctl reload nginx

# HTTPS 证书申请（Certbot 自动修改 Nginx 并安装定时续期）
certbot --nginx -d kuaishou.ssaitool.cn --redirect \
        -m 9710078@qq.com --agree-tos --no-eff-email -n
# 验证自动续期定时器已装
systemctl list-timers | grep certbot
certbot renew --dry-run
```

### 38.5 systemd 服务单元

```
# /etc/systemd/system/ssai-server.service
[Unit]
Description=SSAI Kuaishou Monitor Server
After=network-online.target postgresql.service
Wants=network-online.target postgresql.service

[Service]
Type=simple
User=ssai
Group=ssai
WorkingDirectory=/opt/ssai
EnvironmentFile=/opt/ssai/config/.env
ExecStart=/opt/ssai/bin/ssai-server
Restart=on-failure
RestartSec=5
StartLimitIntervalSec=60
StartLimitBurst=5

# 资源限制（2核2G 安全范围）
MemoryMax=1G
CPUQuota=150%
LimitNOFILE=65535

# 安全加固
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/opt/ssai/data /opt/ssai/logs /opt/ssai/backup
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
RestrictNamespaces=true
LockPersonality=true

# 日志（优先交给 journald）
StandardOutput=journal
StandardError=journal
SyslogIdentifier=ssai-server

[Install]
WantedBy=multi-user.target
```

```bash
systemctl daemon-reload
systemctl enable --now ssai-server
systemctl status ssai-server
journalctl -u ssai-server -f
```

### 38.6 每次发版部署脚本

```bash
# 本地或 CI 构建：
cd server
cargo build --release --locked
scp target/release/ssai-server root@8.162.4.113:/tmp/
scp migrations/*.sql root@8.162.4.113:/tmp/migrations/ 2>/dev/null || true

# /opt/ssai/bin/deploy.sh（服务器上）
#!/usr/bin/env bash
set -euo pipefail
systemctl stop ssai-server
install -o ssai -g ssai -m 750 /tmp/ssai-server /opt/ssai/bin/ssai-server
if [ -d /tmp/migrations ] && [ "$(ls -A /tmp/migrations 2>/dev/null)" ]; then
  sudo -u ssai -H bash -lc \
    'cd /opt/ssai && sqlx migrate run --source /tmp/migrations --database-url "$(grep ^DATABASE_URL /opt/ssai/config/.env | cut -d= -f2-)"'
fi
systemctl start ssai-server
sleep 3
curl -fsS https://kuaishou.ssaitool.cn/health || { echo "HEALTHCHECK FAILED"; exit 1; }
echo "✅ Deploy OK"
```

---

## 39. 客户端 ↔ 服务端 通信协议

### 39.1 REST API 路径全集

| 方法 | 路径 | 说明 | 认证 | POST | /api/auth/login | 邮箱+密码 → 签发 JWT | 公开 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| POST | /api/auth/refresh | JWT 续期 | 旧 JWT | POST | /api/auth/logout | 注销（加入黑名单直到 exp） | Bearer |
| GET | /api/me | 当前用户信息 | Bearer | POST | /api/auth/begin-oauth | 生成快手授权 URL，写 oauth_state | Bearer |
| GET | /api/kuaishou/oauth/callback | 快手回调（浏览器 GET） | 公开 (state 校验) | GET | /api/accounts | 列出已绑定快手账户 | Bearer |
| PATCH | /api/accounts/{id} | 更新 group_tag | Bearer | DELETE | /api/accounts/{id} | 解绑（软删 + 清 token） | Bearer |
| POST | /api/accounts/{id}/refresh-token | 手动刷 Token | Bearer | GET | /api/metrics/dashboard | 概览 5 卡片 | Bearer |
| GET | /api/metrics/trend?range=1d|7d|30d | 趋势图 | Bearer | GET | /api/metrics/channels | 渠道分布 | Bearer |
| GET | /api/metrics/campaigns?filter=… | 计划列表 | Bearer | GET | /api/metrics/scatter?type=cost|roi | 散点图 | Bearer |
| GET | /api/sync?since={iso} | 断线补数据 | Bearer | GET | /api/alerts?… | 告警分页 | Bearer |
| PATCH | /api/alerts/{id}/handle | 标记已处理 | Bearer | GET/POST/PUT/DELETE | /api/alert-rules[/{id}] | 规则 CRUD | Bearer |
| GET/POST/PUT/DELETE | /api/push/channels[/{id}] | 推送渠道 CRUD | Bearer | POST | /api/push/channels/{id}/test | 发测试消息 | Bearer |
| GET | /api/push/logs | 推送记录 | Bearer | GET | /health | 存活探针 | 公开 |
| GET | /ready | DB 可用性 | 公开 | WS | /api/ws | 长连接订阅 | 首帧 auth |

### 39.2 JWT 规范

```yaml
header:
  alg: HS256
  typ: JWT
payload:
  sub: "{user_id_uuid}"
  email: "9710078@qq.com"
  role: "user"
  iat: <unix_ts>
  exp: <unix_ts + 7200>              # 2h
  jti: "{random}"                    # 用于黑名单精确吊销
signing_key: env JWT_SECRET (>= 32 bytes random hex)
refresh_policy:
  客户端检测 (exp - now) < 10min 自动调 /api/auth/refresh。
  旧 JWT 可继续使用到 exp；新 JWT 返回后客户端立即替换。
revoke:
  登出 → jti 加入 Redis/PG 黑名单（TTL = exp - now）。
  v1 简单实现：PG 表 jwt_blacklist(jti PRIMARY KEY, exp_at TIMESTAMPTZ)。
```

### 39.3 统一响应信封

```json
// 成功
{ "ok": true, "data": { ... } }

// 失败
{ "ok": false, "error": { "code": "E2002", "message": "账户授权已失效" } }
```

错误码复用 §16 定义；服务端独有错误码才扩展（如 E7001 USER_DISABLED、E7002 JWT_INVALID、E7003 JWT_REVOKED、E7004 OAUTH_STATE_INVALID）。

### 39.4 WebSocket 帧协议

```tsx
// === 客户端 → 服务端 ===
{ "type": "auth", "token": "<jwt>" }            // 首帧；5s 内必须发否则连接关闭
{ "type": "ping" }                               // 对应服务端 heartbeat
{ "type": "subscribe", "topics": ["metrics","alerts"] }   // 可选频道筛选

// === 服务端 → 客户端 ===
{ "type": "auth_ok", "user_id": "..." }
{ "type": "metric_updated", "account_id": "...", "level": "campaign",
  "ref_id": "C001", "at": "2026-04-24T11:30:00Z" }
{ "type": "new_alert", "event": AlertEvent }
{ "type": "accounts_updated" }
{ "type": "token_status_changed", "account_id": "...", "status": "expiring" }
{ "type": "heartbeat", "at": "..." }             // 每 30s
{ "type": "error", "code": "E7002", "message": "..." }
```

客户端重连策略：断线后 2s → 5s → 10s → 30s → 60s 指数回退，每次重连成功后立即 `GET /api/sync?since={last_ts}` 补齐。

### 39.5 客户端 SQLite 缓存规则

| 资源 | 服务端→本地 | 保留时长 | 触发时机 | metric_snapshots | WS push + 启动拉近7d | 滚动 7 天，超过删除 | WS 事件 + 断线 GET /api/sync |
| --- | --- | --- | --- | --- | --- | --- | --- |
| alert_events | WS push + 登录后拉全量 | 永久（直到用户手动清理） | WS 事件 + 登录 | ks_accounts | 按需拉全量 | 无 TTL（覆盖写） | accounts_updated 事件 |
| alert_rules | 双向同步（服务端为准） | 全量 | 编辑时 PUT 服务端后拉回写本地 | push_channels | 仅元数据，webhook 加密体不下发 | 全量 | CRUD 后重拉 |

客户端本地 SQLite schema 基本维持 §5 不变，仅：

- 删除本地 Keychain Token 相关字段（accounts.token_ref 保留但永远 NULL）
- 新增表 `server_sync(resource TEXT PRIMARY KEY, last_sync_at TEXT)` 记录每资源的最后同步时间
- 新增表 `auth(jwt TEXT, expires_at TEXT, user_json TEXT)` 仅存当前 JWT（OS Keychain 更佳，两者二选一）

---

## 40. 凭据与密钥管理

### 40.1 服务端 `.env` 模板

```bash
# /opt/ssai/config/.env   (chmod 600, chown ssai:ssai)

# === PostgreSQL ===
DATABASE_URL=postgres://ssai_app:REPLACE_STRONG@127.0.0.1:5432/ssai
PG_MAX_CONN=10

# === HTTP Server ===
BIND_ADDR=127.0.0.1:8080
PUBLIC_ORIGIN=https://kuaishou.ssaitool.cn
CORS_ORIGINS=tauri://localhost,https://tauri.localhost

# === JWT ===
JWT_SECRET=REPLACE_64_HEX_RANDOM        # openssl rand -hex 32
JWT_TTL_SEC=7200

# === AES-GCM Token 加密主密钥 ===
TOKEN_ENCRYPTION_KEY=REPLACE_64_HEX_RANDOM   # openssl rand -hex 32
TOKEN_ENCRYPTION_KEY_VERSION=1
# 轮换步骤见 §40.4；轮换期支持 v1+v2 同时解密

# === 快手开放平台凭据 ===
KS_APP_ID=165926585
KS_APP_SECRET=REPLACE_AFTER_ROTATE           # 授权前先在快手平台重置
KS_REDIRECT_URI=https://kuaishou.ssaitool.cn/api/kuaishou/oauth/callback
KS_API_BASE=https://ad.e.kuaishou.com/rest/openapi
KS_OAUTH_TYPE=advertiser
KS_SCOPES=report_service,account_service

# === 限流 ===
KS_GLOBAL_QPS=8
KS_DAILY_QUOTA_BUFFER=0.9
KS_REQUEST_TIMEOUT_MS=10000

# === 日志 ===
RUST_LOG=ssai_server=info,tower_http=warn,sqlx=warn
LOG_FORMAT=json                              # json | pretty

# === 可选：阿里云 KMS（生产建议）===
# ALIYUN_KMS_KEY_ID=alias/ssai-token-mk
# ALIYUN_ACCESS_KEY=
# ALIYUN_ACCESS_SECRET=
# ALIYUN_REGION=cn-hangzhou
```

### 40.2 密钥分层

```
┌──────────────────────────────────────────┐
│ 明文暴露：仅 .env 文件（chmod 600）             │
│ · JWT_SECRET、TOKEN_ENCRYPTION_KEY                  │
│ · KS_APP_SECRET、DATABASE_URL（含 PG 密码）         │
├──────────────────────────────────────────┤
│ AES-GCM 加密存 PG：                                  │
│ · ks_accounts.access_token_enc                       │
│ · ks_accounts.refresh_token_enc                      │
│ · push_channels.webhook_url_enc                      │
│ · push_channels.sign_secret_enc                      │
├──────────────────────────────────────────┤
│ 服务端内存只读：仅运行时解密，无磁盘落盘明文     │
│ 客户端：永远看不到任何 Secret / Token 明文      │
└──────────────────────────────────────────┘
```

### 40.3 AES-256-GCM 加解密（Rust）

```rust
// server/src/crypto/aead.rs
use aes_gcm::{Aes256Gcm, Key, Nonce,
              aead::{Aead, KeyInit, OsRng, rand_core::RngCore}};

pub struct Aead256 { cipher: Aes256Gcm, version: u8 }

impl Aead256 {
    pub fn from_hex(hex_key: &str, version: u8) -> anyhow::Result<Self> {
        let bytes = hex::decode(hex_key)?;
        anyhow::ensure!(bytes.len() == 32, "TOKEN_ENCRYPTION_KEY 必须 32 字节");
        let key = Key::<Aes256Gcm>::from_slice(&bytes);
        Ok(Self { cipher: Aes256Gcm::new(key), version })
    }

    /// 输出格式：[version(1) || nonce(12) || ciphertext + tag]
    pub fn encrypt(&self, plaintext: &[u8]) -> anyhow::Result<Vec<u8>> {
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce);
        let ct = self.cipher
            .encrypt(Nonce::from_slice(&nonce), plaintext)
            .map_err(|e| anyhow::anyhow!("encrypt: {:?}", e))?;
        let mut out = Vec::with_capacity(1 + 12 + ct.len());
        out.push(self.version);
        out.extend_from_slice(&nonce);
        out.extend(ct);
        Ok(out)
    }

    pub fn decrypt(&self, blob: &[u8]) -> anyhow::Result<Vec<u8>> {
        anyhow::ensure!(blob.len() > 13, "ciphertext too short");
        // 调用方根据 blob[0] 选择对应版本的 Aead256
        let (nonce, ct) = (&blob[1..13], &blob[13..]);
        self.cipher
            .decrypt(Nonce::from_slice(nonce), ct)
            .map_err(|e| anyhow::anyhow!("decrypt: {:?}", e))
    }
}
```

### 40.4 凭据轮换 SOP

| 凭据 | 轮换周期 | 步骤 | KS_APP_SECRET | 立即（曾在聊天外泄）+ 每季度 | 快手平台重置 → 写 .env → systemctl restart → 逐个账户触发 /refresh-token 验证 |
| --- | --- | --- | --- | --- | --- |
| JWT_SECRET | 半年 | 生新 key → 灰度期双签验证 → 切换签发 → 旧 key 下线（用户会被强制重登） | TOKEN_ENCRYPTION_KEY | 每年 / 怀疑泄露 | 生新 key + 版本++（v2）→ 服务支持双版本解密 → 后台 job 读-解-重加密 → 全量迁移后删 v1 |
| PG_PASSWORD | 半年 | ALTER USER → .env → restart | Webhook URL | 仅在泄露时（平台生成新 URL） | UI 中替换 → 服务端重新加密存 |

### 40.5 用户登录凭据

- 密码：Argon2id（params: memory=65536, iter=3, parallel=4）存 `users.password_hash`
- 可选 TOTP 2FA：`users.totp_secret` 存 AES-GCM 加密的 Base32 秘钥；首次启用时客户端二维码扫码指定 app
- fail2ban 规则：监控 journalctl 中 `/api/auth/login` 5min 内 5 次 401 → 封禁 IP 1h

```
# /etc/fail2ban/filter.d/ssai-login.conf
[Definition]
failregex = ^.*ssai-server.*login_failed.*ip=<HOST>.*$
ignoreregex =

# /etc/fail2ban/jail.d/ssai-login.conf
[ssai-login]
enabled = true
port    = https
filter  = ssai-login
backend = systemd
journalmatch = _SYSTEMD_UNIT=ssai-server.service
maxretry = 5
findtime = 300
bantime  = 3600
```

---

## 41. 监控、备份与 CI/CD

### 41.1 PostgreSQL 备份

```bash
# /opt/ssai/bin/backup.sh  (chown ssai:ssai, chmod 750)
#!/usr/bin/env bash
set -euo pipefail
BACKUP_DIR=/opt/ssai/backup
STAMP=$(date +%Y%m%d_%H%M%S)
mkdir -p "$BACKUP_DIR"

export PGPASSWORD=$(grep ^DATABASE_URL /opt/ssai/config/.env \
                    | sed -E 's|.*//[^:]+:([^@]+)@.*|\1|')
pg_dump -h 127.0.0.1 -U ssai_app -d ssai -F c \
        -f "$BACKUP_DIR/ssai_$STAMP.dump"
unset PGPASSWORD

# 本地保留 30 天
find "$BACKUP_DIR" -name 'ssai_*.dump' -mtime +30 -delete

# 可选：同步至阿里云 OSS（rclone 配好以后）
# rclone copy "$BACKUP_DIR/ssai_$STAMP.dump" aliyun-oss:ssai-backup/ \
#        --min-age 0 --s3-no-check-bucket

echo "[$(date)] backup ssai_$STAMP.dump OK"
```

```
# /etc/systemd/system/ssai-backup.service
[Unit]
Description=SSAI PostgreSQL daily backup
After=postgresql.service

[Service]
Type=oneshot
User=ssai
ExecStart=/opt/ssai/bin/backup.sh
StandardOutput=journal
StandardError=journal

# /etc/systemd/system/ssai-backup.timer
[Unit]
Description=Run ssai-backup daily at 03:00

[Timer]
OnCalendar=*-*-* 03:00:00
Persistent=true

[Install]
WantedBy=timers.target
```

```bash
systemctl daemon-reload
systemctl enable --now ssai-backup.timer
systemctl list-timers | grep ssai-backup
```

### 41.2 日志轮转

```
# /etc/logrotate.d/ssai
/opt/ssai/logs/*.log {
    daily
    rotate 14
    compress
    missingok
    notifempty
    copytruncate
    su ssai ssai
    postrotate
        systemctl reload ssai-server > /dev/null 2>&1 || true
    endscript
}
```

journald 侧通过 `/etc/systemd/journald.conf` 限制：

```
SystemMaxUse=500M
SystemKeepFree=500M
MaxRetentionSec=2week
```

### 41.3 健康检查 + 服务自监控

- **Nginx → ssai-server**：`GET /health` 返回 `{ ok: true }`
- **ssai-server → PG**：`GET /ready` 执行 `SELECT 1`（失败时返 503）
- **外部拨测**：阿里云云监控站点监控 / UptimeRobot / Better Uptime 每 1min 抨 `https://kuaishou.ssaitool.cn/health`。失败连续 2 次进入告警。
- **服务自自检 cron**：在服务器本地加一份兜底，`/health` 3min 不通则直接通过 IM Webhook（不走 PG）到管理群：

```bash
# /opt/ssai/bin/selfcheck.sh (every 3min cron)
#!/usr/bin/env bash
if ! curl -fsS --max-time 5 https://kuaishou.ssaitool.cn/health > /dev/null; then
  curl -fsS -X POST "$FALLBACK_WECOM_WEBHOOK" \
       -H 'Content-Type: application/json' \
       -d '{"msgtype":"text","text":{"content":"🚨 SSAI 服务端 /health 不可达 3min！请立即检查 kuaishou.ssaitool.cn"}}'
fi
```

### 41.4 可选：Prometheus 指标

```
ssai_http_requests_total{path,method,status}
ssai_http_request_duration_seconds_bucket{path}
ssai_kuaishou_api_calls_total{endpoint,code}
ssai_kuaishou_api_quota_used_ratio{endpoint}
ssai_active_ws_connections
ssai_scheduler_runs_total{job,status}
ssai_push_send_total{provider,status}
ssai_token_refresh_total{status}
ssai_alert_events_total{rule_type,level}
```

通过 `/metrics`（仅 127.0.0.1 绑定）暴露；配 node_exporter 后总内存开销 ~80MB。建议 v1 先不上 Prom，等稳定运行后再接。

### 41.5 GitHub Actions CI/CD

```yaml
# .github/workflows/server-deploy.yml
name: deploy-server
on:
  push:
    branches: [main]
    paths: ['server/**']
concurrency:
  group: deploy-prod
  cancel-in-progress: false

jobs:
  build_deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Lint & Test
        run: |
          cd server
          cargo clippy --all-targets -- -D warnings
          cargo test --locked
      - name: Build release
        run: cd server && cargo build --release --locked
      - name: Upload artifact
        uses: appleboy/scp-action@v0.1.7
        with:
          host: 8.162.4.113
          username: root
          key: $ secrets.DEPLOY_SSH_KEY 
          source: "server/target/release/ssai-server,server/migrations/*.sql"
          target: "/tmp/ssai-deploy/"
          strip_components: 0
      - name: Remote deploy
        uses: appleboy/ssh-action@v1.0.3
        with:
          host: 8.162.4.113
          username: root
          key: $ secrets.DEPLOY_SSH_KEY 
          script: |
            mv /tmp/ssai-deploy/server/target/release/ssai-server /tmp/
            mkdir -p /tmp/migrations && mv /tmp/ssai-deploy/server/migrations/*.sql /tmp/migrations/ 2>/dev/null || true
            /opt/ssai/bin/deploy.sh
```

Secrets 需在 GitHub repo 中配：

- `DEPLOY_SSH_KEY`：ED25519 私钥（服务器 `/root/.ssh/authorized_keys` 加对应公钥）
- 可选 `WECOM_NOTIFY_URL`：部署成功/失败推一条到企微

### 41.6 发版 Checklist

- [ ]  `cargo clippy -- -D warnings` 无告警
- [ ]  `cargo test` 全绿
- [ ]  migrations 向后兼容（支持回滚或有手工回滚 SQL）
- [ ]  本地 docker-compose 跑一遍 E2E（可选）
- [ ]  预发环境验证 `/api/kuaishou/oauth/callback` 授权成功回跳
- [ ]  手动跳一次 [backup.sh](http://backup.sh) 并检查 dump 文件大小
- [ ]  生产部署后 5 分钟内 `journalctl -u ssai-server -p err` 无错
- [ ]  客户端升级下发（如协议破坏性变更时开强制升级）

---

## 43. 浅色主题 Design Token（✅ P2）

```css
/* === src/styles/variables-light.css === */
/* 浅色主题 —— settings.theme === 'light' 时加载；与 §12 深色主题对偶 */
/* 切换方式：在 <html data-theme="light|dark"> 上打标签，CSS 以属性选择器区分 */

[data-theme="light"] {
  /* --- 背景色 --- */
  --bg-primary:        #F7F8FA;
  --bg-card:           #FFFFFF;
  --bg-sidebar:        #FFFFFF;
  --bg-topbar:         #FFFFFF;
  --bg-modal:          #FFFFFF;
  --bg-input:          #F2F3F5;
  --bg-hover:          #F0F2F5;

  /* --- 品牌色（与深色一致，保证辨识度） --- */
  --brand-primary:     #FF6600;
  --brand-hover:       #FF8533;
  --brand-active:      #E65C00;
  --brand-alpha-20:    rgba(255, 102, 0, 0.12);

  /* --- 语义色（饱和度适度降低） --- */
  --color-up:          #00A870;
  --color-down:        #E02020;
  --color-warning:     #F5A623;
  --color-info:        #2E6BE6;

  /* --- 文字色（反向） --- */
  --text-primary:      #1F2329;
  --text-secondary:    #4E5969;
  --text-disabled:     #A8B0B8;
  --text-inverse:      #FFFFFF;

  /* --- 边框 --- */
  --border:            #E5E6EB;
  --border-active:     rgba(255, 102, 0, 0.32);

  /* --- 阴影（去厚度，拟物化弱化） --- */
  --shadow-modal:      0 6px 24px rgba(0, 0, 0, 0.08);
  --shadow-dropdown:   0 3px 12px rgba(0, 0, 0, 0.06);

  /* --- 圆角/间距/布局/字体/动画复用 §12 根变量 --- */
}
```

```yaml
theme_switching:
  storage_key: "ssai.theme"                     # localStorage
  default: "system"
  resolver: |
    if settings.theme === 'system':
      html.setAttribute('data-theme',
        matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light')
    else:
      html.setAttribute('data-theme', settings.theme)
  antd_integration: |
    <ConfigProvider theme={{
      algorithm: isLight ? antdTheme.defaultAlgorithm : antdTheme.darkAlgorithm,
      token: { colorPrimary: '#FF6600' }
    }}>
  echarts_integration: |
    echarts.registerTheme('ssai-light', lightTheme)
    <ReactECharts theme={isLight ? 'ssai-light' : 'ssai-dark'} />
  sparkline_contrast:
    dark:  "brand-primary on bg-card"
    light: "brand-primary on #FFFFFF + 12% 填充带，避免刺眼"
  system_change_listener:
    event: "matchMedia('(prefers-color-scheme: dark)').addEventListener('change', ...)"
    debounce_ms: 200
```

---

## 44. 空状态（Empty State）规范（✅ P2）

<aside>
🎨

所有页面在首次无数据、筛选无结果、断网、授权失效时统一使用 EmptyState 组件。
v1 使用单色线条风格（brand-primary 线稿 + secondary 填充），所有插画 SVG 单色，便于深浅主题继承 currentColor。
Lottie/GIF 暂不引入，控制打包体积。

</aside>

```tsx
// === src/components/EmptyState.tsx ===
export interface EmptyStateProps {
  scene: EmptyScene;
  actionLabel?: string;
  onAction?: () => void;
  /** 替换默认文案（可选） */
  title?: string;
  description?: string;
}

export type EmptyScene =
  | 'no_accounts'       // 未绑定任何账户
  | 'no_campaigns'      // 账户下暂无计划
  | 'no_alerts'         // 无告警（安静状态）
  | 'no_push_channels'  // 未配置推送渠道
  | 'no_data'           // 数据未到达/筛选无结果
  | 'first_launch'      // 首次启动欢迎屏
  | 'error_offline'     // 网络离线
  | 'error_auth';       // 授权失效

const SCENES: Record<EmptyScene, {
  illustration: string;
  title: string;
  description: string;
  defaultAction?: string;
}> = {
  no_accounts: {
    illustration: 'empty-accounts.svg',
    title: '还没有绑定快手账户',
    description: '点击下方按钮跳转快手开放平台授权，绑定后将自动拉取数据',
    defaultAction: '立即授权',
  },
  no_campaigns: {
    illustration: 'empty-campaigns.svg',
    title: '该账户暂无投放计划',
    description: '请在快手后台创建计划，5 分钟内会自动同步到这里',
  },
  no_alerts: {
    illustration: 'empty-peaceful.svg',
    title: '当前一切正常',
    description: '没有触发任何告警规则，持续监控中…',
  },
  no_push_channels: {
    illustration: 'empty-push.svg',
    title: '还未配置推送渠道',
    description: '添加企微/钉钉/飞书/Slack 机器人，告警才能实时送达团队',
    defaultAction: '去添加',
  },
  no_data: {
    illustration: 'empty-search.svg',
    title: '没有匹配的数据',
    description: '换个时间范围或清除筛选条件试试',
  },
  first_launch: {
    illustration: 'welcome.svg',
    title: '欢迎使用 SSAI 快手监控',
    description: '纯只读监控 · 多账户聚合 · 智能告警 · AI 优化建议',
    defaultAction: '开始 3 步配置',
  },
  error_offline: {
    illustration: 'empty-offline.svg',
    title: '网络好像断开了',
    description: '请检查网络连接或稍后重试；本地缓存数据仍可查看',
    defaultAction: '重试',
  },
  error_auth: {
    illustration: 'empty-auth.svg',
    title: '账户授权已失效',
    description: 'Token 自动续期失败，需要重新授权才能继续拉取数据',
    defaultAction: '重新授权',
  },
};
```

```
插画资源清单（public/illustrations/）：
empty-accounts.svg     360×240   空档案 + 加号按钮
empty-campaigns.svg    360×240   空仪表盘
empty-peaceful.svg     360×240   安静山丘 + 绿色勾
empty-push.svg         360×240   4 个 IM 图标 + 对话气泡
empty-search.svg       320×200   空放大镜
welcome.svg            480×300   快手橙小火箭（多色，唯一例外）
empty-offline.svg      320×200   断开的 WiFi
empty-auth.svg         320×200   锁 + 过期沙漏

设计规范：
- 线稿主色：currentColor（通过 CSS var(--brand-primary) 赋值）
- 线稿辅色：var(--text-secondary)
- 描边宽度：2px；圆角端点
- 所有 SVG 单色可被 theme 主题色继承（welcome.svg 除外）
- 每张 SVG ≤ 10KB，未压缩路径控制在 80 个节点内
```

---

## 45. E2E 测试用例清单（✅ P2）

```yaml
# 工具：Playwright + Tauri WebDriver（tauri-driver）
# 运行：pnpm test:e2e
# CI：GitHub Actions macos-latest + windows-latest，PR 必跑 P0 子集

test_suites:

  1_onboarding:
    name: "首次使用引导"
    priority: P0
    cases:
      - id: E01
        name: "首次启动展示欢迎屏"
        steps:
          - 清空 Keychain / 本地 SQLite / 7d 缓存
          - 启动应用
          - 断言 URL = /onboarding
          - 断言存在文案「欢迎使用 SSAI 快手监控」
      - id: E02
        name: "登录服务端 → 触发 OAuth 授权 → 绑定账户"
        steps:
          - 点击「开始 3 步配置」
          - 输入用户名/密码（mock 服务端返回 JWT）
          - 点击「绑定第一个账户」
          - 断言系统默认浏览器已打开 developers.e.kuaishou.com
          - 回调 mock 返回 auth_code=mock_code
          - 断言账户列表出现 1 个账户，状态=已授权

  2_dashboard:
    name: "数据概览页"
    priority: P0
    cases:
      - id: E10
        name: "5 个指标卡渲染"
        steps:
          - 注入 mock 数据（§33）
          - 打开 /
          - 断言存在 5 张 MetricCard
          - 断言「今日消耗(元)」值 = 5280.50
      - id: E11
        name: "渠道分布饼图点击筛选右侧账户表格"

  3_monitor:
    name: "投放监控三级联动"
    priority: P0
    cases:
      - id: E20
        name: "计划 → 单元 → 创意 展开"
      - id: E21
        name: "筛选 ROI<1.0 后列表全部 <1.0"

  4_alerts:
    name: "告警闭环"
    priority: P0
    cases:
      - id: E30
        name: "服务端 WS 推送 new_alert → UI 铃铛 badge +1"
        steps:
          - 服务端 WS 推送一条 new_alert 事件
          - 断言右上角铃铛 badge +1
          - 断言实时告警流顶部出现新条目
      - id: E31
        name: "标记已处理并持久化"

  5_push:
    name: "IM 推送配置"
    priority: P1
    cases:
      - id: E40
        name: "添加企微渠道并测试"
      - id: E41
        name: "钉钉加签 HmacSHA256 验签通过"
      - id: E42
        name: "推送失败进入重试队列（验证 §31）"

  6_ai:
    name: "智能优化"
    priority: P1
    cases:
      - id: E50
        name: "无 LLM Key 时降级为规则建议（§29）"
      - id: E51
        name: "配置 LLM Key 后切换为 AI 建议"

  7_theme:
    name: "主题切换"
    priority: P0
    cases:
      - id: E60
        name: "深 ↔ 浅主题切换"
        steps:
          - /settings → 主题 → 选择「浅色」
          - 断言 html[data-theme] = 'light'
          - 断言 --bg-primary 计算值 = #F7F8FA
      - id: E61
        name: "跟随系统主题实时切换"

  8_resilience:
    name: "容错与断网"
    priority: P0
    cases:
      - id: E70
        name: "断网展示 EmptyState scene=error_offline + 本地缓存"
      - id: E71
        name: "Token 过期自动刷新"
        steps:
          - Mock 首次返回 code=40001，第二次返回 code=0
          - 触发一次 /metrics 请求
          - 断言最终渲染成功，无 Toast 报错
      - id: E72
        name: "服务端 5xx 时指数退避重试（不刷屏报错）"

coverage_goal:
  p0_scenarios: "100%（OAuth / Dashboard / 告警 / 主题切换 / 容错）"
  p1_scenarios: "≥ 80%"
  flaky_tolerance: "<1%（连续 10 次 CI 运行）"

continuous_integration:
  pr_trigger: "P0 用例（E01-E02, E10, E20, E30-E31, E60, E70-E71）≈ 5 分钟"
  nightly: "全量用例 ≈ 25 分钟"
  flaky_retry: 2
  artifact: "失败截图 + trace.zip 保留 14 天"
```

---

## 46. 代码签名与公证（✅ P2）

### 46.1 macOS 签名 + 公证

```yaml
prerequisites:
  - 苹果开发者账号（Individual $99/y 或 Organization $99/y）
  - 一台 macOS 或 GitHub Actions macos-latest runner
  - Xcode Command Line Tools

step_1_certificate:
  - 打开 https://developer.apple.com/account → Certificates
  - 创建「Developer ID Application」证书（用于 .app / .dmg 签名）
  - 下载 .cer → 双击导入「钥匙串访问」→ 登录钥匙串
  - 右键导出为 .p12 文件，设置密码（保存到密码管理器条目 ssai/apple/p12）

step_2_app_specific_password:
  - https://appleid.apple.com → 安全 → App 专用密码 → 生成
  - 保存到密码管理器条目 ssai/apple/notary
  - 供 notarytool 公证使用

step_3_github_secrets:
  APPLE_CERTIFICATE_P12_BASE64: "base64 -i cert.p12 | pbcopy"
  APPLE_CERTIFICATE_PASSWORD:   "导出 .p12 时设置的密码"
  APPLE_TEAM_ID:                "Membership → Team ID（10 位字母数字）"
  APPLE_ID:                     "你的苹果账号邮箱"
  APPLE_APP_SPECIFIC_PASSWORD:  "步骤 2 生成的专用密码"

step_4_tauri_config:
  file: src-tauri/tauri.conf.json
  bundle.macOS:
    signingIdentity: "Developer ID Application: <Your Name> (TEAM_ID)"
    hardenedRuntime: true
    entitlements: "entitlements.plist"

entitlements.plist: |
  <?xml version="1.0" encoding="UTF-8"?>
  <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
  <plist version="1.0">
  <dict>
    <key>com.apple.security.network.client</key><true/>
    <key>com.apple.security.network.server</key><false/>
    <key>com.apple.security.cs.allow-jit</key><false/>
    <key>com.apple.security.cs.allow-unsigned-executable-memory</key><false/>
  </dict>
  </plist>

step_5_notarize:
  command: |
    xcrun notarytool submit target/release/bundle/macos/SSAI.dmg \
      --apple-id "$APPLE_ID" \
      --team-id "$APPLE_TEAM_ID" \
      --password "$APPLE_APP_SPECIFIC_PASSWORD" \
      --wait
    xcrun stapler staple target/release/bundle/macos/SSAI.dmg
  verify: "xcrun stapler validate SSAI.dmg  # The validate action worked!"
  failure_log: "xcrun notarytool log <submission-id> --apple-id ... --team-id ... --password ..."
```

### 46.2 Windows 代码签名

```yaml
certificate_options:
  ov_cert:
    供应商: "Sectigo / DigiCert / SSL.com"
    成本: "≈ $250/年"
    特点: "首次安装仍会出现 SmartScreen 警告，需要累计下载量后自动信任"
  ev_cert:
    供应商: "同上，但需 USB HSM 设备"
    成本: "≈ $450/年"
    特点: "SmartScreen 立即信任，推荐正式发版"
  self_signed:
    用途: "仅本地开发测试，用户会看到红色警告，绝不用于发版"

step_1_buy_cert:
  - 提交公司营业执照（OV）或公司资料 + 电话回访（EV）
  - 等待 1-5 个工作日审核
  - OV：下载 .pfx；EV：收到 USB HSM

step_2_github_secrets:
  WIN_CERTIFICATE_PFX_BASE64: "certutil -encode cert.pfx tmp.b64 && type tmp.b64"
  WIN_CERTIFICATE_PASSWORD:   ".pfx 导出密码"

step_3_tauri_config:
  file: src-tauri/tauri.conf.json
  bundle.windows:
    certificateThumbprint: null       # 由 CI 动态注入
    digestAlgorithm: "sha256"
    timestampUrl: "http://timestamp.digicert.com"
    tsp: false
    wix:
      language: ["zh-CN", "en-US"]
      template: null

step_4_sign_command:
  command: |
    signtool sign /fd sha256 /f cert.pfx /p "$PASSWORD" \
      /tr http://timestamp.digicert.com /td sha256 \
      target/release/bundle/nsis/SSAI_1.0.0_x64-setup.exe
  verify: "signtool verify /pa /v SSAI_1.0.0_x64-setup.exe"
```

### 46.3 GitHub Actions 签名流水线片段

```yaml
# .github/workflows/release.yml（关键片段 —— 完整流水线见 §41.4）
jobs:
  build-macos:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - name: Import signing certificate
        env:
          P12_B64: $ secrets.APPLE_CERTIFICATE_P12_BASE64 
          P12_PWD: $ secrets.APPLE_CERTIFICATE_PASSWORD 
        run: |
          echo "$P12_B64" | base64 -d > cert.p12
          security create-keychain -p tmp build.keychain
          security default-keychain -s build.keychain
          security unlock-keychain -p tmp build.keychain
          security import cert.p12 -k build.keychain -P "$P12_PWD" -T /usr/bin/codesign
          security set-key-partition-list -S apple-tool:,apple: -s -k tmp build.keychain
      - name: Build + Sign + Notarize
        env:
          APPLE_ID: $ secrets.APPLE_ID 
          APPLE_TEAM_ID: $ secrets.APPLE_TEAM_ID 
          APPLE_PASSWORD: $ secrets.APPLE_APP_SPECIFIC_PASSWORD 
        run: pnpm tauri build

  build-windows:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - name: Decode PFX certificate
        shell: pwsh
        run: |
          [IO.File]::WriteAllBytes("cert.pfx",
            [Convert]::FromBase64String("$ secrets.WIN_CERTIFICATE_PFX_BASE64 "))
      - name: Build + Sign
        env:
          TAURI_WINDOWS_CERT_PFX: "cert.pfx"
          TAURI_WINDOWS_CERT_PASSWORD: $ secrets.WIN_CERTIFICATE_PASSWORD 
        run: pnpm tauri build
```

---

## 47. [CHANGELOG.md](http://CHANGELOG.md) 模板与自动化（✅ P2）

### 47.1 CHANGELOG 结构（Keep a Changelog 1.1.0 + SemVer）

```markdown
# CHANGELOG

本文件记录 SSAI-快手监控 的所有发布变更。
格式遵循 Keep a Changelog（https://keepachangelog.com/zh-CN/1.1.0/），
版本号遵循语义化版本（https://semver.org/lang/zh-CN/）。

## [Unreleased]
### Added
### Changed
### Fixed
### Security

---

## [1.0.0] - 2026-06-XX
### Added
- 🚀 首版发布
- 混合架构：Tauri 桌面客户端 + 阿里云 Rust Axum 中转服务端（§35）
- 磁力金牛 ESP OAuth 授权与 Token 自动续期（§9 / §35.3）
- 6 大核心页面：数据概览 / 广告账户 / 投放监控 / 效果分析 / 告警中心 / 智能优化
- 7 类告警规则 + 4 平台 IM 推送（企微 / 钉钉 / 飞书 / Slack）
- AI 计划健康度 5 维度评分 + LLM 归因建议（§27 / §28），无 Key 时走规则降级（§29）
- 服务端 PostgreSQL 权威存储 + 客户端 SQLite 7 天缓存（§37.4 / §39.4）
- 深 / 浅 / 跟随系统 三种主题（§12 / §43）

### Security
- App Secret 全程仅存于服务端加密存储，客户端二进制绝不包含（§23 / §40）
- 日志 10 条脱敏规则，Token / 手机号 / 邮箱 / 签名均不落盘（§24）
- OAuth State 参数 + 防 CSRF；JWT bearer + 短 TTL；fail2ban 基线（§39 / §40.5）

---

## [0.9.0] - 2026-05-XX
### Added
- Beta 内测版本，仅内部团队使用

[Unreleased]: https://github.com/eddylu93/ssai-ksdesktop/compare/v1.0.0...HEAD
[1.0.0]:      https://github.com/eddylu93/ssai-ksdesktop/compare/v0.9.0...v1.0.0
[0.9.0]:      https://github.com/eddylu93/ssai-ksdesktop/releases/tag/v0.9.0
```

### 47.2 约定式提交（Conventional Commits）

```yaml
commit_format: "<type>(<scope>): <subject>"

types:
  feat:     "新功能 → Added"
  fix:      "缺陷修复 → Fixed"
  perf:     "性能优化 → Changed"
  refactor: "重构（不影响功能）→ Changed"
  docs:     "文档"
  style:    "格式（非功能）"
  test:     "测试"
  chore:    "构建 / 依赖"
  security: "安全相关 → Security"
  revert:   "回滚"

scopes:
  - client     # 桌面客户端
  - server     # 云端服务端
  - api        # 快手 API 适配层
  - alert      # 告警引擎
  - push       # IM 推送
  - ai         # AI 建议
  - ui         # 界面/交互
  - db         # 数据库/迁移
  - deploy     # 部署/运维
  - ci         # GitHub Actions

examples:
  - "feat(alert): 支持 CPA 飙升告警规则"
  - "fix(api): 修正 /v2/esp/goods/report 字段映射"
  - "perf(db): metrics_snapshots 新增 (ref_id, snapshot_at) 复合索引"
  - "security(server): 轮换 JWT 签名密钥"
  - "refactor(ui): EmptyState 抽离为通用组件"

validation:
  tool: "commitlint + husky"
  config: ".commitlintrc.json"
  install: "pnpm add -D @commitlint/cli @commitlint/config-conventional"
  hook: ".husky/commit-msg → npx --no commitlint --edit $1"
```

### 47.3 自动生成 CHANGELOG（git-cliff）

```toml
# cliff.toml
[changelog]
header = "# CHANGELOG\n\n"
body = """
{% if version %}## [ version | trim_start_matches(pat="v") ] -  timestamp | date(format="%Y-%m-%d") {% else %}## [Unreleased]{% endif %}
{% for group, commits in commits | group_by(attribute="group") %}
###  group | upper_first 
{% for commit in commits %}-  commit.message | split(pat="\n") | first | upper_first {% if commit.scope %} ( commit.scope ){% endif %}
{% endfor %}{% endfor %}
"""

[git]
conventional_commits = true
filter_unconventional = true
commit_parsers = [
  { message = "^feat",     group = "Added"    },
  { message = "^fix",      group = "Fixed"    },
  { message = "^perf",     group = "Changed"  },
  { message = "^refactor", group = "Changed"  },
  { message = "^security", group = "Security" },
  { message = "^docs",     skip  = true       },
  { message = "^chore",    skip  = true       },
]
```

```bash
# 发版流程（与 §41.4 CI 串联）
git cliff --tag v1.0.1 --output CHANGELOG.md
git add CHANGELOG.md
git commit -m "docs: update CHANGELOG for v1.0.1"
git tag v1.0.1
git push origin main --tags
# CI 触发 release.yml → 构建 + 签名 + 公证 + 发布 GitHub Release
```

---

## 48. MVP 优先级与 Walking Skeleton（⭐⭐⭐ P0 - 最高优先级）

<aside>
🚦

**核心原则**：在任何其他功能开发之前，必须先打通「OAuth 授权 → Server 定时拉取 → Tauri 客户端呈现」的端到端数据流（Walking Skeleton）。此链路不通，告警 / AI / 推送 / 主题 / 插画等所有功能都是摆设。

Codex 必须严格按 §48.1 的 T01→T06 顺序串联交付，不得跳级、不得并行开发 P1/P2 的任何功能。

</aside>

### 48.1 P0 Walking Skeleton — 6 步串联交付

```yaml
milestone_p0_walking_skeleton:
  goal: "一个真实快手账号的 24h 基础投放数据能在 Tauri 客户端 Dashboard 页看到"
  time_budget: "2 人周（1 人 server + 1 人客户端，可 T01-T05 并行，T06 汇合）"

  acceptance:
    - "用户在客户端点击『授权快手账号』，浏览器完成快手 OAuth 回调"
    - "Server 接收 auth_code，换取 access_token + refresh_token，AES-GCM 加密入库"
    - "Server 调度器每 15 分钟拉取 /v1/advertiser/list + /v1/report/unit_report"
    - "客户端 Dashboard 能看到：账号名 / 余额 / 当日消耗 / 曝光 / 点击 / CTR / CPC / 转化 / CPA 至少 9 列"
    - "指标与快手后台误差 < 2%（仅限口径一致的字段）"
    - "E2E 冒烟用例 E01 通过（§45）"

  strict_task_sequence:
    T01_server_skeleton:
      desc: "Rust/Axum + PG + systemd 部署骨架"
      deliverables:
        - "GET /health 返回 {status:'ok', version}"
        - "ks_accounts / ks_reports / ks_oauth_states 三表建成"
      ref: "§37, §41.1"

    T02_oauth_flow:
      desc: "OAuth 授权链接 + callback + token 加密存储"
      deliverables:
        - "GET /api/kuaishou/oauth/url 返回授权 URL（含 state CSRF）"
        - "GET /api/kuaishou/oauth/callback 交换 token，写 ks_accounts.token_encrypted"
        - "dev 环境可打印 masked token 最后 4 位到日志"
      ref: "§9, §23, §24"

    T03_advertiser_list:
      desc: "拉取并持久化广告主列表"
      deliverables:
        - "POST /rest/openapi/v1/advertiser/list 调用成功"
        - "ks_accounts 至少 1 行 is_active=true"
      ref: "§8, §37.3"

    T04_unit_report_pull:
      desc: "定时调度 + unit_report 拉取 + 分页 + 重试"
      deliverables:
        - "tokio-cron-scheduler 每 15min 触发一次 pull_unit_report job"
        - "ks_reports 每次新增当日数据，含 impression/click/cost/conversion"
        - "40001 自动 refresh_token 续签重试，40029 指数退避"
      ref: "§10, §37.3"

    T05_client_rest_consume:
      desc: "Tauri 客户端消费 Server REST + 展示"
      deliverables:
        - "GET /api/accounts / /api/reports/today 返回结构化数据"
        - "Dashboard 页 Antd Table 渲染 9 列指标（可先不加图表）"
        - "客户端通过 OS keyring 保存 server 的 JWT（§6, §24）"
      ref: "§6, §12, §13"

    T06_e2e_smoke:
      desc: "端到端冒烟：真实账号 → server → 客户端 → 数据可见"
      deliverables:
        - "Playwright E01 走完授权 → 等待 15min → Dashboard 有数据"
        - "录制一段 30s 演示视频存档"
      ref: "§45 E01"

  explicit_deferrals:
    # P0 绿灯之前，以下功能禁止开发
    - "告警规则引擎（§10）"
    - "IM 推送任何通道（§11, §30, §31）"
    - "AI 诊断 / 建议 / 多模型（§27, §28, §29）"
    - "浅色主题 / Design Token 切换（§43）"
    - "空状态插画（§44）"
    - "磁力金牛 ESP 独有报表（esp_goods / esp_live / esp_photo_roi / esp_fans / esp_shop_order）"
    - "多账号并发 / 素材诊断 / 归因分析"
    - "代码签名 / 公证 / CHANGELOG 自动化（§46, §47）"
```

### 48.2 P1 基础功能扩展（P0 稳定运行 ≥ 7 天后启动）

```yaml
milestone_p1_essential:
  prerequisites:
    - "P0 Walking Skeleton 全绿"
    - "P0 生产环境稳定运行 ≥ 7 天，无数据中断 > 30min 的事故"
  scope:
    - "磁力金牛 ESP 报表：esp_goods / esp_live / esp_photo_roi（优先级高，商家场景")
    - "告警规则配置 UI + 规则引擎（CTR 断崖 / ROI 下跌 / CPA 飙升，§10）"
    - "单通道 IM 推送（默认 Slack 或飞书，二选一，§30）"
    - "多账号聚合视图（汇总消耗 / ROI 对比）"
    - "基础 ECharts 折线图（日趋势，单账号单指标）"
  acceptance:
    - "用户能在告警页创建 ≥ 3 种规则并触发至少 1 次真实推送"
    - "单账号单报表最近 7 天数据能按天/按小时切换图表"
    - "多账号聚合页能按消耗降序排列所有活跃账号"
```

### 48.3 P2 体验与智能层（P1 上线 ≥ 14 天且 DAU ≥ 3 后启动）

```yaml
milestone_p2_enhancement:
  prerequisites:
    - "P1 上线 ≥ 14 天"
    - "DAU ≥ 3（至少 3 个真实用户日活）"
  scope:
    - "AI 诊断 + 四步推理建议（§27, §28, §29, §49.2.3）"
    - "多 IM 通道推送矩阵（§30, §31，含 §49.2.4 的 ntfy / bark）"
    - "浅色主题 / Design Token 切换（§43）"
    - "空状态 8 场景插画（§44）"
    - "E2E 全套 18 用例（§45）"
    - "macOS + Windows 代码签名与公证（§46）"
    - "CHANGELOG 自动化 + Conventional Commits（§47）"
    - "SQLCipher 整库加密升级（§49.2.2）"
```

### 48.4 红线规则（Codex 必须遵守）

<aside>
⛔

**绝对禁止**：

1. P0 未绿之前，禁止开发任何 UI 主题、空状态插画、动画、骨架屏
2. P0 未绿之前，禁止集成任何 AI 模型 SDK（OpenAI / Claude / DeepSeek / 通义）
3. P0 未绿之前，禁止为 P1/P2 功能新建数据库表或 API 接口
4. 禁止把「数据拉取」和「数据展示」拆成不同 sprint：T01-T06 必须串联到冒烟绿灯
5. 禁止先做磁力金牛 ESP 独有报表：T03/T04 必须用 `/v1/advertiser/list` + `/v1/report/unit_report` 这两个最通用的接口
</aside>

<aside>
✅

**允许的技术债**（P0 阶段可以糙，P1/P2 再抛光）：

1. Dashboard 表格可以用原生 HTML `<table>`，不强求 Antd Table
2. ECharts 图表 P0 可完全留空，只要求数据表格可见
3. 单深色主题即可，无需切换
4. 单账号硬编码也能过 P0，多账号管理 UI 留到 P1
5. token 过期策略 P0 可以简单到「401 → 前端提示重新授权」，后台自动刷新留到 P1
6. 无需登录注册 UI，server JWT 用命令行预置也行
7. 无需 HTTPS 自动续签脚本，P0 用手动 certbot 即可
</aside>

---

## 49. 开源交叉验证与增强项（2026-04-24 调研）

<aside>
🔍

本章汇总对本项目有参考价值的开源项目，并基于调研结论给出 6 个增强 patch。patch 按优先级排序，★ 越多越值得做。

</aside>

### 49.1 参考项目清单

| 项目 | 类型 | 对本方案的验证点 | 链接 |
| --- | --- | --- | --- |
| **bububa/kwai-marketing-api** | 磁力引擎 Go SDK (35★) | API base URL / OAuth 端点 / 模块拆分完全一致；`ACTIVATE_URL` 常量我们漏了 | [github.com/bububa/kwai-marketing-api](http://github.com/bububa/kwai-marketing-api) |
| **google-marketing-solutions/ads-monitor** | Google 官方 Ads 监控 | 官方推荐 15min 拉取窗口，与我们 §10 一致；走 Prometheus+Grafana 标准协议 | [github.com/google-marketing-solutions/ads-monitor](http://github.com/google-marketing-solutions/ads-monitor) |
| **FabianLars/tauri-plugin-oauth** | Tauri OAuth 插件 | 桌面主流做法是 [localhost](http://localhost) loopback，我们走 server 中继是为了中国平台备案合规 | [github.com/FabianLars/tauri-plugin-oauth](http://github.com/FabianLars/tauri-plugin-oauth) |
| **sansan0/TrendRadar** | AI 舆情监控 + 多通道推送 | IM 矩阵标杆：微信/飞书/钉钉/TG/邮件/ntfy/bark/Slack | [github.com/sansan0/TrendRadar](http://github.com/sansan0/TrendRadar) |
| **Tauri + SQLCipher 社区实践** | 本地加密方案 | SQLCipher 整库加密是 local-first 桌面 app 的 defacto | [mhmtsr.medium.com](http://mhmtsr.medium.com) (building-a-local-first-password-manager) |
| **AppsFlyer ROAS 四步诊断** | 方法论 | 验证真实性 → 分维度归因 → 漏斗诊断 → 可执行建议 | [appsflyer-gcr.cn/blog/tips-and-strategy/roas-dropping](http://appsflyer-gcr.cn/blog/tips-and-strategy/roas-dropping) |

### 49.2 增强项清单（P2 阶段集中落地）

#### 49.2.1 ⭐⭐ Server Rust crate 目录重组（影响 §8, §37.3）

对齐 bububa/kwai-marketing-api 风格，便于 Codex 生成对应代码且未来接抖音/巨量引擎时结构一致：

```
ssai-server/src/kuaishou/
├── core/
│   ├── client.rs         # SDKClient（HTTP + Access-Token 鉴权头）
│   ├── const.rs          # BASE_URL / OAUTH_URL / ACTIVATE_URL（§49.2.6）
│   ├── error.rs          # 40001/40029/50000 错误码映射
│   └── retry.rs          # 指数退避 + refresh_token 自动续签
├── oauth/
│   ├── url.rs            # 授权链接生成（含 state CSRF）
│   ├── access_token.rs   # auth_code → access_token
│   └── refresh_token.rs  # refresh_token 刷新
├── advertiser/
│   ├── info.rs           # /v1/advertiser/list
│   └── fund.rs           # /v1/advertiser/fund/get
├── campaign/
│   └── list.rs           # /v1/campaign/list
├── unit/
│   └── list.rs           # /v1/unit/list
├── creative/
│   └── list.rs           # /v1/creative/list
└── report/
    ├── unit_report.rs            # /v1/report/unit_report（DSP 通用）
    ├── ecpm_report.rs            # /gw/dsp/v1/report/ecpm_report
    ├── esp_goods_report.rs       # /v2/esp/goods/report（磁力金牛）
    ├── esp_live_report.rs        # /v2/esp/live/report
    ├── esp_photo_roi_report.rs   # /v2/esp/photo/roi_report
    ├── esp_fans_report.rs        # /v2/esp/fans/report
    └── esp_shop_order.rs         # /v2/esp/shop/order_list
```

每个子模块导出 `async fn fetch(client: &SDKClient, token: &str, req: Req) -> Result<Resp>`，与 §8 的 `FieldMapper<ReportType>` 一一对应。

#### 49.2.2 ⭐⭐ SQLCipher 整库加密（影响 §5, §41.1，P2 阶段落地）

客户端 SQLite 从「列级 AES」升级为「整库 SQLCipher 透明加密」：

```toml
# src-tauri/Cargo.toml
[dependencies]
rusqlite = { version = "0.31", features = ["bundled-sqlcipher"] }
hex = "0.4"
```

```rust
// src-tauri/src/db/mod.rs
use rusqlite::Connection;
use std::path::Path;

pub fn open_encrypted(path: &Path, key: &[u8; 32]) -> anyhow::Result<Connection> {
    let conn = Connection::open(path)?;
    // PRAGMA key 必须在任何其他语句之前执行
    conn.pragma_update(None, "key", format!("x'{}'", hex::encode(key)))?;
    conn.pragma_update(None, "cipher_page_size", 4096_i32)?;
    conn.pragma_update(None, "kdf_iter", 256000_i32)?;
    conn.pragma_update(None, "cipher_hmac_algorithm", "HMAC_SHA256")?;
    conn.pragma_update(None, "cipher_kdf_algorithm", "PBKDF2_HMAC_SHA256")?;
    Ok(conn)
}
```

master key 仍由 OS keyring 提供（§24 `ssai.kuaishou.master_key`）；SQLite 文件在磁盘层完全加密，设备丢失时不可读取。

**迁移路径**：P0/P1 先用普通 rusqlite，P2 升级时写一次性迁移脚本 `migrate_to_sqlcipher.rs`（attach plain db → copy tables → detach）。

#### 49.2.3 ⭐⭐ AI 诊断引入 AppsFlyer 四步法（影响 §27, §28）

将 AI 建议 prompt 从「单轮问答」升级为「四步诊断推理模板」，AI 必须强制走完四步后再给建议：

```
你是 SSAI-快手监控的广告诊断助手。用户传入的异常数据如下：
{异常上下文 JSON}

必须严格按以下四步输出，不允许跳步：

## 步骤 1：验证数据真实性
- 数据拉取时间窗口是否完整？是否跨越快手数据回滚时段（凌晨 00:00-02:00）？
- 样本量是否 ≥ 1000 曝光？（< 1000 视为小样本波动，不视为异常）
- 是否与昨日/上周同期数据一致？
输出：□ 异常真实 / □ 数据问题 / □ 样本不足

## 步骤 2：分维度归因
- 按时段拆：突发性（< 1h）还是持续性（> 6h）？
- 按账号拆：全账号还是单账号？
- 按计划/素材拆：是否由单一计划/素材主导？
输出：主要归因维度 + 影响占比

## 步骤 3：漏斗诊断（按指标分支）
- 曝光 ↓ → 检查定向 / 出价 / 余额 / 审核状态
- CTR ↓ → 检查素材疲劳度 / 封面 / 标题
- 转化率 ↓ → 检查落地页 / 详情页 / 库存 / 活动
- 客单价 ↓ → 检查 SKU / 促销 / 组合策略
输出：定位到的漏斗断点

## 步骤 4：可执行建议（必须带优先级 + 预期影响）
- 🔴 P0 立即（< 1h）：xxx，预期恢复 yyy%
- 🟡 P1 今日内：xxx
- 🟢 P2 本周内：xxx

严格要求：
- 每一步都要有明确结论，不能用「可能」「也许」
- 建议必须是用户能自己在快手后台操作的（我们不做自动化）
- 禁止编造数据，所有归因都必须基于输入的 JSON
```

#### 49.2.4 ⭐ IM 推送补 ntfy + bark 轻量通道（影响 §30, §31）

在钉钉 / 飞书 / 企微 / Slack 四路之外，新增两条极简通道（单用户场景最实用）：

```yaml
additional_channels:
  ntfy:
    description: "开源自托管推送（https://ntfy.sh），无需企业账号"
    use_case: "个人开发者 / 无企业 IM 的小团队"
    config:
      endpoint: "https://ntfy.sh/{topic}"
      method: POST
      headers:
        Title: "{alert_title}"
        Priority: "{1-5}"
        Tags: "warning,chart_with_downwards_trend"
      body: "{alert_body_markdown}"

  bark:
    description: "iOS 原生推送（https://github.com/Finb/Bark）"
    use_case: "iOS 用户，无 IM 依赖"
    config:
      endpoint: "https://api.day.app/{device_key}"
      method: POST
      content_type: application/json
      body_json:
        title: "{alert_title}"
        body: "{alert_body}"
        group: "ssai-kuaishou"
        level: "timeSensitive"
        sound: "alarm"
```

实现上与其他 HTTP 通道共用 `push_channel_trait::PushChannel` trait（§31），仅新增 `impl PushChannel for NtfyChannel` 和 `impl PushChannel for BarkChannel`，每个 < 60 行代码。

#### 49.2.5 ⭐ Server `/metrics` Prometheus endpoint（影响 §39.1）

暴露标准 Prometheus 文本格式，为未来对接 Grafana 留后门，不影响 P0/P1 主线：

```rust
// ssai-server/src/api/metrics.rs
use axum::{http::StatusCode, response::IntoResponse, http::header};
use lazy_static::lazy_static;
use prometheus::{register_int_counter, register_int_counter_vec, IntCounter, IntCounterVec, TextEncoder};

lazy_static! {
    pub static ref KS_API_REQUESTS: IntCounterVec = register_int_counter_vec!(
        "ssai_kuaishou_api_requests_total",
        "Total kuaishou MAPI requests",
        &["endpoint", "status"]
    ).unwrap();

    pub static ref KS_REPORT_ROWS: IntCounterVec = register_int_counter_vec!(
        "ssai_kuaishou_report_rows_total",
        "Total rows pulled from kuaishou report API",
        &["report_type"]
    ).unwrap();

    pub static ref KS_ALERTS_FIRED: IntCounter = register_int_counter!(
        "ssai_alerts_fired_total",
        "Total alerts fired (across all channels)"
    ).unwrap();

    pub static ref KS_TOKEN_REFRESH: IntCounterVec = register_int_counter_vec!(
        "ssai_token_refresh_total",
        "Refresh token exchanges",
        &["result"]  // success / expired / error
    ).unwrap();
}

pub async fn metrics_handler() -> impl IntoResponse {
    let encoder = TextEncoder::new();
    let mut buffer = String::new();
    encoder.encode_utf8(&prometheus::gather(), &mut buffer).unwrap();
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, encoder.format_type())],
        buffer,
    )
}
```

路由：`GET /metrics`，仅监听 `127.0.0.1:8080`，nginx 不反代出外网，需要 Grafana 时通过 SSH tunnel 或内网访问。

#### 49.2.6 〇 补 ACTIVATE_URL 常量（影响 §23, §37.3）

磁力引擎还有一个 partner 上报域，当前阶段不使用但提前写入常量表，避免二期做点击/激活回传时改动大：

```rust
// ssai-server/src/kuaishou/core/const.rs
pub const BASE_URL: &str = "https://ad.e.kuaishou.com/rest/openapi";
pub const OAUTH_URL: &str = "https://developers.e.kuaishou.com";
pub const ACTIVATE_URL: &str = "http://ad.partner.gifshow.com/track/activate";  // ⬅ NEW，P0/P1 阶段不使用
```

### 49.3 差异化定位声明（写入 [README.md](http://README.md) 的 Positioning 段）

以下 4 条是 SSAI-快手监控区别于上述开源方案的核心，README 必须明确：

1. **中国平台合规的 OAuth 中继**：放弃 Tauri [localhost](http://localhost) loopback，采用 `kuaishou.ssaitool.cn` 备案域回调
2. **只读监控 + 克制式建议**：不做投放操作，不自动改计划/预算/暂停广告（与 BI4Sight 等商业产品明确切分）
3. **单租户本地优先**：Tauri 桌面客户端 + 个人 VPS server，非 SaaS，数据主权在用户
4. **AI 诊断作为一等公民**：四步诊断法（§49.2.3）+ 多模型策略（§27-§29）

---

## 42. v1.0 混合架构文档索引（Codex 快查）

| 主题 | 客户端参考 |
| --- | --- |
| 数据类型 | §3 (TS) · §4 (Rust) |
| IPC / REST | §6 IPC |
| 告警引擎 | §10 (本地) |
| UI | §12 · §13 · §17 · §32 |
| 凭据/密钥 | §23 (已废弃) · §24 日志脱敏 |
| 开发优先级 | §21 客户端任务队列 + 服务端推荐顺序：§37.3 目录 → §37.4 PG → §40 .env → §35.3 OAuth → §36 API 客户端 → §39.1 REST → §38 部署 |

<aside>
✅

<strong>§35–§42 补充完成</strong>。现在 CODEX_SPEC 已覆盖：桌面客户端全套 + 云端服务端全套 + 部署运维 + 通信协议 + 凭据管理。
Codex 开工顺序建议：先 <code>server/</code>（§37→§38→§40，拉通 OAuth + 一个报表接口 E2E），再 <code>client/</code> Tauri（§21 Phase_1 → §6 IPC 换为调服务端 REST/WS → §13 UI）。

</aside>

## 🛠 技术栈

| 层 | 技术 |
| --- | --- |
| 框架 | Tauri 2.x |
| 前端 | React 18 + TypeScript 5 + Vite |
| 后端 | Rust (stable ≥ 1.75) |
| 数据库 | SQLite (WAL mode) |
| 状态 | Zustand |
| UI | Ant Design 5 + ECharts 5 |

## 🚀 快速开始

### 环境要求

- Node.js ≥ 18
- pnpm ≥ 8
- Rust stable ≥ 1.75
- 系统: macOS 12+ / Windows 10+

### 安装与运行

```bash
# 克隆仓库
git clone [https://github.com/eddylu93/ssai-ksdesktop.git](https://github.com/eddylu93/ssai-ksdesktop.git)
cd ssai-ksdesktop

# 安装依赖
pnpm install

# 启动开发模式
pnpm tauri dev
```

### 首次使用

1. 启动应用后，按提示输入快手开放平台的 `App ID` 和 `App Secret`
2. 点击「添加账户」完成 OAuth 授权
3. 数据将自动开始同步

## 📁 项目结构

```
ssai-ksdesktop/
├── src-tauri/    # Rust 后端
├── src/          # React 前端
├── public/       # 静态资源
├── tests/        # 测试文件
└── scripts/      # 构建脚本
```

详细目录说明见 [CODEX_SPEC §20](./docs/CODEX_SPEC.md#20-项目目录结构)

## 📖 开发文档

- [CODEX_SPEC — 完整技术规格书](./docs/CODEX_SPEC.md)
- [PRD — 产品需求文档](./docs/PRD.md)

## ⚠️ 重要约束

**本工具仅做数据监控，绝不调用任何快手写入/修改 API。**

所有广告操作（创建、修改、暂停计划等）必须在快手官方后台完成。

## 📝 License

Private — All rights reserved.

```markdown

```