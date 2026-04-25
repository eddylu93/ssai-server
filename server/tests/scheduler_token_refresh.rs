mod common;

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use axum::{extract::State, routing::post, Json, Router};
use serde_json::json;
use sqlx::Row;
use tokio::net::TcpListener;

#[derive(Clone)]
struct MockState {
    refresh_calls: Arc<AtomicUsize>,
}

#[tokio::test]
async fn refresh_account_updates_encrypted_tokens_and_expiry() {
    let mock_state = MockState {
        refresh_calls: Arc::new(AtomicUsize::new(0)),
    };
    let base_url = start_mock_server(mock_state.clone()).await;

    let email = format!("refresh-{}@example.com", uuid::Uuid::new_v4());
    let state = common::test_state_with_base_url(&email, base_url)
        .await
        .expect("state");
    let user_id = common::insert_bootstrap_admin(&state.pool, &email)
        .await
        .expect("admin");
    let account_id = common::insert_ks_account_with_tokens(
        &state,
        user_id,
        "adv-refresh",
        "old-access",
        "old-refresh",
    )
    .await
    .expect("account");

    let before = sqlx::query(
        "SELECT access_token_enc, refresh_token_enc, access_expires_at FROM ks_accounts WHERE id = $1",
    )
    .bind(account_id)
    .fetch_one(&state.pool)
    .await
    .expect("before");
    let before_access: Vec<u8> = before.get("access_token_enc");
    let before_refresh: Vec<u8> = before.get("refresh_token_enc");

    let access_token =
        ssai_server::scheduler::token_refresh::refresh_account(state.clone(), account_id)
            .await
            .expect("refresh");

    assert_eq!(access_token, "new-access-token");
    assert_eq!(mock_state.refresh_calls.load(Ordering::SeqCst), 1);

    let after = sqlx::query(
        "SELECT access_token_enc, refresh_token_enc, access_expires_at FROM ks_accounts WHERE id = $1",
    )
    .bind(account_id)
    .fetch_one(&state.pool)
    .await
    .expect("after");
    let after_access: Vec<u8> = after.get("access_token_enc");
    let after_refresh: Vec<u8> = after.get("refresh_token_enc");
    let after_expires_at: chrono::DateTime<chrono::Utc> = after.get("access_expires_at");

    assert_ne!(before_access, after_access);
    assert_ne!(before_refresh, after_refresh);
    assert!(after_expires_at > chrono::Utc::now() + chrono::Duration::hours(23));

    let decrypted = state.aead.decrypt(&after_access).expect("decrypt");
    assert_eq!(
        String::from_utf8(decrypted).expect("utf8"),
        "new-access-token"
    );

    common::cleanup_user(&state.pool, user_id, &email)
        .await
        .expect("cleanup");
}

async fn start_mock_server(mock_state: MockState) -> String {
    let app = Router::new()
        .route(
            "/oauth2/authorize/refresh_token",
            post(refresh_token_handler),
        )
        .with_state(mock_state);

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });

    format!("http://{}", addr)
}

async fn refresh_token_handler(State(state): State<MockState>) -> Json<serde_json::Value> {
    state.refresh_calls.fetch_add(1, Ordering::SeqCst);
    Json(json!({
        "code": 0,
        "message": "ok",
        "data": {
            "access_token": "new-access-token",
            "refresh_token": "new-refresh-token",
            "expires_in": 86400,
            "refresh_token_expires_in": 2592000
        }
    }))
}
