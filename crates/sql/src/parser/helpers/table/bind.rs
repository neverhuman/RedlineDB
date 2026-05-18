use std::sync::Arc;

use redlinedb_kernel::catalog::{SchemaSnapshot, lookup_table};
use sqlparser::ast::{ObjectName, ObjectNamePart, TableFactor, TableObject, TableWithJoins};

use crate::error::{Error, Result};

use super::super::expr::parse_qualified_name;

pub(crate) fn object_name_part_to_string(part: &ObjectNamePart) -> Result<String> {
    match part {
        ObjectNamePart::Identifier(ident) => Ok(ident.value.clone()),
        ObjectNamePart::Function(_) => Err(Error::UnsupportedSql(
            "function-style object names are not supported".to_owned(),
        )),
    }
}

pub(crate) fn bind_table_name(
    schema: &SchemaSnapshot,
    name: &ObjectName,
) -> Result<Arc<redlinedb_kernel::catalog::TableDef>> {
    // View resolution: if the name resolves to a persisted view, materialize
    // its body and return a synthetic TableDef backed by row storage.
    if let Some(bound) = crate::exec::view::try_resolve_view_bound_table(schema, name, None)? {
        return Ok(bound.table);
    }
    // Cross-database write rejection: callers from DML paths reach here with
    // `alias.table`. Reads route through `bind_select_table_factor` and never
    // hit this code path for cross-DB names; only writes do.
    if crate::exec::cross_db::is_cross_db_name(name) {
        return Err(Error::UnsupportedSql(
            "cross-database writes are not yet supported".to_owned(),
        ));
    }
    let qualified = parse_qualified_name(name.clone())?;
    Ok(lookup_table(schema, &qualified)?)
}

pub(crate) fn bind_table_object(
    schema: &SchemaSnapshot,
    table: &TableObject,
) -> Result<Arc<redlinedb_kernel::catalog::TableDef>> {
    match table {
        TableObject::TableName(name) => bind_table_name(schema, name),
        TableObject::TableFunction(_) => Err(Error::UnsupportedSql(
            "table functions are not supported".to_owned(),
        )),
    }
}

pub(crate) fn bind_table_with_joins(
    schema: &SchemaSnapshot,
    table: &TableWithJoins,
) -> Result<Arc<redlinedb_kernel::catalog::TableDef>> {
    if !table.joins.is_empty() {
        return Err(Error::UnsupportedSql(
            "joins are not supported in UPDATE/DELETE targets yet".to_owned(),
        ));
    }
    match &table.relation {
        TableFactor::Table { name, args, .. } => {
            if args.is_some() {
                return Err(Error::UnsupportedSql(
                    "table-valued functions are not supported".to_owned(),
                ));
            }
            bind_table_name(schema, name)
        }
        _ => Err(Error::UnsupportedSql(
            "only direct table scans are supported".to_owned(),
        )),
    }
}
