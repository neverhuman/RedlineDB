//! Lane SQL-D phase 10: SQLite-compatible date/time functions.
use std::sync::Arc;

use redlinedb_sql::{Connection, Database, DbOptions, Step};
use tempfile::tempdir;

fn open() -> (tempfile::TempDir, Arc<Connection>) {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("dt.db");
    let db = Database::create(&path, DbOptions::default()).expect("create db");
    (dir, db.connect())
}

fn first_text(conn: &Arc<Connection>, sql: &str) -> String {
    let mut stmt = conn.prepare(sql).expect("prepare");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    stmt.column_text(0).expect("text").to_owned()
}

fn first_i64(conn: &Arc<Connection>, sql: &str) -> i64 {
    let mut stmt = conn.prepare(sql).expect("prepare");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    stmt.column_i64(0).expect("i64")
}

fn first_f64(conn: &Arc<Connection>, sql: &str) -> f64 {
    let mut stmt = conn.prepare(sql).expect("prepare");
    assert_eq!(stmt.step().expect("step"), Step::Row);
    stmt.column_f64(0).expect("f64")
}

#[test]
fn date_literal_round_trip() {
    let (_dir, conn) = open();
    assert_eq!(first_text(&conn, "SELECT date('2024-01-15')"), "2024-01-15");
}

#[test]
fn time_literal_round_trip() {
    let (_dir, conn) = open();
    assert_eq!(first_text(&conn, "SELECT time('12:34:56')"), "12:34:56");
}

#[test]
fn datetime_literal_round_trip() {
    let (_dir, conn) = open();
    assert_eq!(
        first_text(&conn, "SELECT datetime('2024-01-15 12:34:56')"),
        "2024-01-15 12:34:56"
    );
}

#[test]
fn julianday_known_value() {
    let (_dir, conn) = open();
    let jd = first_f64(&conn, "SELECT julianday('2000-01-01 12:00:00')");
    // 2000-01-01 noon UTC is exactly Julian Day 2451545.0.
    assert!(
        (jd - 2_451_545.0).abs() < 1e-6,
        "expected ~2451545, got {jd}"
    );
}

#[test]
fn strftime_year_only() {
    let (_dir, conn) = open();
    assert_eq!(
        first_text(&conn, "SELECT strftime('%Y', '2024-01-15')"),
        "2024"
    );
}

#[test]
fn strftime_full_format() {
    let (_dir, conn) = open();
    assert_eq!(
        first_text(
            &conn,
            "SELECT strftime('%Y-%m-%d %H:%M:%S', '2024-01-15 09:08:07')"
        ),
        "2024-01-15 09:08:07"
    );
}

#[test]
fn date_modifier_plus_one_day() {
    let (_dir, conn) = open();
    assert_eq!(
        first_text(&conn, "SELECT date('2024-01-15', '+1 day')"),
        "2024-01-16"
    );
}

#[test]
fn date_modifier_start_of_month() {
    let (_dir, conn) = open();
    assert_eq!(
        first_text(&conn, "SELECT date('2024-01-15', 'start of month')"),
        "2024-01-01"
    );
}

#[test]
fn unixepoch_known_date() {
    let (_dir, conn) = open();
    let ts = first_i64(&conn, "SELECT unixepoch('1970-01-02 00:00:00')");
    assert_eq!(ts, 86_400);
}
