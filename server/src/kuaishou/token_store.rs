use anyhow::Context;
use chrono::{DateTime, Utc};
use sqlx::{query, query_as, Row};
use uuid::Uuid;

use crate::{
    error::AppError,
    kuaishou::oauth::{redact_token_tail, AdvertiserAccount, TokenBundle},
    state::AppState,
};

#[derive(Debug)]
pub struct ConsumedOauthState {
    pub user_id: Uuid,
}

#[derive(Debug, sqlx::FromRow)]
struct BootstrapUser {
    id: Uuid,
}

pub async fn bootstrap_admin_user_id(state: &AppState) -> Result<Uuid, AppError> {
    let email =
        state
            .config
            .bootstrap_admin_email
            .as_deref()
            .ok_or(AppError::ServiceUnavailable(
                "bootstrap_admin_not_configured",
            ))?;

    let user =
        query_as::<_, BootstrapUser>("SELECT id FROM users WHERE email = $1 AND disabled = false")
            .bind(email)
            .fetch_optional(&state.pool)
            .await
            .map_err(anyhow::Error::from)?
            .ok_or(AppError::ServiceUnavailable("bootstrap_admin_missing"))?;

    Ok(user.id)
}

pub async fn create_oauth_state(state: &AppState, user_id: Uuid) -> Result<String, AppError> {
    let csrf_state = Uuid::new_v4().to_string();

    query("INSERT INTO oauth_state (state, user_id, oauth_type) VALUES ($1, $2, 'advertiser')")
        .bind(&csrf_state)
        .bind(user_id)
        .execute(&state.pool)
        .await
        .map_err(anyhow::Error::from)?;

    Ok(csrf_state)
}

pub async fn consume_oauth_state(
    state: &AppState,
    csrf_state: &str,
) -> Result<ConsumedOauthState, AppError> {
    let row = query(
        r#"
        UPDATE oauth_state
        SET used_at = now()
        WHERE state = $1
          AND used_at IS NULL
          AND expires_at > now()
        RETURNING user_id
        "#,
    )
    .bind(csrf_state)
    .fetch_optional(&state.pool)
    .await
    .map_err(anyhow::Error::from)?;

    if let Some(row) = row {
        return Ok(ConsumedOauthState {
            user_id: row.get("user_id"),
        });
    }

    let exists = query("SELECT expires_at, used_at FROM oauth_state WHERE state = $1")
        .bind(csrf_state)
        .fetch_optional(&state.pool)
        .await
        .map_err(anyhow::Error::from)?;

    match exists {
        None => Err(AppError::oauth_state_invalid()),
        Some(row) => {
            let used_at: Option<DateTime<Utc>> = row.try_get("used_at").ok();
            let expires_at: DateTime<Utc> = row
                .try_get("expires_at")
                .context("oauth_state.expires_at missing")?;

            if used_at.is_some() {
                Err(AppError::oauth_state_invalid())
            } else if expires_at <= Utc::now() {
                Err(AppError::oauth_state_expired())
            } else {
                Err(AppError::oauth_state_invalid())
            }
        }
    }
}

pub async fn upsert_accounts(
    state: &AppState,
    user_id: Uuid,
    tokens: &TokenBundle,
    advertisers: &[AdvertiserAccount],
) -> Result<(), AppError> {
    let access_token_enc = state.aead.encrypt(tokens.access_token.as_bytes())?;
    let refresh_token_enc = state.aead.encrypt(tokens.refresh_token.as_bytes())?;

    for advertiser in advertisers {
        query(
            r#"
            INSERT INTO ks_accounts (
                user_id,
                advertiser_id,
                advertiser_name,
                access_token_enc,
                refresh_token_enc,
                access_expires_at,
                refresh_expires_at,
                balance,
                last_refreshed_at,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, now(), now())
            ON CONFLICT (user_id, advertiser_id) DO UPDATE
            SET advertiser_name = EXCLUDED.advertiser_name,
                access_token_enc = EXCLUDED.access_token_enc,
                refresh_token_enc = EXCLUDED.refresh_token_enc,
                access_expires_at = EXCLUDED.access_expires_at,
                refresh_expires_at = EXCLUDED.refresh_expires_at,
                balance = EXCLUDED.balance,
                last_refreshed_at = now(),
                updated_at = now()
            "#,
        )
        .bind(user_id)
        .bind(&advertiser.advertiser_id)
        .bind(&advertiser.advertiser_name)
        .bind(&access_token_enc)
        .bind(&refresh_token_enc)
        .bind(tokens.access_expires_at)
        .bind(tokens.refresh_expires_at)
        .bind(advertiser.balance)
        .execute(&state.pool)
        .await
        .map_err(anyhow::Error::from)?;
    }

    tracing::info!(
        advertiser_count = advertisers.len(),
        access_token = %redact_token_tail(&tokens.access_token),
        refresh_token = %redact_token_tail(&tokens.refresh_token),
        "stored encrypted kuaishou advertiser tokens"
    );

    Ok(())
}
