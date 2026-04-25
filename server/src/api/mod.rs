pub mod advertiser_api;
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
        .route("/api/advertisers", get(advertiser_api::list_advertisers))
        .route(
            "/api/advertisers/:advertiser_id/snapshot/latest",
            get(advertiser_api::latest_snapshot),
        )
        .with_state(state)
}
