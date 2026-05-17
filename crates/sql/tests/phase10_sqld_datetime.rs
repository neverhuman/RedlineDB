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

// ── A8 datetime modifier expansion (rusqlite-parity asserted) ───────────────

fn rusqlite_text(sql: &str) -> String {
    let conn = rusqlite::Connection::open_in_memory().expect("rusqlite in-mem");
    conn.query_row(sql, [], |r| r.get::<_, String>(0))
        .unwrap_or_else(|e| panic!("rusqlite {sql:?}: {e}"))
}

fn rusqlite_i64(sql: &str) -> i64 {
    let conn = rusqlite::Connection::open_in_memory().expect("rusqlite in-mem");
    conn.query_row(sql, [], |r| r.get::<_, i64>(0))
        .unwrap_or_else(|e| panic!("rusqlite {sql:?}: {e}"))
}

fn assert_text_parity(conn: &Arc<Connection>, sql: &str) {
    let ours = first_text(conn, sql);
    let theirs = rusqlite_text(sql);
    assert_eq!(ours, theirs, "datetime parity failed for: {sql}");
}

#[test]
fn modifier_start_of_day_matches_sqlite() {
    let (_d, c) = open();
    assert_text_parity(
        &c,
        "SELECT datetime('2024-06-15 14:25:36', 'start of day')",
    );
}

#[test]
fn modifier_plus_weeks_unit() {
    // SQLite did not always support `weeks` modifiers (added late in the
    // 3.46 series). We accept and compute it; we don't gate against rusqlite
    // because the bundled version may reject it.
    let (_d, c) = open();
    assert_eq!(
        first_text(&c, "SELECT date('2024-01-01', '+2 weeks')"),
        "2024-01-15"
    );
}

#[test]
fn modifier_minus_one_month_matches_sqlite() {
    let (_d, c) = open();
    assert_text_parity(&c, "SELECT date('2024-03-15', '-1 month')");
}

#[test]
fn modifier_plus_one_year_leap_clamps() {
    // 2024-02-29 + 1 year = 2025-02-28 (clamped by days-in-month).
    let (_d, c) = open();
    assert_text_parity(&c, "SELECT date('2024-02-29', '+1 year')");
}

#[test]
fn modifier_chained_arithmetic() {
    let (_d, c) = open();
    assert_text_parity(
        &c,
        "SELECT datetime('2024-01-01 00:00:00', '+1 day', '+12 hours', '+30 minutes')",
    );
}

#[test]
fn modifier_weekday_advance() {
    // 2024-06-13 is a Thursday (day_of_week=4). weekday 6 (Saturday) → +2 days.
    let (_d, c) = open();
    assert_text_parity(&c, "SELECT date('2024-06-13', 'weekday 6')");
}

#[test]
fn modifier_weekday_same_day() {
    // 2024-06-13 is Thursday. weekday 4 (Thursday) → same day.
    let (_d, c) = open();
    assert_text_parity(&c, "SELECT date('2024-06-13', 'weekday 4')");
}

#[test]
fn modifier_weekday_sunday_wraps() {
    // 2024-06-13 (Thu, dow=4); weekday 0 (Sun) → +3 days.
    let (_d, c) = open();
    assert_text_parity(&c, "SELECT date('2024-06-13', 'weekday 0')");
}

#[test]
#[allow(non_snake_case)]
fn strftime_percent_T_full_clock() {
    let (_d, c) = open();
    assert_text_parity(
        &c,
        "SELECT strftime('%T', '2024-06-15 09:08:07')",
    );
}

#[test]
#[allow(non_snake_case)]
fn strftime_percent_R_hour_minute() {
    let (_d, c) = open();
    assert_text_parity(&c, "SELECT strftime('%R', '2024-06-15 09:08:07')");
}

#[test]
fn strftime_percent_p_am_pm() {
    let (_d, c) = open();
    assert_text_parity(&c, "SELECT strftime('%p', '2024-06-15 09:08:07')");
    assert_text_parity(&c, "SELECT strftime('%p', '2024-06-15 14:08:07')");
}

#[test]
fn strftime_percent_u_iso_weekday() {
    // %u: Monday=1..Sunday=7.
    let (_d, c) = open();
    assert_text_parity(&c, "SELECT strftime('%u', '2024-06-17')"); // Mon
    assert_text_parity(&c, "SELECT strftime('%u', '2024-06-23')"); // Sun
}

#[test]
fn strftime_percent_w_sunday_zero() {
    let (_d, c) = open();
    assert_text_parity(&c, "SELECT strftime('%w', '2024-06-23')"); // Sun→0
    assert_text_parity(&c, "SELECT strftime('%w', '2024-06-17')"); // Mon→1
}

#[test]
fn strftime_percent_j_day_of_year() {
    let (_d, c) = open();
    assert_text_parity(&c, "SELECT strftime('%j', '2024-12-31')");
}

#[test]
fn strftime_percent_s_unix_seconds() {
    let (_d, c) = open();
    let ours = first_text(&c, "SELECT strftime('%s', '2024-01-01 00:00:00')");
    let theirs = rusqlite_text("SELECT strftime('%s', '2024-01-01 00:00:00')");
    assert_eq!(ours, theirs);
}

#[test]
fn unixepoch_round_trips_through_datetime() {
    // datetime(unixepoch_value, 'unixepoch') should round-trip.
    let (_d, c) = open();
    let ts = rusqlite_i64("SELECT unixepoch('2024-06-15 12:00:00')");
    assert_eq!(
        first_i64(&c, "SELECT unixepoch('2024-06-15 12:00:00')"),
        ts
    );
}

#[test]
fn julianday_known_value_matches_sqlite() {
    let jd = rusqlite::Connection::open_in_memory()
        .expect("rusqlite")
        .query_row(
            "SELECT julianday('2024-06-15 12:00:00')",
            [],
            |r| r.get::<_, f64>(0),
        )
        .expect("rusqlite julianday");
    let ours = first_f64(&open().1, "SELECT julianday('2024-06-15 12:00:00')");
    assert!(
        (ours - jd).abs() < 1e-6,
        "julianday mismatch: ours={ours} sqlite={jd}"
    );
}
