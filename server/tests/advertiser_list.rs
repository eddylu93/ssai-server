mod common;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::Value;
use sqlx::query;
use tower::ServiceExt;

#[tokio::test]
async fn get_advertisers_returns_empty_array_when_no_accounts() {
    let email = format!("test-empty-{}@example.com", uuid::Uuid::new_v4());
    let state = common::test_state(&email).await.expect("state");
    let user_id = common::insert_bootstrap_admin(&state.pool, &email)
        .await
        .expect("admin");

    let app = ssai_server::api::router(state.clone());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/advertisers")
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
    assert_eq!(value, Value::Array(vec![]));

    common::cleanup_user(&state.pool, user_id, &email)
        .await
        .expect("cleanup");
}

#[tokio::test]
async fn get_advertisers_returns_bootstrap_admin_accounts() {
    let email = format!("test-accounts-{}@example.com", uuid::Uuid::new_v4());
    let state = common::test_state(&email).await.expect("state");
    let user_id = common::insert_bootstrap_admin(&state.pool, &email)
        .await
        .expect("admin");

    query(
        r#"
        INSERT INTO ks_accounts (
            id, user_id, advertiser_id, advertiser_name, group_tag,
            access_token_enc, refresh_token_enc, access_expires_at, refresh_expires_at,
            balance, daily_budget, status, last_refreshed_at
        )
        VALUES (
            $1, $2, 'adv-001', 'Advertiser One', 'group-a',
            '\x0001', '\x0002', now() + interval '1 day', now() + interval '30 days',
            12.34, 56.78, 'normal', now()
        )
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
                .uri("/api/advertisers")
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
    let items = value.as_array().expect("array");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["advertiser_id"], "adv-001");
    assert_eq!(items[0]["advertiser_name"], "Advertiser One");
    assert_eq!(items[0]["group_tag"], "group-a");
    assert_eq!(items[0]["balance"], 12.34);
    assert_eq!(items[0]["daily_budget"], 56.78);
    assert_eq!(items[0]["status"], "normal");

    common::cleanup_user(&state.pool, user_id, &email)
        .await
        .expect("cleanup");
}
