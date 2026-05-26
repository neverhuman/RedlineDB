use redlinedb::{
    Database, RqlBinaryOp, RqlColumnDef, RqlColumnRef, RqlCreateTable, RqlDelete, RqlExpr,
    RqlInsert, RqlJoin, RqlJoinKind, RqlName, RqlOrder, RqlProgram, RqlSelect, RqlSelectItem,
    RqlStatement, RqlTableRef, RqlUpdate, RqlUpdateAssignment, Step,
};

fn name(value: &str) -> RqlName {
    RqlName {
        schema: None,
        name: value.to_owned(),
    }
}

fn table(value: &str) -> RqlTableRef {
    RqlTableRef {
        name: name(value),
        alias: None,
    }
}

fn table_alias(value: &str, alias: &str) -> RqlTableRef {
    RqlTableRef {
        name: name(value),
        alias: Some(alias.to_owned()),
    }
}

fn column(value: &str) -> RqlExpr {
    RqlExpr::Column {
        column: RqlColumnRef {
            table: None,
            name: value.to_owned(),
        },
    }
}

fn qualified_column(table: &str, value: &str) -> RqlExpr {
    RqlExpr::Column {
        column: RqlColumnRef {
            table: Some(table.to_owned()),
            name: value.to_owned(),
        },
    }
}

fn int(value: i64) -> RqlExpr {
    RqlExpr::Integer { value }
}

fn text(value: &str) -> RqlExpr {
    RqlExpr::Text {
        value: value.to_owned(),
    }
}

fn eq(left: RqlExpr, right: RqlExpr) -> RqlExpr {
    RqlExpr::Binary {
        left: Box::new(left),
        op: RqlBinaryOp::Eq,
        right: Box::new(right),
    }
}

fn expr_item(expr: RqlExpr) -> RqlSelectItem {
    RqlSelectItem::Expr { expr, alias: None }
}

fn col_def(name: &str, declared_type: &str, primary_key: bool) -> RqlColumnDef {
    RqlColumnDef {
        name: name.to_owned(),
        declared_type: Some(declared_type.to_owned()),
        primary_key,
        not_null: false,
        unique: false,
        default: None,
    }
}

#[test]
fn rql_program_covers_core_relational_flow() {
    let dir = tempfile::tempdir().expect("dir");
    let db = Database::create(dir.path().join("rql.redline")).expect("db");
    let mut conn = db.connect().expect("conn");

    conn.execute_rql(&RqlProgram {
        statements: vec![
            RqlStatement::CreateTable(RqlCreateTable {
                table: name("departments"),
                if_not_exists: false,
                columns: vec![
                    col_def("id", "INTEGER", true),
                    col_def("dept_name", "TEXT", false),
                ],
                strict: false,
                without_rowid: false,
            }),
            RqlStatement::CreateTable(RqlCreateTable {
                table: name("people"),
                if_not_exists: false,
                columns: vec![
                    col_def("id", "INTEGER", true),
                    col_def("person_name", "TEXT", false),
                    col_def("dept_id", "INTEGER", false),
                ],
                strict: false,
                without_rowid: false,
            }),
            RqlStatement::Insert(RqlInsert {
                table: name("departments"),
                columns: vec!["id".to_owned(), "dept_name".to_owned()],
                values: vec![vec![int(1), text("math")], vec![int(2), text("systems")]],
                default_values: false,
            }),
            RqlStatement::Insert(RqlInsert {
                table: name("people"),
                columns: vec![
                    "id".to_owned(),
                    "person_name".to_owned(),
                    "dept_id".to_owned(),
                ],
                values: vec![
                    vec![int(1), text("Ada"), int(1)],
                    vec![int(2), text("Lin"), int(2)],
                ],
                default_values: false,
            }),
            RqlStatement::Update(RqlUpdate {
                table: name("people"),
                assignments: vec![RqlUpdateAssignment {
                    column: "person_name".to_owned(),
                    value: text("Linus"),
                }],
                filter: Some(eq(column("id"), int(2))),
            }),
        ],
    })
    .expect("setup");

    let join_select = RqlStatement::Select(RqlSelect {
        distinct: false,
        projection: vec![
            expr_item(qualified_column("p", "person_name")),
            expr_item(qualified_column("d", "dept_name")),
        ],
        from: Some(table_alias("people", "p")),
        joins: vec![RqlJoin {
            table: table_alias("departments", "d"),
            kind: RqlJoinKind::Inner,
            on: Some(eq(
                qualified_column("p", "dept_id"),
                qualified_column("d", "id"),
            )),
        }],
        filter: None,
        group_by: Vec::new(),
        having: None,
        order_by: vec![RqlOrder {
            expr: qualified_column("p", "id"),
            descending: false,
            nulls_first: None,
        }],
        limit: None,
        offset: None,
    });
    let mut stmt = conn.prepare_rql(&join_select).expect("join");
    let mut rows = Vec::new();
    while let Step::Row(row) = stmt.step().expect("step") {
        rows.push((
            row.get::<String>(0).expect("person"),
            row.get::<String>(1).expect("dept"),
        ));
    }
    assert_eq!(
        rows,
        vec![
            ("Ada".to_owned(), "math".to_owned()),
            ("Linus".to_owned(), "systems".to_owned())
        ]
    );

    let aggregate = RqlStatement::Select(RqlSelect {
        distinct: false,
        projection: vec![expr_item(RqlExpr::CountStar)],
        from: Some(table("people")),
        joins: Vec::new(),
        filter: None,
        group_by: Vec::new(),
        having: None,
        order_by: Vec::new(),
        limit: None,
        offset: None,
    });
    let mut stmt = conn.prepare_rql(&aggregate).expect("aggregate");
    match stmt.step().expect("step") {
        Step::Row(row) => assert_eq!(row.get::<i64>(0).expect("count"), 2),
        Step::Done => panic!("expected aggregate row"),
    }

    let subquery_select = RqlStatement::Select(RqlSelect {
        distinct: false,
        projection: vec![expr_item(column("person_name"))],
        from: Some(table("people")),
        joins: Vec::new(),
        filter: Some(eq(
            column("dept_id"),
            RqlExpr::Subquery {
                select: Box::new(RqlSelect {
                    distinct: false,
                    projection: vec![expr_item(column("id"))],
                    from: Some(table("departments")),
                    joins: Vec::new(),
                    filter: Some(eq(column("dept_name"), text("math"))),
                    group_by: Vec::new(),
                    having: None,
                    order_by: Vec::new(),
                    limit: None,
                    offset: None,
                }),
            },
        )),
        group_by: Vec::new(),
        having: None,
        order_by: Vec::new(),
        limit: None,
        offset: None,
    });
    let mut stmt = conn.prepare_rql(&subquery_select).expect("subquery");
    match stmt.step().expect("step") {
        Step::Row(row) => assert_eq!(row.get::<String>(0).expect("name"), "Ada"),
        Step::Done => panic!("expected subquery row"),
    }

    conn.execute_rql(&RqlProgram {
        statements: vec![RqlStatement::Delete(RqlDelete {
            table: name("people"),
            filter: Some(eq(column("id"), int(1))),
        })],
    })
    .expect("delete");

    let mut stmt = conn
        .prepare_rql(&aggregate)
        .expect("aggregate after delete");
    match stmt.step().expect("step") {
        Step::Row(row) => assert_eq!(row.get::<i64>(0).expect("count"), 1),
        Step::Done => panic!("expected aggregate row"),
    }
}
