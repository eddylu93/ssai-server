use std::borrow::Cow;

use chrono::{Duration, Utc};
use reqwest::Url;
use serde::{Deserialize, Serialize};

use crate::{
    error::AppError,
    kuaishou::client::{decode_envelope, KsError},
    state::AppState,
};

const AUTHORIZE_URL: &str = "https://developers.e.kuaishou.com/oauth/authorize";

#[derive(Debug, Clone)]
pub struct TokenBundle {
    pub access_token: String,
    pub refresh_token: String,
    pub access_expires_at: chrono::DateTime<Utc>,
    pub refresh_expires_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct AdvertiserAccount {
    pub advertiser_id: String,
    pub advertiser_name: String,
    pub balance: f64,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
    refresh_token_expires_in: i64,
}

#[derive(Debug, Serialize)]
struct AccessTokenRequest<'a> {
    app_id: &'a str,
    secret: &'a str,
    auth_code: &'a str,
}

#[derive(Debug, Deserialize)]
struct AccountInfoResponse {
    advertiser_id: Option<String>,
    account_id: Option<String>,
    advertiser_name: Option<String>,
    account_name: Option<String>,
    balance: Option<f64>,
}

pub fn build_authorize_url(state: &AppState, csrf_state: &str) -> Result<String, AppError> {
    let mut url = Url::parse(AUTHORIZE_URL).map_err(anyhow::Error::from)?;
    url.query_pairs_mut()
        .append_pair("app_id", &state.config.ks_app_id)
        .append_pair("scope", "report_service,account_service")
        .append_pair("redirect_uri", &state.config.ks_redirect_uri)
        .append_pair("state", csrf_state)
        .append_pair("oauth_type", "advertiser");

    Ok(url.to_string())
}

pub async fn exchange_access_token(
    state: &AppState,
    auth_code: &str,
) -> Result<TokenBundle, AppError> {
    let url = format!("{}/oauth2/authorize/access_token", state.config.ks_base_url);
    let response = state
        .http
        .post(url)
        .json(&AccessTokenRequest {
            app_id: &state.config.ks_app_id,
            secret: &state.config.ks_app_secret,
            auth_code,
        })
        .send()
        .await
        .map_err(|_| AppError::ServiceUnavailable("kuaishou_transport_error"))?;

    let body = response
        .text()
        .await
        .map_err(|_| AppError::ServiceUnavailable("kuaishou_transport_error"))?;

    let decoded: TokenResponse = decode_envelope(&body).map_err(map_ks_error)?;

    Ok(TokenBundle {
        access_token: decoded.access_token,
        refresh_token: decoded.refresh_token,
        access_expires_at: Utc::now() + Duration::seconds(decoded.expires_in),
        refresh_expires_at: Utc::now() + Duration::seconds(decoded.refresh_token_expires_in),
    })
}

pub async fn fetch_account_info(
    state: &AppState,
    access_token: &str,
) -> Result<Vec<AdvertiserAccount>, AppError> {
    let url = format!("{}/v2/account/info", state.config.ks_base_url);
    let response = state
        .http
        .post(url)
        .header("Access-Token", access_token)
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(|_| AppError::ServiceUnavailable("kuaishou_transport_error"))?;

    let body = response
        .text()
        .await
        .map_err(|_| AppError::ServiceUnavailable("kuaishou_transport_error"))?;

    let decoded: AccountInfoResponse = decode_envelope(&body).map_err(map_ks_error)?;
    let advertiser_id = decoded
        .advertiser_id
        .or(decoded.account_id)
        .ok_or(AppError::ServiceUnavailable("kuaishou_account_id_missing"))?;
    let advertiser_name = decoded
        .advertiser_name
        .or(decoded.account_name)
        .unwrap_or_else(|| "Kuaishou Advertiser".to_string());

    Ok(vec![AdvertiserAccount {
        advertiser_id,
        advertiser_name,
        balance: decoded.balance.unwrap_or_default(),
    }])
}

pub fn redact_token_tail(token: &str) -> Cow<'static, str> {
    if token.len() <= 4 {
        Cow::Borrowed("***")
    } else {
        Cow::Owned(format!("***{}", &token[token.len() - 4..]))
    }
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
