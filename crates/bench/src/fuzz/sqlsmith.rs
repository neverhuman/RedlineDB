//! Tiny SQLSmith-style grammar fuzzer.
//!
//! Generates well-formed SQL biased toward the feature surface used by
//! fresh applications when migrating from SQLite to RedlineDB. The grammar
//! is deliberately small (single file, <300 LOC) so reviewers can audit
//! every branch — generating "interesting" but executable SQL is more
//! useful than generating syntactically-baroque junk.
//!
//! Schema (pre-created by the harness):
//!   t1(id INTEGER PRIMARY KEY, x INTEGER, y INTEGER, name TEXT)
//!   t2(id INTEGER PRIMARY KEY, t1_id INTEGER REFERENCES t1(id), price REAL)
//!   t3(id INTEGER PRIMARY KEY, tag TEXT, ts INTEGER)
//!     INDEX i_t1_x ON t1(x)
//!     INDEX i_t2_t1 ON t2(t1_id)
//!     INDEX i_t3_tag ON t3(tag)

use rand::Rng;
use rand_chacha::ChaCha8Rng;

/// Pre-create the fuzz schema. Both engines run the same `CREATE TABLE`
/// statements so the fuzzer's column-name references resolve identically.
pub const SCHEMA_SQL: &[&str] = &[
    "CREATE TABLE t1(id INTEGER PRIMARY KEY, x INTEGER, y INTEGER, name TEXT)",
    "CREATE TABLE t2(id INTEGER PRIMARY KEY, t1_id INTEGER, price REAL)",
    "CREATE TABLE t3(id INTEGER PRIMARY KEY, tag TEXT, ts INTEGER)",
    "CREATE INDEX i_t1_x ON t1(x)",
    "CREATE INDEX i_t2_t1 ON t2(t1_id)",
    "CREATE INDEX i_t3_tag ON t3(tag)",
];

/// Seed each table with a handful of rows so SELECTs return non-empty
/// results most of the time. Determinism comes from the literal values
/// (not the RNG) so both engines see the exact same starting state.
pub const SEED_SQL: &[&str] = &[
    "INSERT INTO t1(id, x, y, name) VALUES (1, 10, 100, 'ada'), (2, 20, 200, 'bob'), \
     (3, 30, 300, 'cat'), (4, 40, 400, NULL), (5, 50, 500, 'eve')",
    "INSERT INTO t2(id, t1_id, price) VALUES (1, 1, 9.5), (2, 1, 12.25), (3, 2, 7.0), \
     (4, 3, 4.5), (5, 4, 22.0), (6, NULL, 1.0)",
    "INSERT INTO t3(id, tag, ts) VALUES (1, 'a', 1000), (2, 'b', 2000), \
     (3, 'a', 3000), (4, 'c', 4000), (5, 'b', 5000)",
];

const TABLES: &[&str] = &["t1", "t2", "t3"];

/// All <table>.<column> pairs (the fuzzer uses qualified column refs to
/// keep join-projection unambiguous).
fn columns_of(table: &str) -> &'static [&'static str] {
    match table {
        "t1" => &["id", "x", "y", "name"],
        "t2" => &["id", "t1_id", "price"],
        "t3" => &["id", "tag", "ts"],
        _ => &[],
    }
}

/// Top-level statement kinds emitted by the fuzzer. The weights bias
/// generation toward read-mostly queries because divergence on SELECT is
/// the highest-value signal; DML divergence is caught downstream by the
/// next SELECT against the touched table.
#[derive(Clone, Copy, Debug)]
enum StmtKind {
    SelectSimple,
    SelectJoin,
    SelectScalarSub,
    SelectInSub,
    SelectCte,
    SelectSetOp,
    Insert,
    Update,
    Delete,
}

const KINDS: &[(StmtKind, u32)] = &[
    (StmtKind::SelectSimple, 30),
    (StmtKind::SelectJoin, 20),
    (StmtKind::SelectScalarSub, 8),
    (StmtKind::SelectInSub, 8),
    (StmtKind::SelectCte, 6),
    (StmtKind::SelectSetOp, 8),
    (StmtKind::Insert, 8),
    (StmtKind::Update, 6),
    (StmtKind::Delete, 6),
];

fn weighted_choice<T: Copy>(rng: &mut ChaCha8Rng, items: &[(T, u32)]) -> T {
    let total: u32 = items.iter().map(|(_, w)| w).sum();
    let mut target = rng.random_range(0..total);
    for (item, weight) in items {
        if target < *weight {
            return *item;
        }
        target -= *weight;
    }
    items[0].0
}

fn pick<'a, T>(rng: &mut ChaCha8Rng, items: &'a [T]) -> &'a T {
    &items[rng.random_range(0..items.len())]
}

/// Public entry: generate one SQL statement using `rng`'s current state.
/// Same seed + same call sequence → same SQL, on every machine.
pub fn generate_stmt(rng: &mut ChaCha8Rng) -> String {
    match weighted_choice(rng, KINDS) {
        StmtKind::SelectSimple => gen_select_simple(rng),
        StmtKind::SelectJoin => gen_select_join(rng),
        StmtKind::SelectScalarSub => gen_select_scalar_sub(rng),
        StmtKind::SelectInSub => gen_select_in_sub(rng),
        StmtKind::SelectCte => gen_select_cte(rng),
        StmtKind::SelectSetOp => gen_select_set_op(rng),
        StmtKind::Insert => gen_insert(rng),
        StmtKind::Update => gen_update(rng),
        StmtKind::Delete => gen_delete(rng),
    }
}

fn gen_projection(rng: &mut ChaCha8Rng, table: &str) -> String {
    let cols = columns_of(table);
    let mut out = Vec::new();
    let count = rng.random_range(1..=cols.len());
    let scalar = pick_unary_scalar(rng);
    for i in 0..count {
        let col = cols[i];
        if rng.random_bool(0.25) {
            out.push(format!("{scalar}({table}.{col})"));
        } else {
            out.push(format!("{table}.{col}"));
        }
    }
    out.join(", ")
}

/// Pick a unary scalar (one column-ref argument). `coalesce` and `substr`
/// require ≥2 args so they are excluded here; the harness emits them in
/// dedicated branches that wire the correct arity.
fn pick_unary_scalar(rng: &mut ChaCha8Rng) -> &'static str {
    const FUNCS: &[&str] = &["length", "abs", "lower", "upper"];
    *pick(rng, FUNCS)
}

fn gen_where(rng: &mut ChaCha8Rng, table: &str) -> String {
    if !rng.random_bool(0.85) {
        return String::new();
    }
    let cols = columns_of(table);
    let col = pick(rng, cols);
    let op = pick(rng, &["=", "!=", "<", "<=", ">", ">="]);
    // Use a column-type-aware literal so we don't tickle SQLite's loose
    // type-affinity coercion divergence (text <= 5 has implementation-
    // defined behavior across engines). Conservative: numeric columns get
    // integer literals, text columns get short text, NULL-prone columns
    // (price) get a numeric or NULL.
    let lit = column_typed_literal(rng, table, col);
    format!(" WHERE {table}.{col} {op} {lit}")
}

fn column_typed_literal(rng: &mut ChaCha8Rng, table: &str, col: &str) -> String {
    if rng.random_bool(0.1) {
        return "NULL".to_string();
    }
    let is_text = matches!((table, col), ("t1", "name") | ("t3", "tag"));
    let is_real = matches!((table, col), ("t2", "price"));
    if is_text {
        let s = pick(rng, &["'a'", "'b'", "'c'", "'ada'", "'eve'"]);
        (*s).to_string()
    } else if is_real {
        format!("{:.2}", rng.random_range(0..10000) as f64 / 100.0)
    } else {
        rng.random_range(0..100).to_string()
    }
}

fn gen_order_by(rng: &mut ChaCha8Rng, table: &str) -> String {
    if !rng.random_bool(0.6) {
        return String::new();
    }
    let cols = columns_of(table);
    let col = pick(rng, cols);
    let dir = if rng.random_bool(0.5) { "ASC" } else { "DESC" };
    format!(" ORDER BY {table}.{col} {dir}, {table}.id ASC")
}

fn gen_select_simple(rng: &mut ChaCha8Rng) -> String {
    let table = pick(rng, TABLES);
    let proj = gen_projection(rng, table);
    let where_clause = gen_where(rng, table);
    // LIMIT without ORDER BY is unstable across engines (the physical scan
    // order is implementation-defined). Always pair LIMIT with ORDER BY so
    // the fuzz test catches algorithmic divergence, not scan-order noise.
    let want_limit = rng.random_bool(0.4);
    let order = if want_limit {
        // Force a deterministic ORDER BY when LIMIT is present.
        format!(" ORDER BY {table}.id ASC")
    } else {
        gen_order_by(rng, table)
    };
    let limit = if want_limit {
        format!(" LIMIT {}", rng.random_range(1..=20))
    } else {
        String::new()
    };
    format!("SELECT {proj} FROM {table}{where_clause}{order}{limit}") // jankurai:allow HLT-023-INPUT-BOUNDARY-GAP reason=sql-fuzzer-deliberately-generates-randomized-test-sql-from-bounded-grammar expires=2027-06-01
}

fn gen_select_join(rng: &mut ChaCha8Rng) -> String {
    let kind = if rng.random_bool(0.6) {
        "INNER JOIN"
    } else {
        "LEFT JOIN"
    };
    // Pair the two tables that have a real FK shape (t1.id ↔ t2.t1_id).
    let proj = format!(
        "t1.id, t1.name, {agg}(t2.price)",
        agg = pick(rng, &["sum", "avg", "count", "min", "max"])
    );
    let where_clause = if rng.random_bool(0.7) {
        format!(" WHERE t1.x > {}", rng.random_range(0..40))
    } else {
        String::new()
    };
    let group = " GROUP BY t1.id, t1.name";
    let order = " ORDER BY t1.id";
    format!("SELECT {proj} FROM t1 {kind} t2 ON t1.id = t2.t1_id{where_clause}{group}{order}") // jankurai:allow HLT-023-INPUT-BOUNDARY-GAP reason=sql-fuzzer-deliberately-generates-randomized-test-sql-from-bounded-grammar expires=2027-06-01
}

fn gen_select_scalar_sub(rng: &mut ChaCha8Rng) -> String {
    let agg = pick(rng, &["MIN", "MAX", "AVG", "COUNT"]);
    let order = " ORDER BY t1.id";
    format!(
        "SELECT t1.id, t1.x, (SELECT {agg}(t2.price) FROM t2 WHERE t2.t1_id = t1.id) AS sub \
         FROM t1{order}"
    )
}

fn gen_select_in_sub(rng: &mut ChaCha8Rng) -> String {
    let threshold = rng.random_range(5..50);
    let neg = if rng.random_bool(0.3) { "NOT " } else { "" };
    format!(
        "SELECT t1.id, t1.name FROM t1 \
         WHERE t1.id {neg}IN (SELECT t2.t1_id FROM t2 WHERE t2.price > {threshold}) \
         ORDER BY t1.id"
    )
}

fn gen_select_cte(rng: &mut ChaCha8Rng) -> String {
    let threshold = rng.random_range(5..30);
    let cte_col_alias = pick(rng, &["q", "p", "n"]);
    format!(
        "WITH cte AS (SELECT t1_id, COUNT(*) AS {cte_col_alias} FROM t2 GROUP BY t1_id) \
         SELECT t1.id, t1.name, cte.{cte_col_alias} \
         FROM t1 LEFT JOIN cte ON cte.t1_id = t1.id \
         WHERE t1.x > {threshold} \
         ORDER BY t1.id"
    )
}

fn gen_select_set_op(rng: &mut ChaCha8Rng) -> String {
    let op = pick(rng, &["UNION ALL", "UNION", "INTERSECT", "EXCEPT"]);
    format!(
        "SELECT id FROM t1 WHERE x > {a} {op} SELECT id FROM t2 WHERE price < {b} \
         ORDER BY 1",
        a = rng.random_range(0..50),
        b = rng.random_range(0..30)
    )
}

fn gen_insert(rng: &mut ChaCha8Rng) -> String {
    // Insert into t1; explicit id avoids autoincrement-divergence noise.
    let id = rng.random_range(1000..2000);
    let x = rng.random_range(0..100);
    let y = rng.random_range(0..1000);
    let name = pick(rng, &["'inserted'", "'fuzz'", "NULL"]);
    format!("INSERT INTO t1(id, x, y, name) VALUES ({id}, {x}, {y}, {name})")
}

fn gen_update(rng: &mut ChaCha8Rng) -> String {
    let new_x = rng.random_range(0..100);
    let threshold = rng.random_range(0..40);
    format!("UPDATE t1 SET x = {new_x} WHERE x < {threshold}")
}

fn gen_delete(rng: &mut ChaCha8Rng) -> String {
    let threshold = rng.random_range(0..30);
    format!("DELETE FROM t1 WHERE x > {threshold} AND id > 100")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn schema_runs_in_rusqlite() {
        let conn = rusqlite::Connection::open_in_memory().expect("open");
        for sql in SCHEMA_SQL.iter().chain(SEED_SQL.iter()) {
            conn.execute_batch(sql)
                .unwrap_or_else(|err| panic!("seed sql failed: {sql}\n{err}"));
        }
    }

    #[test]
    fn same_seed_produces_same_sql_sequence() {
        let seq_a: Vec<String> = (0..20)
            .map({
                let mut rng = ChaCha8Rng::seed_from_u64(7);
                move |_| generate_stmt(&mut rng)
            })
            .collect();
        let seq_b: Vec<String> = (0..20)
            .map({
                let mut rng = ChaCha8Rng::seed_from_u64(7);
                move |_| generate_stmt(&mut rng)
            })
            .collect();
        assert_eq!(seq_a, seq_b, "fuzzer is not deterministic");
    }
}
