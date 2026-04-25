use axum::{extract::State, Json};
use chrono::Utc;
use serde::Serialize;
use sqlx::query_scalar;

use crate::{error::AppError, state::AppState};

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: String,
    pub ts: String,
}

pub async fn health(State(state): State<AppState>) -> Result<Json<HealthResponse>, AppError> {
    query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .map_err(anyhow::Error::from)?;

    Ok(Json(HealthResponse {
        status: "ok",
        version: state.version,
        ts: Utc::now().to_rfc3339(),
    }))
}
