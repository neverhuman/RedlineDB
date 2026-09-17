//! Integration tests for the SQLite connector, driving it through the public
//! `Connector` trait against an in-memory database.

use std::time::Duration;

use redline_web_server::connector::{Connector, ConnectorError, PageOptions, SqliteConnector};

fn mem() -> SqliteConnector {
    SqliteConnector::open(":memory:", false, Duration::from_secs(5)).unwrap()
}

#[tokio::test]
async fn seeds_demo_schema() {
    let c = mem();
    let schema = c.schema().await.unwrap();
    let names: Vec<_> = schema.objects.iter().map(|o| o.name.as_str()).collect();
    assert!(names.contains(&"users"));
    assert!(names.contains(&"events"));
}

#[tokio::test]
async fn select_returns_rows() {
    let c = mem();
    let r = c.query("SELECT 1 AS n", 100).await.unwrap();
    assert_eq!(r.columns, vec!["n".to_string()]);
    assert_eq!(r.rows[0][0], serde_json::json!(1));
    assert_eq!(r.row_count, 1);
}

#[tokio::test]
async fn truncates_at_max_rows() {
    let c = mem();
    // The seeded `users` table has three rows; cap the page at two to exercise
    // the truncation path without a synthetic row generator.
    let opts = PageOptions {
        limit: 2,
        ..Default::default()
    };
    let page = c.table_page("users", &opts).await.unwrap();
    assert_eq!(page.rows.len(), 2);
    assert_eq!(page.total, Some(3));
}

#[tokio::test]
async fn query_truncates_when_over_cap() {
    let c = mem();
    let r = c.query("PRAGMA table_info(users)", 1).await.unwrap();
    assert_eq!(r.rows.len(), 1);
    assert!(r.truncated);
}

#[tokio::test]
async fn read_only_rejects_writes() {
    let c = SqliteConnector::open(":memory:", true, Duration::from_secs(5)).unwrap();
    let err = c.query("INSERT INTO users(name) VALUES('x')", 100).await;
    assert!(matches!(err, Err(ConnectorError::ReadOnly(_))));
}

#[tokio::test]
async fn rejects_unknown_order_by() {
    let c = mem();
    let opts = PageOptions {
        order_by: Some("nope".into()),
        ..Default::default()
    };
    let err = c.table_page("users", &opts).await;
    assert!(matches!(err, Err(ConnectorError::InvalidArgument(_))));
}
