CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE users (
  id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  email         TEXT UNIQUE NOT NULL,
  password_hash TEXT NOT NULL,
  totp_secret   TEXT,
  role          TEXT NOT NULL DEFAULT 'user',
  disabled      BOOLEAN NOT NULL DEFAULT false,
  created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
  last_login_at TIMESTAMPTZ
);

CREATE TABLE oauth_state (
  state       TEXT PRIMARY KEY,
  user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  oauth_type  TEXT NOT NULL DEFAULT 'advertiser',
  created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
  used_at     TIMESTAMPTZ,
  expires_at  TIMESTAMPTZ NOT NULL DEFAULT (now() + interval '10 minute')
);
CREATE INDEX ON oauth_state(expires_at);

CREATE TABLE ks_accounts (
  id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id             UUID NOT NULL REFERENCES users(id),
  advertiser_id       TEXT NOT NULL,
  advertiser_name     TEXT NOT NULL,
  group_tag           TEXT,
  access_token_enc    BYTEA NOT NULL,
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

CREATE TABLE metric_snapshots (
  id           BIGSERIAL PRIMARY KEY,
  account_id   UUID NOT NULL REFERENCES ks_accounts(id) ON DELETE CASCADE,
  level        TEXT NOT NULL,
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
  raw          JSONB
);
CREATE INDEX ON metric_snapshots(account_id, level, ref_id, snapshot_at DESC);
CREATE INDEX ON metric_snapshots(snapshot_at);
