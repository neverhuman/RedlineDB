use super::*;
use redlinedb_kernel::catalog::{AlterTableOperationSpec, resolve_schema_id};

pub(super) fn rewrite_drop_column_rows(
    conn: &Connection,
    tx: &mut Txn,
    spec: &redlinedb_kernel::catalog::AlterTableSpec,
) -> Result<()> {
    let AlterTableOperationSpec::DropColumn {
        column_name,
        if_exists: _,
    } = &spec.operation
    else {
        return Ok(());
    };

    let snapshot = conn.engine().schema_snapshot_for_tx(tx);
    let schema_id = resolve_schema_id(&snapshot, Some(&spec.name.schema))?;
    let Some(table) = snapshot.lookup_table(schema_id, spec.name.name.folded()) else {
        return Ok(());
    };
    let Some(drop_ordinal) = table
        .columns
        .iter()
        .position(|column| column.folded.as_ref() == column_name.folded())
    else {
        return Ok(());
    };

    let rowids = collect_table_rowids(conn.engine(), tx, &table)?;
    if rowids.is_empty() {
        return Ok(());
    }

    let mut rewrites = Vec::with_capacity(rowids.len());
    for rowid in rowids {
        let Some(row) = load_table_row_by_rowid(conn.engine(), tx, &table, rowid)? else {
            continue;
        };
        let mut values = row.values;
        if drop_ordinal as usize >= values.len() {
            return Err(Error::UnsupportedSql(
                "ALTER TABLE DROP COLUMN rewrite hit short row payload".to_owned(),
            ));
        }
        values.remove(drop_ordinal);
        let payload = encode_sql_row(table.table_id.0, &values)?;
        rewrites.push((rowid, payload));
    }

    for (rowid, payload) in rewrites {
        conn.engine()
            .update_for_relation(tx, table.relation_id, rowid, payload)?;
    }

    Ok(())
}
