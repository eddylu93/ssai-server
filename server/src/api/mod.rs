pub mod health_api;

use axum::{routing::get, Router};

use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_api::health))
        .with_state(state)
}
