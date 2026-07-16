//! Governed used-operation corpus shared byte-for-byte by all three adapters.

use crate::{Db, Error, Identifier, Result, SqlPart, Statement, Value, GOVERNED_CAPABILITIES};

pub const VERSION: &str = "db-shim.used-operations/v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub schema_version: &'static str,
    pub rows: usize,
    pub rollback_preserved_rows: bool,
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
        (Ok(report), Ok(_)) => Ok(report),
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
    }
}

fn run_inner(db: &mut Db, table: &Identifier) -> Result<Report> {
    db.execute(&table_statement(
        "CREATE TABLE ",
        table,
        "(id BIGINT PRIMARY KEY, name TEXT NOT NULL, score DOUBLE PRECISION NOT NULL, payload BYTEA)",
    )?)?;

    for (id, name, score, payload) in [
        (1, "ada", 9.5, vec![0_u8, 1]),
        (2, "lin", 8.0, vec![2_u8, 3]),
        (3, "grace", 10.0, vec![4_u8, 5]),
    ] {
        db.execute(&insert_statement(table, id, name, score, payload)?)?;
    }

    let rollback_result: Result<()> = db.transaction(|transaction| {
        transaction.execute(&insert_statement(table, 99, "rollback", 0.0, Vec::new())?)?;
        Err(Error::Contract("intentional rollback probe".to_owned()))
    });
    if !matches!(rollback_result, Err(Error::Contract(message)) if message == "intentional rollback probe")
    {
        return Err(Error::Contract(
            "rollback probe did not return its governed error".to_owned(),
        ));
    }

    let count = db.query_row(&table_statement("SELECT COUNT(*) FROM ", table, "")?)?;
    if count != vec![Value::Integer(3)] {
        return Err(Error::Contract(format!(
            "rollback row count differs: {count:?}"
        )));
    }

    let rows = db.query(&Statement::compose([
        SqlPart::Syntax("SELECT id,name,payload FROM "),
        SqlPart::Identifier(table.clone()),
        SqlPart::Syntax(" WHERE score >= "),
        SqlPart::Parameter(Value::Real(9.0)),
        SqlPart::Syntax(" ORDER BY id"),
    ])?)?;
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

    Ok(Report {
        schema_version: VERSION,
        rows: 3,
        rollback_preserved_rows: true,
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
) -> Result<Statement> {
    Statement::compose([
        SqlPart::Syntax("INSERT INTO "),
        SqlPart::Identifier(table.clone()),
        SqlPart::Syntax("(id,name,score,payload) VALUES ("),
        SqlPart::Parameter(Value::Integer(id)),
        SqlPart::Syntax(","),
        SqlPart::Parameter(Value::Text(name.to_owned())),
        SqlPart::Syntax(","),
        SqlPart::Parameter(Value::Real(score)),
        SqlPart::Syntax(","),
        SqlPart::Parameter(Value::Blob(payload)),
        SqlPart::Syntax(")"),
    ])
}
