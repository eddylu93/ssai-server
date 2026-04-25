#![allow(dead_code)]

use anyhow::Context;
use base64::{engine::general_purpose::STANDARD, Engine};
use sqlx::{migrate::Migrator, query, PgPool};
use ssai_server::{config::Config, state::AppState};
use uuid::Uuid;

pub async fn test_state(bootstrap_admin_email: &str) -> anyhow::Result<AppState> {
    test_state_with_base_url(
        bootstrap_admin_email,
        "https://ad.e.kuaishou.com/rest/openapi".to_string(),
    )
    .await
}

pub async fn test_state_with_base_url(
    bootstrap_admin_email: &str,
    ks_base_url: String,
) -> anyhow::Result<AppState> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://ssai_app:ssai2026@localhost:5432/ssai".to_string());

    let config = Config {
        database_url,
        pg_max_conn: 5,
        bind_addr: "127.0.0.1:0".parse().expect("socket addr"),
        rust_log: "warn".to_string(),
        log_format: "pretty".to_string(),
        version: "test".to_string(),
        token_enc_key: STANDARD.encode([7u8; 32]),
        ks_app_id: "test-app".to_string(),
        ks_app_secret: "test-secret".to_string(),
        ks_redirect_uri: "http://localhost/callback".to_string(),
        ks_base_url,
        bootstrap_admin_email: Some(bootstrap_admin_email.to_string()),
        bootstrap_admin_password: Some("unused".to_string()),
    };

    let state = AppState::from_config(config).await?;
    Migrator::new(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations"
    )))
    .await
    .expect("migrator")
    .run(&state.pool)
    .await
    .context("failed to run test migrations")?;

    Ok(state)
}

pub async fn insert_bootstrap_admin(pool: &PgPool, email: &str) -> anyhow::Result<Uuid> {
    let user_id = Uuid::new_v4();

    query(
        r#"
        INSERT INTO users (id, email, password_hash, role, disabled)
        VALUES ($1, $2, 'test-hash', 'admin', false)
        ON CONFLICT (email) DO UPDATE
        SET disabled = false
        "#,
    )
    .bind(user_id)
    .bind(email)
    .execute(pool)
    .await?;

    Ok(user_id)
}

pub async fn cleanup_user(pool: &PgPool, user_id: Uuid, email: &str) -> anyhow::Result<()> {
    query("DELETE FROM metric_snapshots WHERE account_id IN (SELECT id FROM ks_accounts WHERE user_id = $1)")
        .bind(user_id)
        .execute(pool)
        .await?;
    query("DELETE FROM ks_accounts WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;
    query("DELETE FROM oauth_state WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;
    query("DELETE FROM users WHERE id = $1 OR email = $2")
        .bind(user_id)
        .bind(email)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn insert_ks_account_with_tokens(
    state: &AppState,
    user_id: Uuid,
    advertiser_id: &str,
    access_token: &str,
    refresh_token: &str,
) -> anyhow::Result<Uuid> {
    let account_id = Uuid::new_v4();
    let access_token_enc = state.aead.encrypt(access_token.as_bytes())?;
    let refresh_token_enc = state.aead.encrypt(refresh_token.as_bytes())?;

    query(
        r#"
        INSERT INTO ks_accounts (
            id, user_id, advertiser_id, advertiser_name,
            access_token_enc, refresh_token_enc,
            access_expires_at, refresh_expires_at,
            status, last_refreshed_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, now() + interval '5 minute', now() + interval '30 day', 'normal', now())
        "#,
    )
    .bind(account_id)
    .bind(user_id)
    .bind(advertiser_id)
    .bind(format!("Advertiser {advertiser_id}"))
    .bind(access_token_enc)
    .bind(refresh_token_enc)
    .execute(&state.pool)
    .await?;

    Ok(account_id)
}
