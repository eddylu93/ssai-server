mod common;

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use axum::{extract::State, http::HeaderMap, routing::post, Json, Router};
use serde_json::json;
use sqlx::Row;
use tokio::net::TcpListener;

#[derive(Clone)]
struct MockState {
    report_calls: Arc<AtomicUsize>,
    refresh_calls: Arc<AtomicUsize>,
    mode: MockMode,
}

#[derive(Clone, Copy)]
enum MockMode {
    Success,
    RefreshOn401,
}

#[tokio::test]
async fn run_once_with_no_accounts_returns_ok() {
    let email = format!("scheduler-empty-{}@example.com", uuid::Uuid::new_v4());
    let state = common::test_state(&email).await.expect("state");
    let user_id = common::insert_bootstrap_admin(&state.pool, &email)
        .await
        .expect("admin");

    ssai_server::scheduler::unit_report_job::run_once(state.clone())
        .await
        .expect("run once");

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM metric_snapshots WHERE account_id IN (SELECT id FROM ks_accounts WHERE user_id = $1)",
    )
        .bind(user_id)
        .fetch_one(&state.pool)
        .await
        .expect("count");
    assert_eq!(count, 0);

    common::cleanup_user(&state.pool, user_id, &email)
        .await
        .expect("cleanup");
}

#[tokio::test]
async fn run_once_writes_metric_snapshots() {
    let mock_state = MockState {
        report_calls: Arc::new(AtomicUsize::new(0)),
        refresh_calls: Arc::new(AtomicUsize::new(0)),
        mode: MockMode::Success,
    };
    let base_url = start_mock_server(mock_state.clone()).await;

    let email = format!("scheduler-success-{}@example.com", uuid::Uuid::new_v4());
    let state = common::test_state_with_base_url(&email, base_url)
        .await
        .expect("state");
    let user_id = common::insert_bootstrap_admin(&state.pool, &email)
        .await
        .expect("admin");
    let account_id = common::insert_ks_account_with_tokens(
        &state,
        user_id,
        "91207261",
        "valid-access",
        "valid-refresh",
    )
    .await
    .expect("account");

    ssai_server::scheduler::unit_report_job::run_once(state.clone())
        .await
        .expect("run once");

    let row = sqlx::query(
        "SELECT level, ref_id, cost::float8 AS cost, impressions, clicks, conversions FROM metric_snapshots WHERE account_id = $1 ORDER BY snapshot_at DESC LIMIT 1",
    )
    .bind(account_id)
    .fetch_one(&state.pool)
    .await
    .expect("snapshot");

    assert_eq!(row.get::<String, _>("level"), "unit");
    assert_eq!(row.get::<String, _>("ref_id"), "10001");
    assert_eq!(row.get::<f64, _>("cost"), 12.5);
    assert_eq!(row.get::<i64, _>("impressions"), 1000);
    assert_eq!(row.get::<i64, _>("clicks"), 30);
    assert_eq!(row.get::<i64, _>("conversions"), 3);
    assert!(mock_state.report_calls.load(Ordering::SeqCst) >= 1);
    assert_eq!(mock_state.refresh_calls.load(Ordering::SeqCst), 0);

    common::cleanup_user(&state.pool, user_id, &email)
        .await
        .expect("cleanup");
}

#[tokio::test]
async fn run_once_refreshes_on_401_and_retries_successfully() {
    let mock_state = MockState {
        report_calls: Arc::new(AtomicUsize::new(0)),
        refresh_calls: Arc::new(AtomicUsize::new(0)),
        mode: MockMode::RefreshOn401,
    };
    let base_url = start_mock_server(mock_state.clone()).await;

    let email = format!("scheduler-refresh-{}@example.com", uuid::Uuid::new_v4());
    let state = common::test_state_with_base_url(&email, base_url)
        .await
        .expect("state");
    let user_id = common::insert_bootstrap_admin(&state.pool, &email)
        .await
        .expect("admin");
    let account_id = common::insert_ks_account_with_tokens(
        &state,
        user_id,
        "91207262",
        "expired-access",
        "old-refresh",
    )
    .await
    .expect("account");

    ssai_server::scheduler::unit_report_job::run_once(state.clone())
        .await
        .expect("run once");

    let snapshots: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM metric_snapshots WHERE account_id = $1")
            .bind(account_id)
            .fetch_one(&state.pool)
            .await
            .expect("count");
    assert_eq!(snapshots, 1);
    assert!(mock_state.refresh_calls.load(Ordering::SeqCst) >= 1);
    assert!(mock_state.report_calls.load(Ordering::SeqCst) >= 2);

    let updated = sqlx::query("SELECT access_token_enc, status FROM ks_accounts WHERE id = $1")
        .bind(account_id)
        .fetch_one(&state.pool)
        .await
        .expect("updated");
    let status: String = updated.get("status");
    assert_eq!(status, "normal");
    let enc: Vec<u8> = updated.get("access_token_enc");
    let decrypted = state.aead.decrypt(&enc).expect("decrypt");
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
        .route("/v1/report/unit_report", post(unit_report_handler))
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

async fn unit_report_handler(
    State(state): State<MockState>,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    let token = headers
        .get("Access-Token")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    state.report_calls.fetch_add(1, Ordering::SeqCst);

    match state.mode {
        MockMode::Success => Json(success_report_response()),
        MockMode::RefreshOn401 if token == "expired-access" => Json(json!({
            "code": 40001,
            "message": "access_token expired",
            "data": null
        })),
        MockMode::RefreshOn401 if token == "new-access-token" => Json(success_report_response()),
        MockMode::RefreshOn401 => Json(json!({
            "code": 50000,
            "message": "unexpected token",
            "data": null
        })),
    }
}

async fn refresh_token_handler(State(state): State<MockState>) -> Json<serde_json::Value> {
    state.refresh_calls.fetch_add(1, Ordering::SeqCst);
    Json(json!({
        "code": 0,
        "message": "ok",
        "data": {
            "access_token": "new-access-token",
            "refresh_token": "new-refresh-token",
            "access_token_expires_in": 86400,
            "advertiser_id": 91207261,
            "refresh_token_expires_in": 2592000
        }
    }))
}

fn success_report_response() -> serde_json::Value {
    Json(json!({
        "code": 0,
        "message": "ok",
        "data": {
            "total_count": 1,
            "details": [
                {
                    "charge": 12.5,
                    "show": 1000,
                    "photo_click": 30,
                    "photo_click_ratio": 0.03,
                    "form_count": 3,
                    "event_pay_purchase_amount": 99.9,
                    "event_pay_roi": 7.99,
                    "form_cost": 4.17,
                    "unit_id": 10001
                }
            ]
        }
    }))
    .0
}
