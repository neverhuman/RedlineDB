//! Integration tests for the redline-web HTTP API.
//!
//! Drives the real router via `tower::ServiceExt::oneshot`, backed by a
//! temp-file SQLite database. The connector seeds a two-table demo schema
//! (`users`, `events`) for any fresh database, which these tests rely on.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use redline_web_server::build_app;
use redline_web_server::config::Config;
use serde_json::{Value, json};
use tower::ServiceExt;

fn config_for(db: &str, read_only: bool) -> Config {
    Config {
        db: db.to_string(),
        read_only,
        ..Config::default()
    }
}

async fn get(uri: &str, config: Config) -> (StatusCode, Value) {
    let app = build_app(config).unwrap();
    let resp = app
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    into_json(resp).await
}

async fn into_json(resp: axum::response::Response) -> (StatusCode, Value) {
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, value)
}

#[tokio::test]
async fn health_is_ok() {
    let (status, body) = get("/api/health", config_for(":memory:", false)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert_eq!(body["engine"], "sqlite");
}

#[tokio::test]
async fn schema_lists_seeded_tables() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.sqlite");
    let db = path.to_str().unwrap();

    let (status, body) = get("/api/schema", config_for(db, false)).await;
    assert_eq!(status, StatusCode::OK);

    let names: Vec<&str> = body["objects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"users"), "schema names: {names:?}");
    assert!(names.contains(&"events"), "schema names: {names:?}");
}

#[tokio::test]
async fn query_select_returns_one() {
    let app = build_app(config_for(":memory:", false)).unwrap();
    let resp = app
        .oneshot(
            Request::post("/api/query")
                .header("content-type", "application/json")
                .body(Body::from(json!({"sql": "select 1 as n"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = into_json(resp).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["columns"], json!(["n"]));
    assert_eq!(body["rows"], json!([[1]]));
    assert_eq!(body["rowCount"], 1);
}

#[tokio::test]
async fn write_rejected_when_read_only() {
    let app = build_app(config_for(":memory:", true)).unwrap();
    let resp = app
        .oneshot(
            Request::post("/api/query")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"sql": "insert into t(a) values (1)"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, body) = into_json(resp).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("read-only"));
}

#[tokio::test]
async fn metrics_counters_increment() {
    let app = build_app(config_for(":memory:", false)).unwrap();

    for _ in 0..2 {
        let resp = app
            .clone()
            .oneshot(
                Request::post("/api/query")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"sql": "select 1"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    let resp = app
        .oneshot(Request::get("/api/metrics").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, body) = into_json(resp).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body["totalQueries"].as_u64().unwrap() >= 2,
        "totalQueries: {}",
        body["totalQueries"]
    );
    assert_eq!(body["failedQueries"], 0);
}
