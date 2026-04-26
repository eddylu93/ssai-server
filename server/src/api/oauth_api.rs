use axum::{
    extract::{Query, State},
    response::Html,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::{
    error::AppError,
    kuaishou::{oauth, token_store},
    state::AppState,
};

#[derive(Debug, Serialize)]
pub struct AuthorizeResponse {
    pub authorize_url: String,
    pub state: String,
}

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    auth_code: String,
    state: String,
}

pub async fn authorize(State(state): State<AppState>) -> Result<Json<AuthorizeResponse>, AppError> {
    let user_id = token_store::bootstrap_admin_user_id(&state).await?;
    let state_token = token_store::create_oauth_state(&state, user_id).await?;
    let authorize_url = oauth::build_authorize_url(&state, &state_token)?;

    Ok(Json(AuthorizeResponse {
        authorize_url,
        state: state_token,
    }))
}

pub async fn callback(
    State(state): State<AppState>,
    Query(query): Query<CallbackQuery>,
) -> Result<Html<&'static str>, AppError> {
    let consumed = token_store::consume_oauth_state(&state, &query.state).await?;
    let token_bundle = oauth::exchange_access_token(&state, &query.auth_code).await?;
    let advertisers = oauth::fetch_authorized_accounts(&state, &token_bundle).await?;

    token_store::upsert_accounts(&state, consumed.user_id, &token_bundle, &advertisers).await?;

    Ok(Html("授权成功，可关闭此页。"))
}
