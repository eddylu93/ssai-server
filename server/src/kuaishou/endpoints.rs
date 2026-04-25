use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    error::AppError,
    kuaishou::client::{decode_envelope, KsError},
    state::AppState,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KuaishouAdvertiser {
    pub advertiser_id: String,
    pub advertiser_name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UnitReportRow {
    pub level: String,
    pub ref_id: String,
    pub snapshot_at: DateTime<Utc>,
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

#[derive(Debug, Serialize)]
struct UnitReportRequest<'a> {
    advertiser_id: &'a str,
    start_date: &'a str,
    end_date: &'a str,
    granularity: &'static str,
    filtering: UnitReportFiltering<'a>,
}

#[derive(Debug, Serialize)]
struct UnitReportFiltering<'a> {
    level: &'static str,
    advertiser_id: &'a str,
}

pub async fn get_advertiser_list(
    state: &AppState,
    access_token: &str,
) -> Result<Vec<KuaishouAdvertiser>, AppError> {
    let response = state
        .http
        .post(format!("{}/v2/account/list", state.config.ks_base_url))
        .header("Access-Token", access_token)
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(|_| AppError::ServiceUnavailable("kuaishou_transport_error"))?;

    let body = response
        .text()
        .await
        .map_err(|_| AppError::ServiceUnavailable("kuaishou_transport_error"))?;

    decode_envelope(&body).map_err(map_ks_error)
}

pub async fn get_unit_report(
    state: &AppState,
    access_token: &str,
    advertiser_id: &str,
    start: &str,
    end: &str,
) -> Result<Vec<UnitReportRow>, AppError> {
    let response = state
        .http
        .post(format!(
            "{}/v2/report/unit_report",
            state.config.ks_base_url
        ))
        .header("Access-Token", access_token)
        .json(&UnitReportRequest {
            advertiser_id,
            start_date: start,
            end_date: end,
            granularity: "hour",
            filtering: UnitReportFiltering {
                level: "unit",
                advertiser_id,
            },
        })
        .send()
        .await
        .map_err(|_| AppError::ServiceUnavailable("kuaishou_transport_error"))?;

    let body = response
        .text()
        .await
        .map_err(|_| AppError::ServiceUnavailable("kuaishou_transport_error"))?;

    decode_envelope(&body).map_err(map_ks_error)
}

fn map_ks_error(error: KsError) -> AppError {
    match error {
        KsError::TokenExpired => AppError::Unauthorized("kuaishou_token_expired"),
        KsError::RateLimited => AppError::ServiceUnavailable("kuaishou_rate_limited"),
        KsError::UpstreamServer => AppError::ServiceUnavailable("kuaishou_upstream_server"),
        KsError::Business => AppError::BadRequest("kuaishou_business_error"),
        KsError::Decode => AppError::ServiceUnavailable("kuaishou_decode_error"),
    }
}
