use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::{Error, Result};

use super::codec::{BytesReader, BytesWriter, frame_snapshot, parse_header};
use super::ddl::{ConflictAction, IndexOrigin};
use super::expr::{CompiledExpr, ExprOp};
use super::ids::SchemaId;
use super::key::{IndexKeyDef, IndexKeySource, NullOrder, SortDir};
use super::schema::{
    CatalogMeta, CheckDef, ColumnDef, ConstraintDef, ConstraintKind, IndexDef, NamespaceDef,
    SchemaEpoch, SchemaSnapshot, TableDef,
};
use super::value::OwnedValue;
use crate::format::{PageId, RelId};

const MAGIC: u32 = 0x5243_4154; // "RCAT"
const VERSION: u16 = 1;

#[derive(Debug, Clone)]
pub struct CatalogStore {
    path: PathBuf,
}

impl CatalogStore {
    pub fn new(base: impl AsRef<Path>) -> Self {
        Self {
            path: base.as_ref().join("schema.redline"),
        }
    }

    pub fn load(&self) -> Result<Option<Arc<SchemaSnapshot>>> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err.into()),
        };
        decode_snapshot_file(&bytes).map(Some)
    }

    pub fn save(&self, snapshot: &SchemaSnapshot) -> Result<()> {
        self.save_atomic(snapshot)
    }

    pub fn save_atomic(&self, snapshot: &SchemaSnapshot) -> Result<()> {
        let bytes = encode_snapshot_file(snapshot)?;
        let staging = self.path.with_extension("tmp");
        {
            // Lane E failpoint: armed before the staging catalog file is
            // created. A crash here yields no `.tmp`, so recovery must
            // observe the prior catalog generation untouched.
            crate::fail_point!("catalog::save::temp_write");
            let mut file = fs::File::create(&staging)?;
            file.write_all(&bytes)?;
            // Lane E failpoint: armed after the staging write but before
            // fsync. Crashing here lets the OS keep the staging file in
            // page cache only; recovery must still see the prior atomic
            // snapshot.
            crate::fail_point!("catalog::save::fsync");
            file.sync_all()?;
        }
        // Lane E failpoint: armed before the atomic rename. The staging
        // file is fully durable on disk; a crash here guarantees the
        // rename never happened, so the prior schema snapshot remains
        // the canonical one.
        crate::fail_point!("catalog::save::rename");
        fs::rename(staging, &self.path)?;
        if let Some(parent) = self.path.parent() {
            // Lane E failpoint: armed before the parent-directory fsync that
            // makes the rename durable. A crash here may lose the rename even
            // though the inode bytes are durable, exercising the parent-fsync
            // contract.
            crate::fail_point!("catalog::save::parent_fsync");
            let dir = fs::File::open(parent)?;
            dir.sync_all()?;
        }
        Ok(())
    }
}

pub fn encode_snapshot(snapshot: &SchemaSnapshot) -> Result<Vec<u8>> {
    let mut out = BytesWriter::new();
    out.u64(snapshot.meta.format_version);
    out.u64(snapshot.meta.schema_epoch.0);
    out.u64(snapshot.meta.next_object_id.0);
    out.u64(snapshot.meta.next_relation_id.0);
    out.bytes(&snapshot.meta.database_uuid);

    out.u32(snapshot.namespaces.len() as u32);
    for namespace in &snapshot.namespaces {
        encode_namespace(&mut out, namespace)?;
    }

    out.u32(snapshot.tables.len() as u32);
    for table in &snapshot.tables {
        encode_table(&mut out, table)?;
    }

    Ok(out.finish())
}

pub fn decode_snapshot(bytes: &[u8]) -> Result<SchemaSnapshot> {
    let mut reader = BytesReader::new(bytes);
    let format_version = reader.u64()?;
    if format_version > 2 {
        return Err(Error::UnsupportedVersion(format_version as u16));
    }
    let meta = CatalogMeta {
        format_version,
        schema_epoch: SchemaEpoch(reader.u64()?),
        next_object_id: super::ObjectId(reader.u64()?),
        next_relation_id: RelId(reader.u64()?),
        database_uuid: reader.take_array()?,
    };

    let namespace_count = reader.u32()? as usize;
    let mut snapshot = SchemaSnapshot::empty(meta);
    for _ in 0..namespace_count {
        snapshot.namespaces.push(decode_namespace(&mut reader)?);
    }

    let table_count = reader.u32()? as usize;
    for _ in 0..table_count {
        snapshot
            .tables
            .push(Arc::new(decode_table(&mut reader, format_version)?));
    }
    snapshot.rebuild_indexes();
    if reader.remaining() != 0 {
        return Err(Error::CatalogCorrupt("catalog snapshot has trailing bytes"));
    }
    Ok(snapshot)
}

fn encode_snapshot_file(snapshot: &SchemaSnapshot) -> Result<Vec<u8>> {
    let payload = encode_snapshot(snapshot)?;
    Ok(frame_snapshot(MAGIC, VERSION, &payload))
}

fn decode_snapshot_file(bytes: &[u8]) -> Result<Arc<SchemaSnapshot>> {
    let frame = parse_header(
        bytes,
        MAGIC,
        Error::CatalogCorrupt("catalog snapshot file too small"),
        Error::CatalogCorrupt("catalog snapshot magic mismatch"),
        Error::CatalogCorrupt("catalog snapshot length overflow"),
        Error::CatalogCorrupt("catalog snapshot length mismatch"),
        VERSION,
        Error::UnsupportedVersion,
    )?;
    Ok(Arc::new(decode_snapshot(frame.payload)?))
}

fn encode_namespace(out: &mut BytesWriter, namespace: &NamespaceDef) -> Result<()> {
    out.u64(namespace.schema_id.0);
    write_str(out, &namespace.name);
    write_str(out, &namespace.folded);
    Ok(())
}

fn decode_namespace(reader: &mut BytesReader<'_>) -> Result<NamespaceDef> {
    Ok(NamespaceDef {
        schema_id: SchemaId(reader.u64()?),
        name: read_box_str(reader)?,
        folded: read_box_str(reader)?,
    })
}

fn encode_table(out: &mut BytesWriter, table: &TableDef) -> Result<()> {
    out.u64(table.table_id.0);
    out.u64(table.schema_id.0);
    out.u64(table.relation_id.0);
    write_str(out, &table.name);
    write_str(out, &table.folded);
    out.u64(table.flags);
    write_opt_u16(out, table.rowid_alias_column);
    write_opt_str(out, table.normalized_sql.as_deref());

    out.u32(table.columns.len() as u32);
    for column in &table.columns {
        encode_column(out, column)?;
    }

    out.u32(table.indexes.len() as u32);
    for index in &table.indexes {
        encode_index(out, index)?;
    }

    out.u32(table.constraints.len() as u32);
    for constraint in &table.constraints {
        encode_constraint(out, constraint)?;
    }

    out.u32(table.checks.len() as u32);
    for check in &table.checks {
        encode_check(out, check)?;
    }

    Ok(())
}

fn decode_table(reader: &mut BytesReader<'_>, format_version: u64) -> Result<TableDef> {
    let table_id = super::TableId(reader.u64()?);
    let schema_id = SchemaId(reader.u64()?);
    let relation_id = RelId(reader.u64()?);
    let name = read_box_str(reader)?;
    let folded = read_box_str(reader)?;
    let flags = reader.u64()?;
    let rowid_alias_column = read_opt_u16(reader)?;
    let normalized_sql = read_opt_box_str(reader)?;

    let column_count = reader.u32()? as usize;
    let mut columns = Vec::with_capacity(column_count);
    for _ in 0..column_count {
        columns.push(decode_column(reader)?);
    }

    let index_count = reader.u32()? as usize;
    let mut indexes = Vec::with_capacity(index_count);
    for _ in 0..index_count {
        indexes.push(decode_index(reader, format_version)?);
    }

    let constraint_count = reader.u32()? as usize;
    let mut constraints = Vec::with_capacity(constraint_count);
    for _ in 0..constraint_count {
        constraints.push(decode_constraint(reader)?);
    }

    let check_count = reader.u32()? as usize;
    let mut checks = Vec::with_capacity(check_count);
    for _ in 0..check_count {
        checks.push(decode_check(reader)?);
    }

    Ok(TableDef {
        table_id,
        schema_id,
        relation_id,
        name,
        folded,
        columns,
        indexes,
        constraints,
        checks,
        rowid_alias_column,
        flags,
        normalized_sql,
    })
}

fn encode_column(out: &mut BytesWriter, column: &ColumnDef) -> Result<()> {
    out.u64(column.column_id.0);
    out.u16(column.ordinal);
    write_str(out, &column.name);
    write_str(out, &column.folded);
    write_opt_str(out, column.declared_type.as_deref());
    out.u8(column.affinity as u8);
    out.bool(column.not_null);
    write_opt_value(out, column.default_value.as_ref());
    write_opt_expr(out, column.default_expr.as_deref());
    Ok(())
}

fn decode_column(reader: &mut BytesReader<'_>) -> Result<ColumnDef> {
    Ok(ColumnDef {
        column_id: super::ColumnId(reader.u64()?),
        ordinal: reader.u16()?,
        name: read_box_str(reader)?,
        folded: read_box_str(reader)?,
        declared_type: read_opt_box_str(reader)?,
        affinity: match reader.u8()? {
            0 => super::Affinity::Blob,
            1 => super::Affinity::Text,
            2 => super::Affinity::Numeric,
            3 => super::Affinity::Integer,
            4 => super::Affinity::Real,
            _ => return Err(Error::CatalogCorrupt("invalid affinity")),
        },
        not_null: reader.bool()?,
        default_value: read_opt_value(reader)?,
        default_expr: read_opt_expr(reader)?,
    })
}

fn encode_index(out: &mut BytesWriter, index: &IndexDef) -> Result<()> {
    out.u64(index.index_id.0);
    out.u64(index.table_id.0);
    out.u64(index.relation_id.0);
    write_opt_u64(out, index.meta_page_id.map(|value| value.0));
    write_str(out, &index.name);
    write_str(out, &index.folded);
    out.bool(index.unique);
    out.bool(index.primary);
    out.u8(index.origin as u8);
    out.u64(index.flags);
    write_opt_str(out, index.normalized_sql.as_deref());
    out.u32(index.keys.len() as u32);
    for key in &index.keys {
        encode_index_key(out, key)?;
    }
    Ok(())
}

fn decode_index(reader: &mut BytesReader<'_>, format_version: u64) -> Result<IndexDef> {
    let index_id = super::IndexId(reader.u64()?);
    let table_id = super::TableId(reader.u64()?);
    let relation_id = RelId(reader.u64()?);
    let meta_page_id = if format_version >= 2 {
        read_opt_u64(reader)?.map(PageId)
    } else {
        None
    };
    let name = read_box_str(reader)?;
    let folded = read_box_str(reader)?;
    let unique = reader.bool()?;
    let primary = reader.bool()?;
    let origin = match reader.u8()? {
        0 => IndexOrigin::User,
        1 => IndexOrigin::PrimaryKey,
        2 => IndexOrigin::UniqueConstraint,
        _ => return Err(Error::CatalogCorrupt("invalid index origin")),
    };
    let flags = reader.u64()?;
    let normalized_sql = read_opt_box_str(reader)?;
    let key_count = reader.u32()? as usize;
    let mut keys = Vec::with_capacity(key_count);
    for _ in 0..key_count {
        keys.push(decode_index_key(reader)?);
    }
    Ok(IndexDef {
        index_id,
        table_id,
        relation_id,
        meta_page_id,
        name,
        folded,
        unique,
        primary,
        origin,
        keys,
        flags,
        normalized_sql,
    })
}

fn encode_index_key(out: &mut BytesWriter, key: &IndexKeyDef) -> Result<()> {
    out.u16(key.ordinal);
    match key.source {
        IndexKeySource::Column { attnum } => {
            out.u8(0);
            out.u16(attnum);
        }
    }
    out.u8(key.sort_dir as u8);
    out.u8(key.null_order as u8);
    Ok(())
}

fn decode_index_key(reader: &mut BytesReader<'_>) -> Result<IndexKeyDef> {
    let ordinal = reader.u16()?;
    let source = match reader.u8()? {
        0 => IndexKeySource::Column {
            attnum: reader.u16()?,
        },
        _ => return Err(Error::CatalogCorrupt("invalid index key source")),
    };
    let sort_dir = match reader.u8()? {
        0 => SortDir::Asc,
        1 => SortDir::Desc,
        _ => return Err(Error::CatalogCorrupt("invalid sort direction")),
    };
    let null_order = match reader.u8()? {
        0 => NullOrder::First,
        1 => NullOrder::Last,
        _ => return Err(Error::CatalogCorrupt("invalid null ordering")),
    };
    Ok(IndexKeyDef {
        ordinal,
        source,
        sort_dir,
        null_order,
    })
}

fn encode_constraint(out: &mut BytesWriter, constraint: &ConstraintDef) -> Result<()> {
    out.u64(constraint.constraint_id.0);
    out.u64(constraint.table_id.0);
    write_opt_str(out, constraint.name.as_deref());
    out.u8(constraint.kind as u8);
    write_opt_u64(out, constraint.column_id.map(|v| v.0));
    write_opt_u64(out, constraint.index_id.map(|v| v.0));
    out.u8(constraint.conflict_action as u8);
    write_opt_expr(out, constraint.expr.as_deref());
    Ok(())
}

fn decode_constraint(reader: &mut BytesReader<'_>) -> Result<ConstraintDef> {
    Ok(ConstraintDef {
        constraint_id: super::ConstraintId(reader.u64()?),
        table_id: super::TableId(reader.u64()?),
        name: read_opt_box_str(reader)?,
        kind: match reader.u8()? {
            1 => ConstraintKind::PrimaryKey,
            2 => ConstraintKind::Unique,
            3 => ConstraintKind::NotNull,
            4 => ConstraintKind::Check,
            5 => ConstraintKind::Default,
            _ => return Err(Error::CatalogCorrupt("invalid constraint kind")),
        },
        column_id: read_opt_u64(reader)?.map(super::ColumnId),
        index_id: read_opt_u64(reader)?.map(super::IndexId),
        conflict_action: match reader.u8()? {
            0 => ConflictAction::Abort,
            1 => ConflictAction::Ignore,
            2 => ConflictAction::Replace,
            _ => return Err(Error::CatalogCorrupt("invalid conflict action")),
        },
        expr: read_opt_expr(reader)?,
    })
}

fn encode_check(out: &mut BytesWriter, check: &CheckDef) -> Result<()> {
    out.u64(check.constraint_id.0);
    write_opt_str(out, check.name.as_deref());
    write_opt_expr(out, Some(check.expr.as_ref()));
    Ok(())
}

fn decode_check(reader: &mut BytesReader<'_>) -> Result<CheckDef> {
    Ok(CheckDef {
        constraint_id: super::ConstraintId(reader.u64()?),
        name: read_opt_box_str(reader)?,
        expr: read_opt_expr(reader)?.ok_or(Error::CatalogCorrupt("missing check expression"))?,
    })
}

fn encode_expr_op(out: &mut BytesWriter, op: &ExprOp) -> Result<()> {
    match op {
        ExprOp::Const(v) => {
            out.u8(0);
            write_value(out, v);
        }
        ExprOp::Column(col) => {
            out.u8(1);
            out.u16(*col);
        }
        ExprOp::Not => out.u8(2),
        ExprOp::And => out.u8(3),
        ExprOp::Or => out.u8(4),
        ExprOp::Eq => out.u8(5),
        ExprOp::Ne => out.u8(6),
        ExprOp::Lt => out.u8(7),
        ExprOp::Le => out.u8(8),
        ExprOp::Gt => out.u8(9),
        ExprOp::Ge => out.u8(10),
        // Phase-10 Lane V1: `BlobLen` was introduced for vector-dimension
        // CHECK constraints. Older binaries cannot read databases that use
        // it (the unknown-opcode arm in `decode_expr_op` will surface as
        // catalog corruption), which is the correct forward-compatibility
        // posture.
        ExprOp::BlobLen => out.u8(11),
    }
    Ok(())
}

fn decode_expr_op(reader: &mut BytesReader<'_>) -> Result<ExprOp> {
    Ok(match reader.u8()? {
        0 => ExprOp::Const(
            read_opt_value(reader)?.ok_or(Error::CatalogCorrupt("missing expr const"))?,
        ),
        1 => ExprOp::Column(reader.u16()?),
        2 => ExprOp::Not,
        3 => ExprOp::And,
        4 => ExprOp::Or,
        5 => ExprOp::Eq,
        6 => ExprOp::Ne,
        7 => ExprOp::Lt,
        8 => ExprOp::Le,
        9 => ExprOp::Gt,
        10 => ExprOp::Ge,
        11 => ExprOp::BlobLen,
        _ => return Err(Error::CatalogCorrupt("invalid expr opcode")),
    })
}

fn encode_expr_bytes(expr: &CompiledExpr) -> Result<Vec<u8>> {
    let mut out = BytesWriter::new();
    encode_expr_into(&mut out, expr)?;
    Ok(out.finish())
}

fn encode_expr_into(out: &mut BytesWriter, expr: &CompiledExpr) -> Result<()> {
    out.u32(expr.bytecode.len() as u32);
    for op in &expr.bytecode {
        encode_expr_op(out, op)?;
    }
    out.u32(expr.referenced_cols.len() as u32);
    for col in &expr.referenced_cols {
        out.u16(*col);
    }
    Ok(())
}

fn decode_expr_from_bytes(bytes: &[u8]) -> Result<CompiledExpr> {
    let mut reader = BytesReader::new(bytes);
    let expr = decode_expr(&mut reader)?;
    if reader.remaining() != 0 {
        return Err(Error::CatalogCorrupt("expression has trailing bytes"));
    }
    Ok(expr)
}

fn decode_expr(reader: &mut BytesReader<'_>) -> Result<CompiledExpr> {
    let bytecode_len = reader.u32()? as usize;
    let mut bytecode = Vec::with_capacity(bytecode_len);
    for _ in 0..bytecode_len {
        bytecode.push(decode_expr_op(reader)?);
    }
    let col_count = reader.u32()? as usize;
    let mut referenced_cols = Vec::with_capacity(col_count);
    for _ in 0..col_count {
        referenced_cols.push(reader.u16()?);
    }
    Ok(CompiledExpr {
        bytecode: bytecode.into_boxed_slice(),
        referenced_cols,
    })
}

// Catalog (schema) wire-format helpers. Primitives live in `super::codec`;
// these helpers add the schema-specific framing: length-prefixed strings,
// the `bool`-tag + value `opt_*` pattern, and the nested-bytes encoding
// for compiled expressions.

fn write_str(out: &mut BytesWriter, value: &str) {
    out.u32(value.len() as u32);
    out.bytes(value.as_bytes());
}

fn write_opt_str(out: &mut BytesWriter, value: Option<&str>) {
    match value {
        Some(value) => {
            out.bool(true);
            write_str(out, value);
        }
        None => out.bool(false),
    }
}

fn write_opt_u16(out: &mut BytesWriter, value: Option<u16>) {
    match value {
        Some(value) => {
            out.bool(true);
            out.u16(value);
        }
        None => out.bool(false),
    }
}

fn write_opt_u64(out: &mut BytesWriter, value: Option<u64>) {
    match value {
        Some(value) => {
            out.bool(true);
            out.u64(value);
        }
        None => out.bool(false),
    }
}

fn write_opt_value(out: &mut BytesWriter, value: Option<&OwnedValue>) {
    match value {
        Some(value) => {
            out.bool(true);
            write_value(out, value);
        }
        None => out.bool(false),
    }
}

fn write_value(out: &mut BytesWriter, value: &OwnedValue) {
    match value {
        OwnedValue::Null => out.u8(0),
        OwnedValue::Integer(v) => {
            out.u8(1);
            out.bytes(&v.to_le_bytes());
        }
        OwnedValue::Real(v) => {
            out.u8(2);
            out.u64(v.to_bits());
        }
        OwnedValue::Text(v) => {
            out.u8(3);
            write_str(out, v);
        }
        OwnedValue::Blob(v) => {
            out.u8(4);
            out.u32(v.len() as u32);
            out.bytes(v);
        }
    }
}

fn write_opt_expr(out: &mut BytesWriter, value: Option<&CompiledExpr>) {
    match value {
        Some(value) => {
            out.bool(true);
            let expr = encode_expr_bytes(value).expect("expr encoding should not fail");
            out.u32(expr.len() as u32);
            out.bytes(&expr);
        }
        None => out.bool(false),
    }
}

fn read_string(reader: &mut BytesReader<'_>) -> Result<String> {
    let len = reader.u32()? as usize;
    let bytes = reader.take(len)?;
    let text = std::str::from_utf8(bytes).map_err(|_| Error::CatalogCorrupt("invalid utf8"))?;
    Ok(text.to_owned())
}

fn read_box_str(reader: &mut BytesReader<'_>) -> Result<Box<str>> {
    Ok(read_string(reader)?.into_boxed_str())
}

fn read_opt_box_str(reader: &mut BytesReader<'_>) -> Result<Option<Box<str>>> {
    if reader.bool()? {
        Ok(Some(read_box_str(reader)?))
    } else {
        Ok(None)
    }
}

fn read_opt_u16(reader: &mut BytesReader<'_>) -> Result<Option<u16>> {
    if reader.bool()? {
        Ok(Some(reader.u16()?))
    } else {
        Ok(None)
    }
}

fn read_opt_u64(reader: &mut BytesReader<'_>) -> Result<Option<u64>> {
    if reader.bool()? {
        Ok(Some(reader.u64()?))
    } else {
        Ok(None)
    }
}

fn read_opt_value(reader: &mut BytesReader<'_>) -> Result<Option<OwnedValue>> {
    if !reader.bool()? {
        return Ok(None);
    }
    Ok(Some(read_value(reader)?))
}

fn read_value(reader: &mut BytesReader<'_>) -> Result<OwnedValue> {
    Ok(match reader.u8()? {
        0 => OwnedValue::Null,
        1 => OwnedValue::Integer(i64::from_le_bytes(reader.take_array()?)),
        2 => OwnedValue::Real(f64::from_bits(reader.u64()?)),
        3 => OwnedValue::Text(Arc::from(read_string(reader)?)),
        4 => {
            let len = reader.u32()? as usize;
            OwnedValue::Blob(Arc::from(reader.take(len)?))
        }
        _ => return Err(Error::CatalogCorrupt("invalid value tag")),
    })
}

fn read_opt_expr(reader: &mut BytesReader<'_>) -> Result<Option<Arc<CompiledExpr>>> {
    if !reader.bool()? {
        return Ok(None);
    }
    let len = reader.u32()? as usize;
    let bytes = reader.take(len)?;
    Ok(Some(Arc::new(decode_expr_from_bytes(bytes)?)))
}
