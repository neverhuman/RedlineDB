mod beyond_oracle;

use beyond_oracle::{PostgresHarness, RedlineHarness, SqliteHarness, both_engines_reject};

#[test]
fn ilike_ascii_reference_matches_postgres() {
    let redline = RedlineHarness::in_memory();
    let Some(mut postgres) = PostgresHarness::try_connect_from_env() else {
        eprintln!("skipping Postgres reference test; REDLINEDB_POSTGRES_URL is not set");
        return;
    };
    let setup = "
        CREATE TABLE things(name TEXT);
        INSERT INTO things(name) VALUES ('Alpha'), ('beta'), ('ALMANAC'), ('zeta');
    ";
    redline.execute(setup);
    postgres.execute(setup);

    let redline_rows =
        redline.query_text_rows("SELECT name FROM things WHERE name ILIKE '%al%' ORDER BY name");
    let postgres_rows = postgres
        .query_text_rows("SELECT name::text FROM things WHERE name ILIKE '%al%' ORDER BY name");

    assert_eq!(redline_rows, postgres_rows);
    assert_eq!(
        redline_rows,
        vec![vec!["ALMANAC".to_owned()], vec!["Alpha".to_owned()]]
    );
}

#[test]
fn ilike_unicode_reference_matches_postgres() {
    let redline = RedlineHarness::in_memory();
    let Some(mut postgres) = PostgresHarness::try_connect_from_env() else {
        eprintln!("skipping Postgres reference test; REDLINEDB_POSTGRES_URL is not set");
        return;
    };
    let setup = "
        CREATE TABLE things(name TEXT);
        INSERT INTO things(name) VALUES ('Äpfel'), ('äther'), ('Banane'), ('ångström');
    ";
    redline.execute(setup);
    postgres.execute(setup);

    let redline_rows =
        redline.query_text_rows("SELECT name FROM things WHERE name ILIKE 'ä%' ORDER BY name");
    let postgres_rows = postgres
        .query_text_rows("SELECT name::text FROM things WHERE name ILIKE 'ä%' ORDER BY name");

    assert_eq!(redline_rows, postgres_rows);
}

#[test]
fn sqlite_rejects_ilike_as_control_behavior() {
    let sqlite = SqliteHarness::in_memory();
    let err = sqlite.prepare_error("SELECT 'Alpha' ILIKE '%al%'");
    assert!(
        err.to_ascii_uppercase().contains("ILIKE"),
        "unexpected sqlite error: {err}"
    );
}

#[test]
fn default_keyword_matches_postgres_across_dml_forms() {
    let redline = RedlineHarness::in_memory();
    let mut postgres = PostgresHarness::connect_from_env();
    let setup = "
        CREATE TABLE things(
            id INTEGER PRIMARY KEY,
            value TEXT DEFAULT 'fallback',
            note TEXT DEFAULT 'reset'
        );
    ";
    redline.execute(setup);
    postgres.execute(setup);

    let statements = [
        (
            "INSERT INTO things(id, value, note) VALUES (1, DEFAULT, DEFAULT) RETURNING id, value, note",
            "INSERT INTO things(id, value, note) VALUES (1, DEFAULT, DEFAULT) RETURNING id::text, value, note",
        ),
        (
            "UPDATE things SET value = DEFAULT, note = DEFAULT WHERE id = 1 RETURNING id, value, note",
            "UPDATE things SET value = DEFAULT, note = DEFAULT WHERE id = 1 RETURNING id::text, value, note",
        ),
        (
            "INSERT INTO things(id, value, note) VALUES (1, 'ignored', 'ignored') ON CONFLICT(id) DO UPDATE SET value = DEFAULT, note = DEFAULT RETURNING id, value, note",
            "INSERT INTO things(id, value, note) VALUES (1, 'ignored', 'ignored') ON CONFLICT(id) DO UPDATE SET value = DEFAULT, note = DEFAULT RETURNING id::text, value, note",
        ),
    ];
    for (redline_sql, postgres_sql) in statements {
        let redline_returned = redline.query_text_rows(redline_sql);
        let postgres_returned = postgres.query_text_rows(postgres_sql);
        assert_eq!(
            redline_returned, postgres_returned,
            "statement: {redline_sql}"
        );
        assert_eq!(
            redline_returned,
            vec![vec![
                "1".to_owned(),
                "fallback".to_owned(),
                "reset".to_owned()
            ]]
        );
    }

    let redline_rows = redline.query_text_rows("SELECT id, value, note FROM things ORDER BY id");
    let postgres_rows =
        postgres.query_text_rows("SELECT id::text, value, note FROM things ORDER BY id");
    assert_eq!(redline_rows, postgres_rows);
    assert_eq!(
        redline_rows,
        vec![vec![
            "1".to_owned(),
            "fallback".to_owned(),
            "reset".to_owned()
        ]]
    );
}

#[test]
fn boolean_and_uuid_strict_storage_matches_postgres() {
    let redline = RedlineHarness::in_memory();
    let mut postgres = PostgresHarness::connect_from_env();
    redline.execute("CREATE TABLE t(flag BOOLEAN, ident UUID) STRICT");
    postgres.execute("CREATE TABLE t(flag BOOLEAN, ident UUID)");

    let sql = "INSERT INTO t(flag, ident) VALUES (TRUE, '550E8400-E29B-41D4-A716-446655440000')";
    redline.execute(sql);
    postgres.execute(sql);

    let redline_rows =
        redline.query_text_rows("SELECT CAST(flag AS INTEGER), CAST(ident AS TEXT) FROM t");
    let postgres_rows =
        postgres.query_text_rows("SELECT CASE WHEN flag THEN '1' ELSE '0' END, ident::text FROM t");
    assert_eq!(redline_rows, postgres_rows);
    assert_eq!(
        redline_rows,
        vec![vec![
            "1".to_owned(),
            "550e8400-e29b-41d4-a716-446655440000".to_owned(),
        ]]
    );
}

#[test]
fn invalid_boolean_and_uuid_strict_writes_match_postgres_rejection() {
    let redline = RedlineHarness::in_memory();
    let mut postgres = PostgresHarness::connect_from_env();
    redline.execute("CREATE TABLE t(flag BOOLEAN, ident UUID) STRICT");
    postgres.execute("CREATE TABLE t(flag BOOLEAN, ident UUID)");

    for sql in [
        "INSERT INTO t(flag, ident) VALUES (2, '550E8400-E29B-41D4-A716-446655440001')",
        "INSERT INTO t(flag, ident) VALUES (1.5, '550E8400-E29B-41D4-A716-446655440002')",
        "INSERT INTO t(flag, ident) VALUES (1, 123)",
        "INSERT INTO t(flag, ident) VALUES ('true', 'not-a-uuid')",
    ] {
        both_engines_reject(&redline, &mut postgres, sql);
    }
}

#[test]
fn alter_table_add_column_if_not_exists_matches_postgres() {
    let redline = RedlineHarness::in_memory();
    let mut postgres = PostgresHarness::connect_from_env();
    let setup = "
        CREATE TABLE t(id INTEGER PRIMARY KEY);
        INSERT INTO t VALUES (1);
    ";
    redline.execute(setup);
    postgres.execute(setup);

    let sql = "ALTER TABLE t ADD COLUMN IF NOT EXISTS value TEXT NOT NULL DEFAULT 'x'";
    redline.execute(sql);
    postgres.execute(sql);
    redline.execute(sql);
    postgres.execute(sql);

    let redline_rows = redline.query_text_rows("SELECT id, value FROM t ORDER BY id");
    let postgres_rows = postgres.query_text_rows("SELECT id::text, value FROM t ORDER BY id");
    assert_eq!(redline_rows, postgres_rows);
    assert_eq!(redline_rows, vec![vec!["1".to_owned(), "x".to_owned()]]);
}
