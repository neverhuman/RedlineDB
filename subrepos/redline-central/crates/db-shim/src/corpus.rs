//! Governed used-operation corpus shared byte-for-byte by all three adapters.

use crate::{
    Db, Error, Identifier, Result, SqlPart, Statement, Value, ValueType, GOVERNED_CAPABILITIES,
};

pub const VERSION: &str = "db-shim.used-operations/v2";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub schema_version: &'static str,
    pub rows: usize,
    pub rollback_preserved_rows: bool,
    pub typed_null_round_trip: bool,
    pub execute_success_only: bool,
}

pub fn run(db: &mut Db) -> Result<Report> {
    if db.capabilities() != GOVERNED_CAPABILITIES {
        return Err(Error::Contract(
            "selected adapter does not provide the governed capabilities".to_owned(),
        ));
    }
    let table = db.table("backend_corpus")?;
    let drop = table_statement("DROP TABLE IF EXISTS ", &table, "")?;
    db.execute(&drop)?;
    let result = run_inner(db, &table);
    let cleanup = db.execute(&drop);
    match (result, cleanup) {
        (Ok(report), Ok(())) => Ok(report),
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
    }
}

fn run_inner(db: &mut Db, table: &Identifier) -> Result<Report> {
    let (): () = db.execute(&table_statement(
        "CREATE TABLE ",
        table,
        "(id BIGINT PRIMARY KEY, name TEXT NOT NULL, score DOUBLE PRECISION NOT NULL, payload BYTEA, nullable_integer BIGINT, nullable_real DOUBLE PRECISION, nullable_text TEXT, nullable_blob BYTEA)",
    )?)?;

    for (id, name, score, payload, nullable_values) in [
        (
            1,
            "ada",
            9.5,
            vec![0_u8, 1],
            [
                Value::Integer(101),
                Value::Real(1.25),
                Value::Text("present".to_owned()),
                Value::Blob(vec![10, 11]),
            ],
        ),
        (
            2,
            "lin",
            8.0,
            vec![2_u8, 3],
            [
                Value::Integer(202),
                Value::Real(2.5),
                Value::Text("also-present".to_owned()),
                Value::Blob(vec![12, 13]),
            ],
        ),
        (
            3,
            "grace",
            10.0,
            vec![4_u8, 5],
            [
                Value::Null(ValueType::Integer),
                Value::Null(ValueType::Real),
                Value::Null(ValueType::Text),
                Value::Null(ValueType::Blob),
            ],
        ),
    ] {
        db.execute(&insert_statement(
            table,
            id,
            name,
            score,
            payload,
            nullable_values,
        )?)?;
    }

    let rollback_result: Result<()> = db.transaction(|transaction| {
        transaction.execute(&insert_statement(
            table,
            99,
            "rollback",
            0.0,
            Vec::new(),
            [
                Value::Integer(999),
                Value::Real(99.0),
                Value::Text("rollback".to_owned()),
                Value::Blob(Vec::new()),
            ],
        )?)?;
        Err(Error::Contract("intentional rollback probe".to_owned()))
    });
    if !matches!(rollback_result, Err(Error::Contract(message)) if message == "intentional rollback probe")
    {
        return Err(Error::Contract(
            "rollback probe did not return its governed error".to_owned(),
        ));
    }

    let count = db.query_row(
        &table_statement("SELECT COUNT(*) FROM ", table, "")?.returning([ValueType::Integer])?,
    )?;
    if count != vec![Value::Integer(3)] {
        return Err(Error::Contract(format!(
            "rollback row count differs: {count:?}"
        )));
    }

    let rows = db.query(
        &Statement::compose([
            SqlPart::Syntax("SELECT id,name,payload FROM "),
            SqlPart::Identifier(table.clone()),
            SqlPart::Syntax(" WHERE score >= "),
            SqlPart::Parameter(Value::Real(9.0)),
            SqlPart::Syntax(" ORDER BY id"),
        ])?
        .returning([ValueType::Integer, ValueType::Text, ValueType::Blob])?,
    )?;
    let expected = vec![
        vec![
            Value::Integer(1),
            Value::Text("ada".to_owned()),
            Value::Blob(vec![0, 1]),
        ],
        vec![
            Value::Integer(3),
            Value::Text("grace".to_owned()),
            Value::Blob(vec![4, 5]),
        ],
    ];
    if rows != expected {
        return Err(Error::Contract(format!(
            "parameterized query differs: {rows:?}"
        )));
    }

    let null_row = db.query_row(
        &Statement::compose([
            SqlPart::Syntax(
                "SELECT id,nullable_integer,nullable_real,nullable_text,nullable_blob FROM ",
            ),
            SqlPart::Identifier(table.clone()),
            SqlPart::Syntax(" WHERE id = "),
            SqlPart::Parameter(Value::Integer(3)),
        ])?
        .returning([
            ValueType::Integer,
            ValueType::Integer,
            ValueType::Real,
            ValueType::Text,
            ValueType::Blob,
        ])?,
    )?;
    let expected_null_row = vec![
        Value::Integer(3),
        Value::Null(ValueType::Integer),
        Value::Null(ValueType::Real),
        Value::Null(ValueType::Text),
        Value::Null(ValueType::Blob),
    ];
    if null_row != expected_null_row {
        return Err(Error::Contract(format!(
            "typed null round trip differs: {null_row:?}"
        )));
    }

    Ok(Report {
        schema_version: VERSION,
        rows: 3,
        rollback_preserved_rows: true,
        typed_null_round_trip: true,
        execute_success_only: true,
    })
}

fn table_statement(
    prefix: &'static str,
    table: &Identifier,
    suffix: &'static str,
) -> Result<Statement> {
    Statement::compose([
        SqlPart::Syntax(prefix),
        SqlPart::Identifier(table.clone()),
        SqlPart::Syntax(suffix),
    ])
}

fn insert_statement(
    table: &Identifier,
    id: i64,
    name: &str,
    score: f64,
    payload: Vec<u8>,
    nullable_values: [Value; 4],
) -> Result<Statement> {
    let [nullable_integer, nullable_real, nullable_text, nullable_blob] = nullable_values;
    Statement::compose([
        SqlPart::Syntax("INSERT INTO "),
        SqlPart::Identifier(table.clone()),
        SqlPart::Syntax("(id,name,score,payload,nullable_integer,nullable_real,nullable_text,nullable_blob) VALUES ("),
        SqlPart::Parameter(Value::Integer(id)),
        SqlPart::Syntax(","),
        SqlPart::Parameter(Value::Text(name.to_owned())),
        SqlPart::Syntax(","),
        SqlPart::Parameter(Value::Real(score)),
        SqlPart::Syntax(","),
        SqlPart::Parameter(Value::Blob(payload)),
        SqlPart::Syntax(","),
        SqlPart::Parameter(nullable_integer),
        SqlPart::Syntax(","),
        SqlPart::Parameter(nullable_real),
        SqlPart::Syntax(","),
        SqlPart::Parameter(nullable_text),
        SqlPart::Syntax(","),
        SqlPart::Parameter(nullable_blob),
        SqlPart::Syntax(")"),
    ])
}
