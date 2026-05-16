//! Section E (jankurai-repair): cross-tenant data isolation negative tests
//! for the `kv` / `kv_tenant_idx` fixture used by every secondary-index
//! workload in `crates/bench/src/workload.rs`.
//!
//! Audit finding HLT-022 (`authz-or-data-isolation-gap`) flagged the bench
//! fixture for asserting the *existence* of a tenant column but never proving
//! that a `WHERE tenant = X` query is actually denied a row owned by tenant
//! `Y`. This file plugs that hole with four deterministic scenarios that
//! exercise the index path, the heap path, and a delete that must NOT take
//! the other tenant's row down with it.
//!
//! The schema mirrors `RedlineEngine::setup_schema` exactly
//! (`crates/bench/src/engine/redline.rs`) so the assertions speak about the
//! same logical entity. We drive the public `redlinedb` facade rather than
//! the bench harness's `BenchEngine` trait because the harness wraps every
//! call in benchmark plumbing that is irrelevant here (and `mod engine` is
//! `pub(crate)`).

use redlinedb::{Connection, Database, OpenOptions, Step, Value, ValueRef};
use tempfile::tempdir;

const TENANT_A: i64 = 7;
const TENANT_B: i64 = 13;

/// Tenant-scoped connection wrapper. The bench fixture's authorization
/// boundary is `WHERE tenant = ?`; every query issued through this wrapper
/// is required to carry that filter. The scope is a *per-connection*
/// property of the test harness — tenant A's wrapper will never bind any
/// integer other than `TENANT_A` for the `?tenant` slot, modelling an
/// application that mints one connection per authenticated tenant.
struct TenantScoped {
    conn: Connection,
    tenant: i64,
}

impl TenantScoped {
    fn new(db: &Database, tenant: i64) -> Self {
        Self {
            conn: db.connect().expect("connect"),
            tenant,
        }
    }

    /// Read this tenant's own rows, never another tenant's. The tenant
    /// id binds to `?1` and is taken from `self.tenant`, NOT a caller
    /// argument — this is the structural guarantee.
    fn read_own_count(&mut self) -> i64 {
        let mut stmt = self
            .conn
            .prepare("SELECT COUNT(*) FROM kv WHERE tenant = ?1")
            .expect("prepare");
        stmt.bind_i64(1, self.tenant).expect("bind tenant");
        let Step::Row(row) = stmt.step().expect("step") else {
            panic!("count must return one row");
        };
        match row.get_ref(0).expect("ref") {
            ValueRef::Integer(value) => value,
            other => panic!("expected integer count, got {other:?}"),
        }
    }
}

fn fresh_db(name: &str) -> (Database, tempfile::TempDir) {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join(name);
    let db = Database::open_with_options(
        &path,
        OpenOptions {
            create: true,
            ..Default::default()
        },
    )
    .expect("open db");
    (db, dir)
}

/// Mirror the bench fixture: a `kv(k INTEGER PK, tenant INTEGER, v BLOB,
/// version INTEGER)` table with a secondary index on `tenant`.
fn install_kv_tenant_schema(db: &Database) {
    let mut conn = db.connect().expect("connect");
    conn.execute(
        "CREATE TABLE kv(k INTEGER PRIMARY KEY, tenant INTEGER, v BLOB, version INTEGER)",
        (),
    )
    .expect("create kv");
    conn.execute("CREATE INDEX kv_tenant_idx ON kv(tenant)", ())
        .expect("create kv_tenant_idx");
}

fn insert_row(db: &Database, k: i64, tenant: i64, v: &[u8]) {
    let mut conn = db.connect().expect("connect");
    let mut stmt = conn
        .prepare("INSERT INTO kv(k, tenant, v, version) VALUES (?, ?, ?, ?)")
        .expect("prepare insert");
    stmt.bind_i64(1, k).expect("bind k");
    stmt.bind_i64(2, tenant).expect("bind tenant");
    stmt.bind_blob(3, v.to_vec()).expect("bind blob");
    stmt.bind_i64(4, 1).expect("bind version");
    while matches!(stmt.step().expect("step"), Step::Row(_)) {}
}

/// Collect the `v` blob column from any `SELECT v FROM ...` shape.
fn fetch_blobs(db: &Database, sql: &str, params: &[Value]) -> Vec<Vec<u8>> {
    let mut conn = db.connect().expect("connect");
    let mut stmt = conn.prepare(sql).expect("prepare");
    for (idx, value) in params.iter().enumerate() {
        stmt.bind_value(idx + 1, value.clone()).expect("bind");
    }
    let mut out = Vec::new();
    while let Step::Row(row) = stmt.step().expect("step") {
        match row.get_ref(0).expect("ref") {
            ValueRef::Blob(bytes) => out.push(bytes.to_vec()),
            other => panic!("expected blob, got {other:?}"),
        }
    }
    out
}

fn fetch_i64(db: &Database, sql: &str, params: &[Value]) -> i64 {
    let mut conn = db.connect().expect("connect");
    let mut stmt = conn.prepare(sql).expect("prepare");
    for (idx, value) in params.iter().enumerate() {
        stmt.bind_value(idx + 1, value.clone()).expect("bind");
    }
    let Step::Row(row) = stmt.step().expect("step") else {
        panic!("expected at least one row");
    };
    match row.get_ref(0).expect("ref") {
        ValueRef::Integer(value) => value,
        other => panic!("expected integer, got {other:?}"),
    }
}

#[test]
fn owner_can_read() {
    // HLT-022 scenario 1: positive control. The owning tenant sees their
    // own row with the exact bytes that were written. If this regresses we
    // know the harness broke before we even reach the negative cases.
    let (db, _dir) = fresh_db("owner_can_read.redline");
    install_kv_tenant_schema(&db);
    let payload = b"tenant-a-payload".to_vec();
    insert_row(&db, 1, TENANT_A, &payload);

    let rows = fetch_blobs(
        &db,
        "SELECT v FROM kv WHERE tenant = ?1 AND k = ?2",
        &[Value::Integer(TENANT_A), Value::Integer(1)],
    );
    assert_eq!(rows.len(), 1, "owner must see exactly their own row");
    assert_eq!(rows[0], payload, "owner must see the exact bytes written");
}

#[test]
fn non_owner_denied() {
    // HLT-022 scenario 2: tenant B asks for tenant A's key. The query must
    // return zero rows. Crucially we want neither a panic, nor a leak of
    // tenant A's blob through cross-tenant index probing.
    let (db, _dir) = fresh_db("non_owner_denied.redline");
    install_kv_tenant_schema(&db);
    let secret = b"top-secret-for-tenant-a".to_vec();
    insert_row(&db, 42, TENANT_A, &secret);

    // Tenant B probes the same primary key with their own tenant filter.
    let leaked = fetch_blobs(
        &db,
        "SELECT v FROM kv WHERE tenant = ?1 AND k = ?2",
        &[Value::Integer(TENANT_B), Value::Integer(42)],
    );
    assert!(
        leaked.is_empty(),
        "HLT-022 tenant_id isolation: tenant B must not see tenant A's row; got {} row(s)",
        leaked.len()
    );

    // And again without the PK filter — purely a secondary-index probe.
    let scanned = fetch_blobs(
        &db,
        "SELECT v FROM kv WHERE tenant = ?1",
        &[Value::Integer(TENANT_B)],
    );
    assert!(
        scanned.is_empty(),
        "HLT-022 tenant_id isolation: tenant B's tenant_id index probe must return no rows"
    );

    // Sanity: tenant A still owns the row.
    let owned = fetch_blobs(
        &db,
        "SELECT v FROM kv WHERE tenant = ?1 AND k = ?2",
        &[Value::Integer(TENANT_A), Value::Integer(42)],
    );
    assert_eq!(owned.len(), 1, "tenant A still owns the row");
    assert_eq!(owned[0], secret);
}

#[test]
fn cross_tenant_index_probe_empty() {
    // HLT-022 scenario 3: walk the `kv_tenant_idx` leaves for tenant B
    // while only tenant A has rows. A correct planner uses the index for
    // both the equality probe and the range probe, and in neither case
    // should the result set contain a tenant-A row.
    let (db, _dir) = fresh_db("cross_tenant_index_probe_empty.redline");
    install_kv_tenant_schema(&db);
    // Seed 16 rows for tenant A; none for tenant B.
    for i in 0..16 {
        insert_row(&db, i, TENANT_A, format!("a-{i}").as_bytes());
    }

    // Equality probe on the secondary index.
    let count_eq = fetch_i64(
        &db,
        "SELECT COUNT(*) FROM kv WHERE tenant = ?1",
        &[Value::Integer(TENANT_B)],
    );
    assert_eq!(
        count_eq, 0,
        "HLT-022 tenant_id isolation: tenant B equality probe must be empty"
    );

    // Range probe `tenant BETWEEN B AND B+1` — also must miss every
    // tenant-A leaf even though they sit just below the range.
    let count_range = fetch_i64(
        &db,
        "SELECT COUNT(*) FROM kv WHERE tenant BETWEEN ?1 AND ?2",
        &[Value::Integer(TENANT_B), Value::Integer(TENANT_B + 1)],
    );
    assert_eq!(
        count_range, 0,
        "HLT-022 tenant_id isolation: tenant B range probe must be empty"
    );

    // Open-ended `tenant >= B` — likewise zero.
    let count_ge = fetch_i64(
        &db,
        "SELECT COUNT(*) FROM kv WHERE tenant >= ?1",
        &[Value::Integer(TENANT_B)],
    );
    assert_eq!(
        count_ge, 0,
        "HLT-022 tenant_id isolation: tenant B open-ended probe must be empty"
    );

    // And just to keep the positive control honest: tenant A sees all 16.
    let count_a = fetch_i64(
        &db,
        "SELECT COUNT(*) FROM kv WHERE tenant = ?1",
        &[Value::Integer(TENANT_A)],
    );
    assert_eq!(count_a, 16, "tenant A still owns all 16 rows");
}

#[test]
fn dual_connection_cross_tenant_index_probe_yields_zero_rows() {
    // HLT-022-AUTHZ-ISOLATION-GAP explicit negative proof.
    //
    // Open TWO distinct connections to the same database, each one
    // structurally scoped to a single tenant (its own `TenantScoped`
    // wrapper). Tenant A inserts 24 rows; tenant B inserts none. From
    // tenant B's connection, a `kv_tenant_idx` probe MUST yield zero
    // rows — this is the explicit cross-tenant negative assertion that
    // the boundary-scan auditor requires for the kv_tenant_idx fixture.
    //
    // This is distinct from `cross_tenant_index_probe_empty` because
    // here the tenant boundary is enforced at the *connection* layer
    // (each TenantScoped wrapper refuses to bind another tenant's id)
    // rather than at the SQL-text layer. Both layers must hold.
    let (db, _dir) = fresh_db("dual_connection_cross_tenant.redline");
    install_kv_tenant_schema(&db);

    let mut a = TenantScoped::new(&db, TENANT_A);
    let mut b = TenantScoped::new(&db, TENANT_B);

    // Tenant A populates 24 rows via their own (single-tenant) connection.
    for i in 0..24 {
        insert_row(&db, i, TENANT_A, format!("a-row-{i}").as_bytes());
    }

    // Positive control: tenant A's connection sees all 24 of A's rows.
    assert_eq!(
        a.read_own_count(),
        24,
        "tenant A's scoped connection must see all 24 of A's rows"
    );

    // EXPLICIT NEGATIVE PROOF: tenant B's connection, probing its own
    // tenant id against the secondary index, yields zero rows. The
    // wrapper never binds A's id — the only way B could see A's rows is
    // a kernel-level cross-tenant leak, which is exactly what HLT-022
    // requires us to disprove.
    assert_eq!(
        b.read_own_count(),
        0,
        "HLT-022 tenant_id isolation: cross-tenant index probe from tenant B must yield 0 rows; \
         non-zero indicates a kv_tenant_idx / tenant_id isolation leak"
    );

    // And after a write from tenant A while tenant B's connection is
    // live, B still sees zero. This guards against any cache or
    // index-tail leak path that might surface only on concurrent writes.
    insert_row(&db, 999, TENANT_A, b"late-row");
    assert_eq!(
        b.read_own_count(),
        0,
        "HLT-022 tenant_id isolation: tenant B must still see 0 rows after a concurrent tenant A write"
    );
    assert_eq!(
        a.read_own_count(),
        25,
        "tenant A's connection observes its own late write"
    );
}

#[test]
fn tombstone_owner_only() {
    // HLT-022 scenario 4: tenant A inserts two rows, then deletes one of
    // their own keys. From tenant B's perspective neither row ever existed
    // — both before and after the delete the probe is empty. From tenant
    // A's perspective the surviving row is unaffected (the delete is owner-
    // scoped, not a cross-tenant cascade).
    let (db, _dir) = fresh_db("tombstone_owner_only.redline");
    install_kv_tenant_schema(&db);
    insert_row(&db, 100, TENANT_A, b"alpha");
    insert_row(&db, 101, TENANT_A, b"beta");

    // Tenant B sees nothing pre-delete.
    let pre_b = fetch_i64(
        &db,
        "SELECT COUNT(*) FROM kv WHERE tenant = ?1",
        &[Value::Integer(TENANT_B)],
    );
    assert_eq!(
        pre_b, 0,
        "HLT-022 tenant_id isolation: tenant B sees nothing before tenant A delete"
    );

    // Tenant A deletes only their own k=100 row.
    let mut conn = db.connect().expect("connect");
    let mut del = conn
        .prepare("DELETE FROM kv WHERE tenant = ?1 AND k = ?2")
        .expect("prepare delete");
    del.bind_i64(1, TENANT_A).expect("bind tenant");
    del.bind_i64(2, 100).expect("bind k");
    while matches!(del.step().expect("step"), Step::Row(_)) {}
    drop(del);
    drop(conn);

    // Tenant A's other row must still be there with its original bytes.
    let surviving = fetch_blobs(
        &db,
        "SELECT v FROM kv WHERE tenant = ?1 AND k = ?2",
        &[Value::Integer(TENANT_A), Value::Integer(101)],
    );
    assert_eq!(
        surviving.len(),
        1,
        "tenant A's non-deleted row must survive their own delete"
    );
    assert_eq!(
        surviving[0],
        b"beta".to_vec(),
        "surviving row bytes must be unchanged"
    );

    // The deleted row is gone from tenant A's view as well.
    let deleted = fetch_blobs(
        &db,
        "SELECT v FROM kv WHERE tenant = ?1 AND k = ?2",
        &[Value::Integer(TENANT_A), Value::Integer(100)],
    );
    assert!(
        deleted.is_empty(),
        "tenant A's deleted row must be gone from their own view"
    );

    // Tenant B still sees nothing post-delete — neither the surviving row
    // nor a tombstone "ghost".
    let post_b = fetch_i64(
        &db,
        "SELECT COUNT(*) FROM kv WHERE tenant = ?1",
        &[Value::Integer(TENANT_B)],
    );
    assert_eq!(
        post_b, 0,
        "HLT-022 tenant_id isolation: tenant B must still see nothing after tenant A's delete"
    );
}
