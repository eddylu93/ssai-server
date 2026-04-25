use serde::{Deserialize, Serialize};

use crate::{
    error::AppError,
    kuaishou::client::{decode_envelope, KsError, ACCESS_TOKEN_HEADER},
    state::AppState,
};

#[derive(Debug, Clone)]
pub struct AuthorizedAdvertiserPage {
    pub details: Vec<String>,
    pub is_end: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdvertiserInfo {
    pub user_name: Option<String>,
    pub corporation_name: Option<String>,
    pub product_name: Option<String>,
}

impl AdvertiserInfo {
    pub fn advertiser_name(&self) -> Option<String> {
        self.product_name
            .clone()
            .or_else(|| self.user_name.clone())
            .or_else(|| self.corporation_name.clone())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdvertiserFund {
    pub balance: f64,
}

#[derive(Debug, Clone)]
pub struct UnitReportRow {
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

#[derive(Debug, Serialize)]
struct ApprovalListRequest<'a> {
    app_id: &'a str,
    secret: &'a str,
    access_token: &'a str,
    page_no: i32,
    page_size: i32,
}

#[derive(Debug, Deserialize)]
struct ApprovalListResponse {
    #[serde(default, deserialize_with = "deserialize_id_list")]
    details: Vec<String>,
    #[serde(rename = "isEnd")]
    is_end: bool,
}

#[derive(Debug, Serialize)]
struct AdvertiserInfoRequest {
    advertiser_id: i64,
}

#[derive(Debug, Serialize)]
struct UnitReportRequest {
    advertiser_id: i64,
    start_date: String,
    end_date: String,
    temporal_granularity: &'static str,
    page: i32,
    page_size: i32,
}

#[derive(Debug, Deserialize)]
struct UnitReportEnvelope {
    details: Vec<UnitReportDetail>,
}

#[derive(Debug, Deserialize)]
struct UnitReportDetail {
    charge: Option<f64>,
    show: Option<i64>,
    photo_click: Option<i64>,
    photo_click_ratio: Option<f64>,
    form_count: Option<i64>,
    event_register: Option<i64>,
    event_pay: Option<i64>,
    event_order_submit: Option<i64>,
    event_form_submit: Option<i64>,
    submit: Option<i64>,
    bclick: Option<i64>,
    event_pay_purchase_amount: Option<f64>,
    event_pay_purchase_amount_first_day: Option<f64>,
    event_pay_roi: Option<f64>,
    event_pay_first_day_roi: Option<f64>,
    conversion_cost: Option<f64>,
    form_cost: Option<f64>,
    unit_id: i64,
}

pub async fn list_authorized_advertiser_ids(
    state: &AppState,
    access_token: &str,
    page_no: i32,
    page_size: i32,
) -> Result<AuthorizedAdvertiserPage, AppError> {
    let response = state
        .http
        .post(format!(
            "{}/oauth2/authorize/approval/list",
            state.config.ks_base_url
        ))
        .json(&ApprovalListRequest {
            app_id: &state.config.ks_app_id,
            secret: &state.config.ks_app_secret,
            access_token,
            page_no,
            page_size,
        })
        .send()
        .await
        .map_err(|_| AppError::ServiceUnavailable("kuaishou_transport_error"))?;

    let body = response
        .text()
        .await
        .map_err(|_| AppError::ServiceUnavailable("kuaishou_transport_error"))?;

    let decoded: ApprovalListResponse = decode_envelope(&body).map_err(map_ks_error)?;
    Ok(AuthorizedAdvertiserPage {
        details: decoded.details,
        is_end: decoded.is_end,
    })
}

pub async fn get_advertiser_info(
    state: &AppState,
    access_token: &str,
    advertiser_id: &str,
) -> Result<AdvertiserInfo, AppError> {
    let advertiser_id = parse_advertiser_id(advertiser_id)?;
    let response = state
        .http
        .post(format!("{}/v1/advertiser/info", state.config.ks_base_url))
        .header(ACCESS_TOKEN_HEADER, access_token)
        .json(&AdvertiserInfoRequest { advertiser_id })
        .send()
        .await
        .map_err(|_| AppError::ServiceUnavailable("kuaishou_transport_error"))?;

    let body = response
        .text()
        .await
        .map_err(|_| AppError::ServiceUnavailable("kuaishou_transport_error"))?;

    decode_envelope(&body).map_err(map_ks_error)
}

pub async fn get_advertiser_fund(
    state: &AppState,
    access_token: &str,
    advertiser_id: &str,
) -> Result<AdvertiserFund, AppError> {
    let advertiser_id = parse_advertiser_id(advertiser_id)?;
    let response = state
        .http
        .get(format!(
            "{}/v1/advertiser/fund/get",
            state.config.ks_base_url
        ))
        .header(ACCESS_TOKEN_HEADER, access_token)
        .query(&[("advertiser_id", advertiser_id)])
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
    let advertiser_id = parse_advertiser_id(advertiser_id)?;
    let request = UnitReportRequest {
        advertiser_id,
        start_date: normalize_report_date(start),
        end_date: normalize_report_date(end),
        temporal_granularity: "HOURLY",
        page: 1,
        page_size: 2000,
    };

    let response = state
        .http
        .post(format!(
            "{}/v1/report/unit_report",
            state.config.ks_base_url
        ))
        .header(ACCESS_TOKEN_HEADER, access_token)
        .json(&request)
        .send()
        .await
        .map_err(|_| AppError::ServiceUnavailable("kuaishou_transport_error"))?;

    let body = response
        .text()
        .await
        .map_err(|_| AppError::ServiceUnavailable("kuaishou_transport_error"))?;

    let decoded: UnitReportEnvelope = decode_envelope(&body).map_err(map_ks_error)?;
    Ok(decoded
        .details
        .into_iter()
        .map(|detail| {
            let cost = detail.charge.unwrap_or_default();
            let impressions = detail.show.unwrap_or_default();
            let clicks = detail.photo_click.unwrap_or_default();
            let conversions = first_nonzero_i64(&[
                detail.form_count,
                detail.event_register,
                detail.event_pay,
                detail.event_order_submit,
                detail.event_form_submit,
                detail.submit,
                detail.bclick,
            ]);
            let revenue = detail
                .event_pay_purchase_amount
                .or(detail.event_pay_purchase_amount_first_day)
                .unwrap_or_default();
            let roi = detail
                .event_pay_roi
                .or(detail.event_pay_first_day_roi)
                .unwrap_or_default();
            let ctr = detail
                .photo_click_ratio
                .unwrap_or_else(|| ratio(clicks, impressions));
            let cvr = ratio(conversions, clicks);
            let cpa = detail
                .conversion_cost
                .or(detail.form_cost)
                .unwrap_or_else(|| unit_cost(cost, conversions));

            UnitReportRow {
                level: "unit".to_string(),
                ref_id: detail.unit_id.to_string(),
                cost,
                impressions,
                clicks,
                conversions,
                revenue,
                roi,
                ctr,
                cvr,
                cpa,
            }
        })
        .collect())
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

fn parse_advertiser_id(advertiser_id: &str) -> Result<i64, AppError> {
    advertiser_id
        .parse::<i64>()
        .map_err(|_| AppError::BadRequest("kuaishou_invalid_advertiser_id"))
}

fn normalize_report_date(value: &str) -> String {
    value.chars().take(10).collect()
}

fn ratio(numerator: i64, denominator: i64) -> f64 {
    if denominator <= 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn unit_cost(cost: f64, count: i64) -> f64 {
    if count <= 0 {
        0.0
    } else {
        cost / count as f64
    }
}

fn first_nonzero_i64(values: &[Option<i64>]) -> i64 {
    values
        .iter()
        .flatten()
        .copied()
        .find(|value| *value > 0)
        .unwrap_or_default()
}

fn deserialize_id_list<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let values = Vec::<serde_json::Value>::deserialize(deserializer)?;
    Ok(values
        .into_iter()
        .filter_map(|value| match value {
            serde_json::Value::String(value) => Some(value),
            serde_json::Value::Number(value) => Some(value.to_string()),
            _ => None,
        })
        .collect())
}
