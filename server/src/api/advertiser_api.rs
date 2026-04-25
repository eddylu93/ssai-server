use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;

use crate::{error::AppError, state::AppState};

#[derive(Debug, Serialize, FromRow)]
pub struct AdvertiserListItem {
    pub advertiser_id: String,
    pub advertiser_name: String,
    pub group_tag: Option<String>,
    pub balance: f64,
    pub daily_budget: f64,
    pub status: String,
    pub last_refreshed_at: Option<DateTime<Utc>>,
    pub access_expires_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct LatestSnapshotResponse {
    pub snapshot_at: DateTime<Utc>,
    pub level: String,
    pub ref_id: String,
    pub cost: f64,
    pub impressions: i64,
    pub clicks: i64,
    pub conversions: i64,
    pub revenue: f64,
    pub roi: f64,
    pub ctr: f64,
    pub cvr: f64,
    pub cpa: f64,
}

pub async fn list_advertisers(
    State(state): State<AppState>,
) -> Result<Json<Vec<AdvertiserListItem>>, AppError> {
    let Some(email) = state.config.bootstrap_admin_email.as_deref() else {
        return Ok(Json(Vec::new()));
    };

    let rows = sqlx::query_as::<_, AdvertiserListItem>(
        r#"
        SELECT
            ks.advertiser_id,
            ks.advertiser_name,
            ks.group_tag,
            ks.balance::float8 AS balance,
            ks.daily_budget::float8 AS daily_budget,
            ks.status,
            ks.last_refreshed_at,
            ks.access_expires_at
        FROM ks_accounts ks
        INNER JOIN users u ON u.id = ks.user_id
        WHERE u.email = $1 AND u.disabled = false
        ORDER BY ks.advertiser_name ASC
        "#,
    )
    .bind(email)
    .fetch_all(&state.pool)
    .await
    .map_err(anyhow::Error::from)?;

    Ok(Json(rows))
}

pub async fn latest_snapshot(
    State(state): State<AppState>,
    Path(advertiser_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let Some(email) = state.config.bootstrap_admin_email.as_deref() else {
        return Ok(StatusCode::NO_CONTENT.into_response());
    };

    let row = sqlx::query_as::<_, LatestSnapshotResponse>(
        r#"
        SELECT
            ms.snapshot_at,
            ms.level,
            ms.ref_id,
            ms.cost::float8 AS cost,
            ms.impressions,
            ms.clicks,
            ms.conversions,
            ms.revenue::float8 AS revenue,
            ms.roi::float8 AS roi,
            ms.ctr::float8 AS ctr,
            ms.cvr::float8 AS cvr,
            ms.cpa::float8 AS cpa
        FROM metric_snapshots ms
        INNER JOIN ks_accounts ks ON ks.id = ms.account_id
        INNER JOIN users u ON u.id = ks.user_id
        WHERE ks.advertiser_id = $1
          AND u.email = $2
          AND u.disabled = false
        ORDER BY ms.snapshot_at DESC
        LIMIT 1
        "#,
    )
    .bind(&advertiser_id)
    .bind(email)
    .fetch_optional(&state.pool)
    .await
    .map_err(anyhow::Error::from)?;

    match row {
        Some(snapshot) => Ok(Json(snapshot).into_response()),
        None => Ok(StatusCode::NO_CONTENT.into_response()),
    }
}
