use std::sync::Arc;

use anyhow::Context;
use chrono::{Datelike, TimeZone, Timelike, Utc};
use sqlx::FromRow;
use tokio::{sync::Semaphore, task::JoinSet};
use uuid::Uuid;

use crate::{
    kuaishou::endpoints::{self, UnitReportRow},
    scheduler::token_refresh,
    state::AppState,
};

#[derive(Debug, FromRow, Clone)]
struct AccountRow {
    id: Uuid,
    advertiser_id: String,
    access_token_enc: Vec<u8>,
}

#[derive(Debug, Default, Clone, Copy)]
struct AccountRunResult {
    snapshots_written: usize,
    errors: usize,
}

pub async fn run_once(state: AppState) -> anyhow::Result<()> {
    let accounts = sqlx::query_as::<_, AccountRow>(
        r#"
        SELECT id, advertiser_id, access_token_enc
        FROM ks_accounts
        WHERE status = 'normal'
        ORDER BY advertiser_id ASC
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .context("failed to load accounts for unit report job")?;

    if accounts.is_empty() {
        tracing::info!(
            "unit_report_job completed: accounts_processed=0 snapshots_written=0 errors=0"
        );
        return Ok(());
    }

    let slot = floor_to_15min(Utc::now());
    let start = Utc
        .with_ymd_and_hms(slot.year(), slot.month(), slot.day(), 0, 0, 0)
        .single()
        .context("failed to compute report start time")?;
    let start_date = start.format("%Y-%m-%d %H:%M:%S").to_string();
    let end_date = slot.format("%Y-%m-%d %H:%M:%S").to_string();

    let semaphore = Arc::new(Semaphore::new(5));
    let mut join_set = JoinSet::new();

    for account in accounts {
        let permit = semaphore.clone().acquire_owned().await?;
        let state = state.clone();
        let start_date = start_date.clone();
        let end_date = end_date.clone();

        join_set.spawn(async move {
            let _permit = permit;
            let result = run_account(state, account, &start_date, &end_date, slot).await;
            result.unwrap_or(AccountRunResult {
                snapshots_written: 0,
                errors: 1,
            })
        });
    }

    let mut accounts_processed = 0usize;
    let mut snapshots_written = 0usize;
    let mut errors = 0usize;

    while let Some(joined) = join_set.join_next().await {
        accounts_processed += 1;
        match joined {
            Ok(result) => {
                snapshots_written += result.snapshots_written;
                errors += result.errors;
            }
            Err(_) => {
                errors += 1;
            }
        }
    }

    tracing::info!(
        accounts_processed,
        snapshots_written,
        errors,
        "unit_report_job completed"
    );

    Ok(())
}

async fn run_account(
    state: AppState,
    account: AccountRow,
    start_date: &str,
    end_date: &str,
    slot: chrono::DateTime<Utc>,
) -> anyhow::Result<AccountRunResult> {
    let access_token = decrypt_access_token(&state, &account.access_token_enc)?;

    let rows = match endpoints::get_unit_report(
        &state,
        &access_token,
        &account.advertiser_id,
        start_date,
        end_date,
    )
    .await
    {
        Ok(rows) => rows,
        Err(crate::error::AppError::Unauthorized("kuaishou_token_expired")) => {
            let refreshed = token_refresh::refresh_account(state.clone(), account.id)
                .await
                .context("failed to refresh expired access token")?;

            match endpoints::get_unit_report(
                &state,
                &refreshed,
                &account.advertiser_id,
                start_date,
                end_date,
            )
            .await
            {
                Ok(rows) => rows,
                Err(error) => {
                    mark_auth_error(&state, account.id).await?;
                    tracing::error!(account_id = %account.id, advertiser_id = %account.advertiser_id, error = %error, "unit report retry failed after refresh");
                    return Ok(AccountRunResult {
                        snapshots_written: 0,
                        errors: 1,
                    });
                }
            }
        }
        Err(error) => {
            tracing::error!(account_id = %account.id, advertiser_id = %account.advertiser_id, error = %error, "unit report fetch failed");
            return Ok(AccountRunResult {
                snapshots_written: 0,
                errors: 1,
            });
        }
    };

    let mut written = 0usize;
    for row in rows {
        insert_snapshot(&state, account.id, slot, row).await?;
        written += 1;
    }

    Ok(AccountRunResult {
        snapshots_written: written,
        errors: 0,
    })
}

fn decrypt_access_token(state: &AppState, encrypted: &[u8]) -> anyhow::Result<String> {
    let decrypted = state
        .aead
        .decrypt(encrypted)
        .context("failed to decrypt access token")?;
    String::from_utf8(decrypted).context("access token is not valid utf-8")
}

async fn insert_snapshot(
    state: &AppState,
    account_id: Uuid,
    slot: chrono::DateTime<Utc>,
    row: UnitReportRow,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO metric_snapshots (
            account_id, level, ref_id, snapshot_at,
            cost, impressions, clicks, conversions, revenue, roi, ctr, cvr, cpa
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
        "#,
    )
    .bind(account_id)
    .bind(row.level)
    .bind(row.ref_id)
    .bind(slot)
    .bind(row.cost)
    .bind(row.impressions)
    .bind(row.clicks)
    .bind(row.conversions)
    .bind(row.revenue)
    .bind(row.roi)
    .bind(row.ctr)
    .bind(row.cvr)
    .bind(row.cpa)
    .execute(&state.pool)
    .await
    .context("failed to insert metric snapshot")?;

    Ok(())
}

async fn mark_auth_error(state: &AppState, account_id: Uuid) -> anyhow::Result<()> {
    sqlx::query("UPDATE ks_accounts SET status = 'auth_error', updated_at = now() WHERE id = $1")
        .bind(account_id)
        .execute(&state.pool)
        .await
        .context("failed to mark account auth_error")?;

    Ok(())
}

fn floor_to_15min(ts: chrono::DateTime<Utc>) -> chrono::DateTime<Utc> {
    let minute = (ts.minute() / 15) * 15;
    ts.with_second(0)
        .and_then(|t| t.with_nanosecond(0))
        .and_then(|t| t.with_minute(minute))
        .expect("valid timestamp")
}
