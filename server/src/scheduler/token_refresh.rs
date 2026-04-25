use anyhow::Context;
use chrono::{Duration, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{kuaishou::oauth, state::AppState};

pub type DecryptedAccessToken = String;

#[derive(Debug, FromRow)]
struct RefreshableAccount {
    id: Uuid,
    refresh_token_enc: Vec<u8>,
}

pub async fn refresh_due_accounts(state: AppState) -> anyhow::Result<()> {
    let accounts = sqlx::query_as::<_, RefreshableAccount>(
        r#"
        SELECT id, refresh_token_enc
        FROM ks_accounts
        WHERE status = 'normal'
          AND access_expires_at < now() + interval '10 minute'
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .context("failed to load accounts due for refresh")?;

    for account in accounts {
        if let Err(error) = refresh_account(state.clone(), account.id).await {
            tracing::error!(account_id = %account.id, error = %error, "token refresh sweep failed");
        }
    }

    Ok(())
}

pub async fn refresh_account(
    state: AppState,
    account_id: Uuid,
) -> anyhow::Result<DecryptedAccessToken> {
    let row = sqlx::query_as::<_, RefreshableAccount>(
        "SELECT id, refresh_token_enc FROM ks_accounts WHERE id = $1",
    )
    .bind(account_id)
    .fetch_one(&state.pool)
    .await
    .context("failed to load account for token refresh")?;

    let refresh_token = state
        .aead
        .decrypt(&row.refresh_token_enc)
        .context("failed to decrypt refresh token")?;
    let refresh_token =
        String::from_utf8(refresh_token).context("refresh token is not valid utf-8")?;

    let token_bundle = oauth::refresh_token(&state, &refresh_token)
        .await
        .map_err(anyhow::Error::from)?;
    let access_token_enc = state
        .aead
        .encrypt(token_bundle.access_token.as_bytes())
        .context("failed to encrypt refreshed access token")?;
    let refresh_token_enc = state
        .aead
        .encrypt(token_bundle.refresh_token.as_bytes())
        .context("failed to encrypt refreshed refresh token")?;

    sqlx::query(
        r#"
        UPDATE ks_accounts
        SET access_token_enc = $2,
            refresh_token_enc = $3,
            access_expires_at = $4,
            refresh_expires_at = $5,
            last_refreshed_at = now(),
            updated_at = now(),
            status = 'normal'
        WHERE id = $1
        "#,
    )
    .bind(account_id)
    .bind(access_token_enc)
    .bind(refresh_token_enc)
    .bind(token_bundle.access_expires_at)
    .bind(token_bundle.refresh_expires_at)
    .execute(&state.pool)
    .await
    .context("failed to persist refreshed tokens")?;

    Ok(token_bundle.access_token)
}

pub fn refresh_cutoff() -> chrono::DateTime<Utc> {
    Utc::now() + Duration::minutes(10)
}
