use std::{borrow::Cow, collections::BTreeSet};

use chrono::{Duration, Utc};
use reqwest::Url;
use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    error::AppError,
    kuaishou::{
        client::{decode_envelope, KsError},
        endpoints,
    },
    state::AppState,
};

const AUTHORIZE_URL: &str = "https://developers.e.kuaishou.com/tools/authorize";
const ADVERTISER_SCOPES: [&str; 7] = [
    "esp_ad_query",
    "esp_ad_manage",
    "esp_report_service",
    "esp_account_service",
    "public_dmp_service",
    "public_agent_service",
    "public_account_service",
];

#[derive(Debug, Clone)]
pub struct TokenBundle {
    pub access_token: String,
    pub refresh_token: String,
    pub access_expires_at: chrono::DateTime<Utc>,
    pub refresh_expires_at: chrono::DateTime<Utc>,
    pub advertiser_id: Option<String>,
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
    access_token_expires_in: i64,
    refresh_token_expires_in: i64,
    #[serde(default, deserialize_with = "deserialize_optional_id")]
    advertiser_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct AccessTokenRequest<'a> {
    app_id: &'a str,
    secret: &'a str,
    auth_code: &'a str,
}

#[derive(Debug, Serialize)]
struct RefreshTokenRequest<'a> {
    app_id: &'a str,
    secret: &'a str,
    refresh_token: &'a str,
}

pub fn build_authorize_url(state: &AppState, csrf_state: &str) -> Result<String, AppError> {
    let mut url = Url::parse(AUTHORIZE_URL).map_err(anyhow::Error::from)?;
    let scope = serde_json::to_string(&ADVERTISER_SCOPES).map_err(anyhow::Error::from)?;

    url.query_pairs_mut()
        .append_pair("app_id", &state.config.ks_app_id)
        .append_pair("scope", &scope)
        .append_pair("redirect_uri", &state.config.ks_redirect_uri)
        .append_pair("state", csrf_state)
        .append_pair("oauth_type", "advertiser");

    Ok(url.to_string())
}

pub async fn exchange_access_token(
    state: &AppState,
    auth_code: &str,
) -> Result<TokenBundle, AppError> {
    let decoded = request_token_bundle(
        state,
        "/oauth2/authorize/access_token",
        &AccessTokenRequest {
            app_id: &state.config.ks_app_id,
            secret: &state.config.ks_app_secret,
            auth_code,
        },
    )
    .await?;

    Ok(to_token_bundle(decoded))
}

pub async fn refresh_token(state: &AppState, refresh_token: &str) -> Result<TokenBundle, AppError> {
    let decoded = request_token_bundle(
        state,
        "/oauth2/authorize/refresh_token",
        &RefreshTokenRequest {
            app_id: &state.config.ks_app_id,
            secret: &state.config.ks_app_secret,
            refresh_token,
        },
    )
    .await?;

    Ok(to_token_bundle(decoded))
}

pub async fn fetch_authorized_accounts(
    state: &AppState,
    tokens: &TokenBundle,
) -> Result<Vec<AdvertiserAccount>, AppError> {
    let mut advertiser_ids = BTreeSet::new();
    let mut page_no = 1;

    loop {
        let page =
            endpoints::list_authorized_advertiser_ids(state, &tokens.access_token, page_no, 200)
                .await?;

        for advertiser_id in page.details {
            advertiser_ids.insert(advertiser_id);
        }

        if page.is_end {
            break;
        }

        page_no += 1;
    }

    if let Some(primary_advertiser_id) = &tokens.advertiser_id {
        advertiser_ids.insert(primary_advertiser_id.clone());
    }

    if advertiser_ids.is_empty() {
        return Err(AppError::ServiceUnavailable(
            "kuaishou_authorized_accounts_missing",
        ));
    }

    let mut accounts = Vec::with_capacity(advertiser_ids.len());
    for advertiser_id in advertiser_ids {
        let info =
            endpoints::get_advertiser_info(state, &tokens.access_token, &advertiser_id).await;
        let fund =
            endpoints::get_advertiser_fund(state, &tokens.access_token, &advertiser_id).await;

        let advertiser_name = info
            .as_ref()
            .ok()
            .and_then(|item| item.advertiser_name())
            .unwrap_or_else(|| format!("Advertiser {advertiser_id}"));
        let balance = fund.ok().map(|item| item.balance).unwrap_or_default();

        accounts.push(AdvertiserAccount {
            advertiser_id,
            advertiser_name,
            balance,
        });
    }

    Ok(accounts)
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

async fn request_token_bundle<T: Serialize>(
    state: &AppState,
    path: &str,
    body: &T,
) -> Result<TokenResponse, AppError> {
    let response = state
        .http
        .post(format!("{}{}", state.config.ks_base_url, path))
        .json(body)
        .send()
        .await
        .map_err(|_| AppError::ServiceUnavailable("kuaishou_transport_error"))?;

    let body = response
        .text()
        .await
        .map_err(|_| AppError::ServiceUnavailable("kuaishou_transport_error"))?;

    decode_envelope(&body).map_err(map_ks_error)
}

fn to_token_bundle(decoded: TokenResponse) -> TokenBundle {
    TokenBundle {
        access_token: decoded.access_token,
        refresh_token: decoded.refresh_token,
        access_expires_at: Utc::now() + Duration::seconds(decoded.access_token_expires_in),
        refresh_expires_at: Utc::now() + Duration::seconds(decoded.refresh_token_expires_in),
        advertiser_id: decoded.advertiser_id,
    }
}

fn deserialize_optional_id<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value.and_then(|item| match item {
        serde_json::Value::Null => None,
        serde_json::Value::String(value) => Some(value),
        serde_json::Value::Number(value) => Some(value.to_string()),
        _ => None,
    }))
}
