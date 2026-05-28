use std::sync::{Arc, Mutex, MutexGuard};

use redlinedb_sql::{
    Connection, Database, DbOptions, RqlBinaryOp, RqlColumnRef, RqlExpr, RqlName, RqlOrder,
    RqlSelect, RqlSelectItem, RqlStatement, RqlTableRef, SqlValue, Step,
};

static ENV_LOCK: Mutex<()> = Mutex::new(());

struct EnvGuard {
    old: Vec<(&'static str, Option<std::ffi::OsString>)>,
    _guard: MutexGuard<'static, ()>,
}

impl EnvGuard {
    fn set_many(vars: &[(&'static str, Option<&str>)]) -> Self {
        let guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let old = vars
            .iter()
            .map(|(name, _)| (*name, std::env::var_os(name)))
            .collect::<Vec<_>>();
        // SAFETY: this test file serializes all environment mutations.
        unsafe {
            for (name, value) in vars {
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
        }
        Self { old, _guard: guard }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // SAFETY: this test file serializes all environment mutations.
        unsafe {
            for (name, value) in &self.old {
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
        }
    }
}

fn memory_conn() -> Arc<Connection> {
    Database::create_in_memory(DbOptions::default())
        .expect("db")
        .connect()
}

fn create_items(conn: &Arc<Connection>) {
    conn.execute("CREATE TABLE items(id INTEGER PRIMARY KEY, name TEXT, score INTEGER)")
        .expect("create items");
    conn.execute(
        "INSERT INTO items(id, name, score) VALUES (1, 'Bob', 10), (2, 'Ada', 20), (3, 'Zoe', 30)",
    )
    .expect("seed items");
}

fn table_ref(schema: Option<&str>, name: &str, alias: Option<&str>) -> RqlTableRef {
    RqlTableRef {
        name: RqlName {
            schema: schema.map(str::to_owned),
            name: name.to_owned(),
        },
        alias: alias.map(str::to_owned),
    }
}

fn column(name: &str) -> RqlExpr {
    RqlExpr::Column {
        column: RqlColumnRef {
            table: None,
            name: name.to_owned(),
        },
    }
}

fn qualified_column(table: &str, name: &str) -> RqlExpr {
    RqlExpr::Column {
        column: RqlColumnRef {
            table: Some(table.to_owned()),
            name: name.to_owned(),
        },
    }
}

fn select_from(from: RqlTableRef, projection: Vec<RqlSelectItem>) -> RqlStatement {
    RqlStatement::Select(RqlSelect {
        distinct: false,
        projection,
        from: Some(from),
        joins: Vec::new(),
        filter: None,
        group_by: Vec::new(),
        having: None,
        order_by: Vec::new(),
        limit: None,
        offset: None,
    })
}

fn select_no_from(projection: Vec<RqlSelectItem>) -> RqlStatement {
    RqlStatement::Select(RqlSelect {
        distinct: false,
        projection,
        from: None,
        joins: Vec::new(),
        filter: None,
        group_by: Vec::new(),
        having: None,
        order_by: Vec::new(),
        limit: None,
        offset: None,
    })
}

fn is_native_select(conn: &Arc<Connection>, statement: &RqlStatement) -> bool {
    conn.prepare_rql(statement)
        .expect("prepare rql")
        .template()
        .sql
        .as_ref()
        .ends_with("select_native")
}

fn snapshot(conn: &Arc<Connection>, statement: &RqlStatement) -> (Vec<String>, Vec<Vec<SqlValue>>) {
    let mut stmt = conn.prepare_rql(statement).expect("prepare rql");
    let names = (0..stmt.column_count())
        .map(|idx| stmt.column_name(idx).to_owned())
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    while stmt.step().expect("step") == Step::Row {
        let mut row = Vec::with_capacity(stmt.column_count());
        for idx in 0..stmt.column_count() {
            row.push(stmt.column_value(idx).expect("value").clone());
        }
        rows.push(row);
    }
    (names, rows)
}

#[test]
fn native_select_falls_back_for_sql_binder_only_sources() {
    let _env = EnvGuard::set_many(&[("REDLINE_RQL_NATIVE_SELECT", Some("1"))]);
    let conn = memory_conn();
    create_items(&conn);

    let schema_select = select_from(
        table_ref(None, "sqlite_schema", None),
        vec![RqlSelectItem::Expr {
            expr: column("name"),
            alias: None,
        }],
    );
    assert!(!is_native_select(&conn, &schema_select));
    let mut schema_stmt = conn.prepare_rql(&schema_select).expect("sqlite_schema");
    assert!(matches!(schema_stmt.step().expect("schema row"), Step::Row));

    let temp_schema_select = select_from(
        table_ref(None, "sqlite_temp_schema", None),
        vec![RqlSelectItem::Wildcard],
    );
    assert!(!is_native_select(&conn, &temp_schema_select));

    let pragma_select = select_from(
        table_ref(None, "pragma_database_list", None),
        vec![RqlSelectItem::Wildcard],
    );
    assert!(!is_native_select(&conn, &pragma_select));
    let mut pragma_stmt = conn
        .prepare_rql(&pragma_select)
        .expect("pragma_database_list");
    assert!(matches!(pragma_stmt.step().expect("pragma row"), Step::Row));
}

#[test]
fn native_select_falls_back_for_attached_schema_sources() {
    let _env = EnvGuard::set_many(&[("REDLINE_RQL_NATIVE_SELECT", Some("1"))]);
    let dir = tempfile::tempdir().expect("tempdir");
    let conn = Database::create(dir.path().join("main.db"), DbOptions::default())
        .expect("create main")
        .connect();
    let aux = dir.path().join("aux.db");
    conn.execute(&format!("ATTACH DATABASE '{}' AS aux", aux.display()))
        .expect("attach aux");
    conn.execute("CREATE TABLE aux.events(id INTEGER, kind TEXT)")
        .expect("create aux table");
    conn.execute("INSERT INTO aux.events VALUES (1, 'boot')")
        .expect("insert aux row");

    let select = RqlStatement::Select(RqlSelect {
        distinct: false,
        projection: vec![RqlSelectItem::Expr {
            expr: column("kind"),
            alias: None,
        }],
        from: Some(table_ref(Some("aux"), "events", None)),
        joins: Vec::new(),
        filter: None,
        group_by: Vec::new(),
        having: None,
        order_by: vec![RqlOrder {
            expr: column("id"),
            descending: false,
            nulls_first: None,
        }],
        limit: None,
        offset: None,
    });
    assert!(!is_native_select(&conn, &select));
    let mut stmt = conn.prepare_rql(&select).expect("aux select");
    assert!(matches!(stmt.step().expect("row"), Step::Row));
    assert_eq!(stmt.column_text(0).expect("kind"), "boot");
}

#[test]
fn native_select_wildcard_shapes_match_sql_route() {
    let conn = memory_conn();
    create_items(&conn);
    let cases = [
        select_from(table_ref(None, "items", None), Vec::new()),
        select_from(
            table_ref(None, "items", None),
            vec![RqlSelectItem::Wildcard],
        ),
        select_from(
            table_ref(None, "items", Some("i")),
            vec![RqlSelectItem::QualifiedWildcard {
                table: "i".to_owned(),
            }],
        ),
        select_from(
            table_ref(None, "items", None),
            vec![
                RqlSelectItem::Expr {
                    expr: column("name"),
                    alias: Some("label".to_owned()),
                },
                RqlSelectItem::Wildcard,
            ],
        ),
    ];

    for statement in cases {
        let _sql_route = EnvGuard::set_many(&[("REDLINE_RQL_NATIVE_SELECT", None)]);
        let expected = snapshot(&conn, &statement);
        drop(_sql_route);

        let _native_route = EnvGuard::set_many(&[("REDLINE_RQL_NATIVE_SELECT", Some("1"))]);
        assert!(is_native_select(&conn, &statement));
        assert_eq!(snapshot(&conn, &statement), expected);
    }
}

#[test]
fn native_select_falls_back_for_order_by_ordinals() {
    let _env = EnvGuard::set_many(&[("REDLINE_RQL_NATIVE_SELECT", Some("1"))]);
    let conn = memory_conn();
    create_items(&conn);
    let mut select = match select_from(
        table_ref(None, "items", None),
        vec![RqlSelectItem::Expr {
            expr: column("name"),
            alias: None,
        }],
    ) {
        RqlStatement::Select(select) => select,
        _ => unreachable!(),
    };
    select.order_by.push(RqlOrder {
        expr: RqlExpr::Integer { value: 1 },
        descending: false,
        nulls_first: None,
    });
    let statement = RqlStatement::Select(select.clone());
    assert!(!is_native_select(&conn, &statement));
    let mut stmt = conn.prepare_rql(&statement).expect("order by ordinal");
    assert!(matches!(stmt.step().expect("row"), Step::Row));
    assert_eq!(stmt.column_text(0).expect("name"), "Ada");

    select.order_by[0].expr = RqlExpr::Integer { value: 2 };
    let err = conn.prepare_rql(&RqlStatement::Select(select)).unwrap_err();
    assert!(
        err.to_string().contains("ORDER BY term out of range"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn native_select_cache_mode_does_not_split_mutation_templates() {
    let conn = memory_conn();
    create_items(&conn);
    let insert = RqlStatement::Insert(redlinedb_sql::RqlInsert {
        table: RqlName {
            schema: None,
            name: "items".to_owned(),
        },
        columns: vec!["id".to_owned(), "name".to_owned(), "score".to_owned()],
        values: vec![vec![
            RqlExpr::Param { index: 1 },
            RqlExpr::Param { index: 2 },
            RqlExpr::Param { index: 3 },
        ]],
        default_values: false,
    });

    let _sql_route = EnvGuard::set_many(&[
        ("REDLINE_RQL_TEMPLATE_CACHE", Some("1")),
        ("REDLINE_RQL_NATIVE_SELECT", None),
    ]);
    let sql_template = conn.prepare_rql(&insert).expect("cache insert").template();
    drop(_sql_route);

    let _native_route = EnvGuard::set_many(&[
        ("REDLINE_RQL_TEMPLATE_CACHE", Some("1")),
        ("REDLINE_RQL_NATIVE_SELECT", Some("1")),
    ]);
    let native_template = conn
        .prepare_rql(&insert)
        .expect("cached insert with native gate")
        .template();
    assert!(Arc::ptr_eq(&sql_template, &native_template));
}

#[test]
fn native_select_rejects_hidden_table_qualifier_when_alias_is_present() {
    let _env = EnvGuard::set_many(&[("REDLINE_RQL_NATIVE_SELECT", Some("1"))]);
    let conn = memory_conn();
    create_items(&conn);
    let select = select_from(
        table_ref(None, "items", Some("i")),
        vec![RqlSelectItem::Expr {
            expr: qualified_column("items", "name"),
            alias: None,
        }],
    );
    let mut stmt = conn.prepare_rql(&select).expect("fallback prepare");
    assert!(!stmt.template().sql.as_ref().ends_with("select_native"));
    let err = stmt
        .step()
        .expect_err("hidden qualifier should fail on eval");
    assert!(
        err.to_string().contains("items.name"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn native_select_supports_scalar_functions_but_falls_back_for_aggregates() {
    let _env = EnvGuard::set_many(&[("REDLINE_RQL_NATIVE_SELECT", Some("1"))]);
    let conn = memory_conn();
    create_items(&conn);

    let scalar = RqlStatement::Select(RqlSelect {
        distinct: false,
        projection: vec![
            RqlSelectItem::Expr {
                expr: RqlExpr::Function {
                    name: "upper".to_owned(),
                    args: vec![column("name")],
                    distinct: false,
                },
                alias: Some("up".to_owned()),
            },
            RqlSelectItem::Expr {
                expr: RqlExpr::Function {
                    name: "min".to_owned(),
                    args: vec![column("score"), RqlExpr::Integer { value: 25 }],
                    distinct: false,
                },
                alias: Some("capped".to_owned()),
            },
        ],
        from: Some(table_ref(None, "items", None)),
        joins: Vec::new(),
        filter: Some(RqlExpr::Binary {
            left: Box::new(RqlExpr::Function {
                name: "lower".to_owned(),
                args: vec![column("name")],
                distinct: false,
            }),
            op: RqlBinaryOp::Eq,
            right: Box::new(RqlExpr::Text {
                value: "ada".to_owned(),
            }),
        }),
        group_by: Vec::new(),
        having: None,
        order_by: Vec::new(),
        limit: None,
        offset: None,
    });
    let mut stmt = conn.prepare_rql(&scalar).expect("scalar function select");
    assert!(stmt.template().sql.as_ref().ends_with("select_native"));
    assert!(matches!(stmt.step().expect("row"), Step::Row));
    assert_eq!(stmt.column_text(0).expect("upper name"), "ADA");
    assert_eq!(stmt.column_i64(1).expect("capped score"), 20);

    let aggregate = RqlStatement::Select(RqlSelect {
        distinct: false,
        projection: vec![RqlSelectItem::Expr {
            expr: RqlExpr::Function {
                name: "sum".to_owned(),
                args: vec![column("score")],
                distinct: false,
            },
            alias: None,
        }],
        from: Some(table_ref(None, "items", None)),
        joins: Vec::new(),
        filter: None,
        group_by: Vec::new(),
        having: None,
        order_by: Vec::new(),
        limit: None,
        offset: None,
    });
    let mut stmt = conn.prepare_rql(&aggregate).expect("aggregate fallback");
    assert!(!stmt.template().sql.as_ref().ends_with("select_native"));
    assert!(matches!(stmt.step().expect("aggregate row"), Step::Row));
    assert_eq!(stmt.column_i64(0).expect("sum"), 60);
}

#[test]
fn native_select_no_from_arith_matches_sql_route() {
    let conn = memory_conn();
    let statement = select_no_from(vec![
        RqlSelectItem::Expr {
            expr: RqlExpr::Binary {
                left: Box::new(RqlExpr::Integer { value: 1 }),
                op: RqlBinaryOp::Add,
                right: Box::new(RqlExpr::Integer { value: 2 }),
            },
            alias: Some("sum".to_owned()),
        },
        RqlSelectItem::Expr {
            expr: RqlExpr::Binary {
                left: Box::new(RqlExpr::Text {
                    value: "a".to_owned(),
                }),
                op: RqlBinaryOp::Concat,
                right: Box::new(RqlExpr::Text {
                    value: "b".to_owned(),
                }),
            },
            alias: Some("joined".to_owned()),
        },
    ]);

    let _sql_route = EnvGuard::set_many(&[("REDLINE_RQL_NATIVE_SELECT", None)]);
    let expected = snapshot(&conn, &statement);
    drop(_sql_route);

    let _native_route = EnvGuard::set_many(&[("REDLINE_RQL_NATIVE_SELECT", Some("1"))]);
    assert!(is_native_select(&conn, &statement));
    assert_eq!(snapshot(&conn, &statement), expected);
}

#[test]
fn native_select_no_from_cast_typeof_matches_sql_route() {
    let conn = memory_conn();
    let statement = select_no_from(vec![
        RqlSelectItem::Expr {
            expr: RqlExpr::Function {
                name: "typeof".to_owned(),
                args: vec![RqlExpr::Integer { value: 7 }],
                distinct: false,
            },
            alias: Some("kind".to_owned()),
        },
        RqlSelectItem::Expr {
            expr: RqlExpr::Cast {
                expr: Box::new(RqlExpr::Text {
                    value: "42".to_owned(),
                }),
                data_type: "INTEGER".to_owned(),
            },
            alias: Some("casted".to_owned()),
        },
    ]);

    let _sql_route = EnvGuard::set_many(&[("REDLINE_RQL_NATIVE_SELECT", None)]);
    let expected = snapshot(&conn, &statement);
    drop(_sql_route);

    let _native_route = EnvGuard::set_many(&[("REDLINE_RQL_NATIVE_SELECT", Some("1"))]);
    assert!(is_native_select(&conn, &statement));
    assert_eq!(snapshot(&conn, &statement), expected);
}

#[test]
fn native_select_no_from_null_functions_matches_sql_route() {
    let conn = memory_conn();
    let statement = select_no_from(vec![
        RqlSelectItem::Expr {
            expr: RqlExpr::Function {
                name: "coalesce".to_owned(),
                args: vec![
                    RqlExpr::Null,
                    RqlExpr::Text {
                        value: "fallback".to_owned(),
                    },
                ],
                distinct: false,
            },
            alias: Some("coalesced".to_owned()),
        },
        RqlSelectItem::Expr {
            expr: RqlExpr::Function {
                name: "ifnull".to_owned(),
                args: vec![
                    RqlExpr::Null,
                    RqlExpr::Text {
                        value: "ifnull".to_owned(),
                    },
                ],
                distinct: false,
            },
            alias: Some("ifnull_value".to_owned()),
        },
        RqlSelectItem::Expr {
            expr: RqlExpr::Function {
                name: "nullif".to_owned(),
                args: vec![
                    RqlExpr::Text {
                        value: "same".to_owned(),
                    },
                    RqlExpr::Text {
                        value: "same".to_owned(),
                    },
                ],
                distinct: false,
            },
            alias: Some("nullif_value".to_owned()),
        },
    ]);

    let _sql_route = EnvGuard::set_many(&[("REDLINE_RQL_NATIVE_SELECT", None)]);
    let expected = snapshot(&conn, &statement);
    drop(_sql_route);

    let _native_route = EnvGuard::set_many(&[("REDLINE_RQL_NATIVE_SELECT", Some("1"))]);
    assert!(is_native_select(&conn, &statement));
    assert_eq!(snapshot(&conn, &statement), expected);
}

#[test]
fn native_select_no_from_filter_order_limit_matches_sql_route() {
    let conn = memory_conn();
    let mut select = match select_no_from(vec![RqlSelectItem::Expr {
        expr: RqlExpr::Integer { value: 7 },
        alias: Some("value".to_owned()),
    }]) {
        RqlStatement::Select(select) => select,
        _ => unreachable!(),
    };
    select.filter = Some(RqlExpr::Binary {
        left: Box::new(RqlExpr::Integer { value: 1 }),
        op: RqlBinaryOp::Eq,
        right: Box::new(RqlExpr::Integer { value: 1 }),
    });
    select.order_by.push(RqlOrder {
        expr: RqlExpr::Text {
            value: "constant".to_owned(),
        },
        descending: true,
        nulls_first: Some(false),
    });
    select.limit = Some(1);
    let statement = RqlStatement::Select(select);

    let _sql_route = EnvGuard::set_many(&[("REDLINE_RQL_NATIVE_SELECT", None)]);
    let expected = snapshot(&conn, &statement);
    drop(_sql_route);

    let _native_route = EnvGuard::set_many(&[("REDLINE_RQL_NATIVE_SELECT", Some("1"))]);
    assert!(is_native_select(&conn, &statement));
    assert_eq!(snapshot(&conn, &statement), expected);
}

#[test]
fn native_select_table_json_functions_match_sql_route() {
    let conn = memory_conn();
    conn.execute("CREATE TABLE docs(id INTEGER PRIMARY KEY, body TEXT)")
        .expect("create docs");
    conn.execute(
        "INSERT INTO docs(id, body) VALUES (1, '{\"a\":7,\"b\":\"x\"}'), (2, '{\"a\":9}')",
    )
    .expect("seed docs");
    let mut select = match select_from(
        table_ref(None, "docs", None),
        vec![
            RqlSelectItem::Expr {
                expr: RqlExpr::Function {
                    name: "json_extract".to_owned(),
                    args: vec![
                        column("body"),
                        RqlExpr::Text {
                            value: "$.a".to_owned(),
                        },
                    ],
                    distinct: false,
                },
                alias: Some("a".to_owned()),
            },
            RqlSelectItem::Expr {
                expr: RqlExpr::Function {
                    name: "json_type".to_owned(),
                    args: vec![
                        column("body"),
                        RqlExpr::Text {
                            value: "$.b".to_owned(),
                        },
                    ],
                    distinct: false,
                },
                alias: Some("b_type".to_owned()),
            },
            RqlSelectItem::Expr {
                expr: RqlExpr::Function {
                    name: "json_valid".to_owned(),
                    args: vec![column("body")],
                    distinct: false,
                },
                alias: Some("valid".to_owned()),
            },
        ],
    ) {
        RqlStatement::Select(select) => select,
        _ => unreachable!(),
    };
    select.order_by.push(RqlOrder {
        expr: column("id"),
        descending: false,
        nulls_first: None,
    });
    select.limit = Some(1);
    let statement = RqlStatement::Select(select);

    let _sql_route = EnvGuard::set_many(&[("REDLINE_RQL_NATIVE_SELECT", None)]);
    let expected = snapshot(&conn, &statement);
    drop(_sql_route);

    let _native_route = EnvGuard::set_many(&[("REDLINE_RQL_NATIVE_SELECT", Some("1"))]);
    assert!(is_native_select(&conn, &statement));
    assert_eq!(snapshot(&conn, &statement), expected);
}
