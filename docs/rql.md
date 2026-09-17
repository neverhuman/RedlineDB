# RQL

RQL is the Redline Query Language. In v0.1 it is an additive, default-off typed
relational IR for RedlineDB, not an alternate SQL keyword syntax.

SQL remains the compatibility frontend. RQL callers submit a `RqlProgram` or
`RqlStatement` through Rust APIs, or JSON through `redlinedb --rql <db>`.
The RQL path lowers directly into existing prepared templates and executor
plans. It does not render SQL text and does not round-trip through the SQL
parser.

## Rust APIs

The public API surface is:

- `Connection::prepare_rql(&RqlStatement)`
- `Connection::execute_rql(&RqlProgram)`
- `Database::prepare_rql(&RqlStatement)`

The typed IR covers the phase-1 relational surface: create/drop table and
index, insert values, update, delete, select, filters, joins, grouping,
ordering, limits, scalar expressions, functions, casts, and simple subqueries.

## CLI

`redlinedb --rql <db>` reads RQL JSON from stdin and renders query output
through the normal CLI output modes.

```json
{
  "statements": [
    {
      "type": "create_table",
      "table": { "name": "items" },
      "columns": [
        { "name": "id", "declared_type": "INTEGER", "primary_key": true },
        { "name": "label", "declared_type": "TEXT" }
      ]
    },
    {
      "type": "insert",
      "table": { "name": "items" },
      "columns": ["id", "label"],
      "values": [[
        { "type": "integer", "value": 1 },
        { "type": "text", "value": "Ada" }
      ]]
    },
    {
      "type": "select",
      "projection": [
        {
          "type": "expr",
          "expr": { "type": "column", "column": { "name": "label" } }
        }
      ],
      "from": { "name": { "name": "items" } }
    }
  ]
}
```

`redline-testing --suite rql_phase1` keeps the existing SQLite parity SQL suite
unchanged. It runs SQLite against the original SQL case and RedlineDB against
the generated RQL target input, producing the same JSONL evidence format used
by the existing benchmark/report pipeline.
