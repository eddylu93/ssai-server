mod common;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use chrono::{Duration, Utc};
use serde_json::Value;
use sqlx::query;
use tower::ServiceExt;

#[tokio::test]
async fn snapshot_latest_returns_204_when_no_data() {
    let email = format!("test-no-snapshot-{}@example.com", uuid::Uuid::new_v4());
    let state = common::test_state(&email).await.expect("state");
    let user_id = common::insert_bootstrap_admin(&state.pool, &email)
        .await
        .expect("admin");

    query(
        r#"
        INSERT INTO ks_accounts (
            id, user_id, advertiser_id, advertiser_name,
            access_token_enc, refresh_token_enc, access_expires_at, refresh_expires_at
        )
        VALUES ($1, $2, 'adv-204', 'Advertiser Empty', '\x0001', '\x0002', now() + interval '1 day', now() + interval '30 days')
        "#,
    )
    .bind(uuid::Uuid::new_v4())
    .bind(user_id)
    .execute(&state.pool)
    .await
    .expect("insert account");

    let app = ssai_server::api::router(state.clone());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/advertisers/adv-204/snapshot/latest")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    common::cleanup_user(&state.pool, user_id, &email)
        .await
        .expect("cleanup");
}

#[tokio::test]
async fn snapshot_latest_returns_newest_row() {
    let email = format!("test-latest-{}@example.com", uuid::Uuid::new_v4());
    let state = common::test_state(&email).await.expect("state");
    let user_id = common::insert_bootstrap_admin(&state.pool, &email)
        .await
        .expect("admin");
    let account_id = uuid::Uuid::new_v4();
    let older = Utc::now() - Duration::hours(2);
    let newer = Utc::now() - Duration::hours(1);

    query(
        r#"
        INSERT INTO ks_accounts (
            id, user_id, advertiser_id, advertiser_name,
            access_token_enc, refresh_token_enc, access_expires_at, refresh_expires_at
        )
        VALUES ($1, $2, 'adv-latest', 'Advertiser Latest', '\x0001', '\x0002', now() + interval '1 day', now() + interval '30 days')
        "#,
    )
    .bind(account_id)
    .bind(user_id)
    .execute(&state.pool)
    .await
    .expect("insert account");

    for (snapshot_at, cost) in [(older, 11.0_f64), (newer, 22.0_f64)] {
        query(
            r#"
            INSERT INTO metric_snapshots (
                account_id, level, ref_id, snapshot_at,
                cost, impressions, clicks, conversions, revenue, roi, ctr, cvr, cpa
            )
            VALUES ($1, 'unit', 'unit-1', $2, $3, 100, 10, 1, 50, 2.5, 0.1, 0.1, 11)
            "#,
        )
        .bind(account_id)
        .bind(snapshot_at)
        .bind(cost)
        .execute(&state.pool)
        .await
        .expect("insert snapshot");
    }

    let app = ssai_server::api::router(state.clone());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/advertisers/adv-latest/snapshot/latest")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let value: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(value["level"], "unit");
    assert_eq!(value["ref_id"], "unit-1");
    assert_eq!(value["cost"], 22.0);
    assert_eq!(value["impressions"], 100);

    common::cleanup_user(&state.pool, user_id, &email)
        .await
        .expect("cleanup");
}
