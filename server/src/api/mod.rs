pub mod health_api;
pub mod oauth_api;

use axum::{routing::get, Router};

use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_api::health))
        .route(
            "/api/kuaishou/oauth/authorize",
            get(oauth_api::authorize).post(oauth_api::authorize),
        )
        .route("/api/kuaishou/oauth/callback", get(oauth_api::callback))
        .with_state(state)
}
