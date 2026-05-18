use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crc32fast::Hasher;

use crate::{Error, Result};

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
        let tmp = self.path.with_extension("tmp");
        {
            // Lane E failpoint: armed before the temp catalog file is created.
            // A crash here yields no `.tmp`, so recovery must observe the old
            // catalog generation untouched.
            crate::fail_point!("catalog::save::temp_write");
            let mut file = fs::File::create(&tmp)?;
            file.write_all(&bytes)?;
            // Lane E failpoint: armed after the temp write but before fsync.
            // Crashing here lets the OS keep the temp file in page cache only;
            // recovery must still see the prior atomic snapshot.
            crate::fail_point!("catalog::save::fsync");
            file.sync_all()?;
        }
        // Lane E failpoint: armed before the atomic rename. The temp file is
        // fully durable on disk; a crash here guarantees the rename never
        // happened, so the prior schema snapshot remains the canonical one.
        crate::fail_point!("catalog::save::rename");
        fs::rename(tmp, &self.path)?;
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
    let mut out = Writer::new();
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
    let mut reader = Reader::new(bytes);
    let format_version = reader.u64()?;
    if format_version > 2 {
        return Err(Error::UnsupportedVersion(format_version as u16));
    }
    let meta = CatalogMeta {
        format_version,
        schema_epoch: SchemaEpoch(reader.u64()?),
        next_object_id: super::ObjectId(reader.u64()?),
        next_relation_id: RelId(reader.u64()?),
        database_uuid: reader.array_16()?,
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
    let mut out = Vec::with_capacity(20 + payload.len());
    out.extend_from_slice(&MAGIC.to_le_bytes());
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&0_u16.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    let crc = checksum(&payload);
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(&payload);
    Ok(out)
}

fn decode_snapshot_file(bytes: &[u8]) -> Result<Arc<SchemaSnapshot>> {
    if bytes.len() < 20 {
        return Err(Error::CatalogCorrupt("catalog snapshot file too small"));
    }
    let magic = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
    if magic != MAGIC {
        return Err(Error::CatalogCorrupt("catalog snapshot magic mismatch"));
    }
    let version = u16::from_le_bytes(bytes[4..6].try_into().unwrap());
    if version != VERSION {
        return Err(Error::UnsupportedVersion(version));
    }
    let payload_len = u64::from_le_bytes(bytes[8..16].try_into().unwrap()) as usize;
    let crc = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
    let expected = 20_usize
        .checked_add(payload_len)
        .ok_or(Error::CatalogCorrupt("catalog snapshot length overflow"))?;
    if bytes.len() != expected {
        return Err(Error::CatalogCorrupt("catalog snapshot length mismatch"));
    }
    let payload = &bytes[20..];
    if checksum(payload) != crc {
        return Err(Error::InvalidChecksum);
    }
    Ok(Arc::new(decode_snapshot(payload)?))
}

fn encode_namespace(out: &mut Writer, namespace: &NamespaceDef) -> Result<()> {
    out.u64(namespace.schema_id.0);
    out.str(&namespace.name);
    out.str(&namespace.folded);
    Ok(())
}

fn decode_namespace(reader: &mut Reader<'_>) -> Result<NamespaceDef> {
    Ok(NamespaceDef {
        schema_id: SchemaId(reader.u64()?),
        name: reader.box_str()?,
        folded: reader.box_str()?,
    })
}

fn encode_table(out: &mut Writer, table: &TableDef) -> Result<()> {
    out.u64(table.table_id.0);
    out.u64(table.schema_id.0);
    out.u64(table.relation_id.0);
    out.str(&table.name);
    out.str(&table.folded);
    out.u64(table.flags);
    out.opt_u16(table.rowid_alias_column);
    out.opt_str(table.normalized_sql.as_deref());

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

fn decode_table(reader: &mut Reader<'_>, format_version: u64) -> Result<TableDef> {
    let table_id = super::TableId(reader.u64()?);
    let schema_id = SchemaId(reader.u64()?);
    let relation_id = RelId(reader.u64()?);
    let name = reader.box_str()?;
    let folded = reader.box_str()?;
    let flags = reader.u64()?;
    let rowid_alias_column = reader.opt_u16()?;
    let normalized_sql = reader.opt_box_str()?;

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

fn encode_column(out: &mut Writer, column: &ColumnDef) -> Result<()> {
    out.u64(column.column_id.0);
    out.u16(column.ordinal);
    out.str(&column.name);
    out.str(&column.folded);
    out.opt_str(column.declared_type.as_deref());
    out.u8(column.affinity as u8);
    out.bool(column.not_null);
    out.opt_value(column.default_value.as_ref());
    out.opt_expr(column.default_expr.as_deref());
    Ok(())
}

fn decode_column(reader: &mut Reader<'_>) -> Result<ColumnDef> {
    Ok(ColumnDef {
        column_id: super::ColumnId(reader.u64()?),
        ordinal: reader.u16()?,
        name: reader.box_str()?,
        folded: reader.box_str()?,
        declared_type: reader.opt_box_str()?,
        affinity: match reader.u8()? {
            0 => super::Affinity::Blob,
            1 => super::Affinity::Text,
            2 => super::Affinity::Numeric,
            3 => super::Affinity::Integer,
            4 => super::Affinity::Real,
            _ => return Err(Error::CatalogCorrupt("invalid affinity")),
        },
        not_null: reader.bool()?,
        default_value: reader.opt_value()?,
        default_expr: reader.opt_expr()?,
    })
}

fn encode_index(out: &mut Writer, index: &IndexDef) -> Result<()> {
    out.u64(index.index_id.0);
    out.u64(index.table_id.0);
    out.u64(index.relation_id.0);
    out.opt_u64(index.meta_page_id.map(|value| value.0));
    out.str(&index.name);
    out.str(&index.folded);
    out.bool(index.unique);
    out.bool(index.primary);
    out.u8(index.origin as u8);
    out.u64(index.flags);
    out.opt_str(index.normalized_sql.as_deref());
    out.u32(index.keys.len() as u32);
    for key in &index.keys {
        encode_index_key(out, key)?;
    }
    Ok(())
}

fn decode_index(reader: &mut Reader<'_>, format_version: u64) -> Result<IndexDef> {
    let index_id = super::IndexId(reader.u64()?);
    let table_id = super::TableId(reader.u64()?);
    let relation_id = RelId(reader.u64()?);
    let meta_page_id = if format_version >= 2 {
        reader.opt_u64()?.map(PageId)
    } else {
        None
    };
    let name = reader.box_str()?;
    let folded = reader.box_str()?;
    let unique = reader.bool()?;
    let primary = reader.bool()?;
    let origin = match reader.u8()? {
        0 => IndexOrigin::User,
        1 => IndexOrigin::PrimaryKey,
        2 => IndexOrigin::UniqueConstraint,
        _ => return Err(Error::CatalogCorrupt("invalid index origin")),
    };
    let flags = reader.u64()?;
    let normalized_sql = reader.opt_box_str()?;
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

fn encode_index_key(out: &mut Writer, key: &IndexKeyDef) -> Result<()> {
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

fn decode_index_key(reader: &mut Reader<'_>) -> Result<IndexKeyDef> {
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

fn encode_constraint(out: &mut Writer, constraint: &ConstraintDef) -> Result<()> {
    out.u64(constraint.constraint_id.0);
    out.u64(constraint.table_id.0);
    out.opt_str(constraint.name.as_deref());
    out.u8(constraint.kind as u8);
    out.opt_u64(constraint.column_id.map(|v| v.0));
    out.opt_u64(constraint.index_id.map(|v| v.0));
    out.u8(constraint.conflict_action as u8);
    out.opt_expr(constraint.expr.as_deref());
    Ok(())
}

fn decode_constraint(reader: &mut Reader<'_>) -> Result<ConstraintDef> {
    Ok(ConstraintDef {
        constraint_id: super::ConstraintId(reader.u64()?),
        table_id: super::TableId(reader.u64()?),
        name: reader.opt_box_str()?,
        kind: match reader.u8()? {
            1 => ConstraintKind::PrimaryKey,
            2 => ConstraintKind::Unique,
            3 => ConstraintKind::NotNull,
            4 => ConstraintKind::Check,
            5 => ConstraintKind::Default,
            _ => return Err(Error::CatalogCorrupt("invalid constraint kind")),
        },
        column_id: reader.opt_u64()?.map(super::ColumnId),
        index_id: reader.opt_u64()?.map(super::IndexId),
        conflict_action: match reader.u8()? {
            0 => ConflictAction::Abort,
            1 => ConflictAction::Ignore,
            2 => ConflictAction::Replace,
            _ => return Err(Error::CatalogCorrupt("invalid conflict action")),
        },
        expr: reader.opt_expr()?,
    })
}

fn encode_check(out: &mut Writer, check: &CheckDef) -> Result<()> {
    out.u64(check.constraint_id.0);
    out.opt_str(check.name.as_deref());
    out.opt_expr(Some(check.expr.as_ref()));
    Ok(())
}

fn decode_check(reader: &mut Reader<'_>) -> Result<CheckDef> {
    Ok(CheckDef {
        constraint_id: super::ConstraintId(reader.u64()?),
        name: reader.opt_box_str()?,
        expr: reader
            .opt_expr()?
            .ok_or(Error::CatalogCorrupt("missing check expression"))?,
    })
}

fn encode_expr_op(out: &mut Writer, op: &ExprOp) -> Result<()> {
    match op {
        ExprOp::Const(v) => {
            out.u8(0);
            out.value(Some(v));
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
        // catalog corruption), which is the correct forward-compat posture.
        ExprOp::BlobLen => out.u8(11),
    }
    Ok(())
}

fn decode_expr_op(reader: &mut Reader<'_>) -> Result<ExprOp> {
    Ok(match reader.u8()? {
        0 => ExprOp::Const(
            reader
                .opt_value()?
                .ok_or(Error::CatalogCorrupt("missing expr const"))?,
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
    let mut out = Writer::new();
    encode_expr_into(&mut out, expr)?;
    Ok(out.finish())
}

fn encode_expr_into(out: &mut Writer, expr: &CompiledExpr) -> Result<()> {
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
    let mut reader = Reader::new(bytes);
    let expr = decode_expr(&mut reader)?;
    if reader.remaining() != 0 {
        return Err(Error::CatalogCorrupt("expression has trailing bytes"));
    }
    Ok(expr)
}

fn decode_expr(reader: &mut Reader<'_>) -> Result<CompiledExpr> {
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

fn checksum(bytes: &[u8]) -> u32 {
    let mut hasher = Hasher::new();
    hasher.update(bytes);
    hasher.finalize()
}

struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }

    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn bool(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn bytes(&mut self, value: &[u8]) {
        self.bytes.extend_from_slice(value);
    }

    fn str(&mut self, value: &str) {
        self.u32(value.len() as u32);
        self.bytes(value.as_bytes());
    }

    fn opt_str(&mut self, value: Option<&str>) {
        match value {
            Some(value) => {
                self.bool(true);
                self.str(value);
            }
            None => self.bool(false),
        }
    }

    fn opt_u16(&mut self, value: Option<u16>) {
        match value {
            Some(value) => {
                self.bool(true);
                self.u16(value);
            }
            None => self.bool(false),
        }
    }

    fn opt_u64(&mut self, value: Option<u64>) {
        match value {
            Some(value) => {
                self.bool(true);
                self.u64(value);
            }
            None => self.bool(false),
        }
    }

    fn opt_value(&mut self, value: Option<&OwnedValue>) {
        match value {
            Some(value) => {
                self.bool(true);
                self.value(Some(value));
            }
            None => self.bool(false),
        }
    }

    fn value(&mut self, value: Option<&OwnedValue>) {
        match value.expect("value present") {
            OwnedValue::Null => self.u8(0),
            OwnedValue::Integer(v) => {
                self.u8(1);
                self.bytes(&v.to_le_bytes());
            }
            OwnedValue::Real(v) => {
                self.u8(2);
                self.u64(v.to_bits());
            }
            OwnedValue::Text(v) => {
                self.u8(3);
                self.str(v);
            }
            OwnedValue::Blob(v) => {
                self.u8(4);
                self.u32(v.len() as u32);
                self.bytes(v);
            }
        }
    }

    fn opt_expr(&mut self, value: Option<&CompiledExpr>) {
        match value {
            Some(value) => {
                self.bool(true);
                let expr = encode_expr_bytes(value).expect("expr encoding should not fail");
                self.u32(expr.len() as u32);
                self.bytes(&expr);
            }
            None => self.bool(false),
        }
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.pos)
    }

    fn u8(&mut self) -> Result<u8> {
        let byte = *self
            .bytes
            .get(self.pos)
            .ok_or(Error::CatalogCorrupt("catalog snapshot truncated"))?;
        self.pos += 1;
        Ok(byte)
    }

    fn bool(&mut self) -> Result<bool> {
        Ok(self.u8()? != 0)
    }

    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take_array()?))
    }

    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take_array()?))
    }

    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take_array()?))
    }

    fn array_16(&mut self) -> Result<[u8; 16]> {
        self.take_array()
    }

    fn take_array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let end = self
            .pos
            .checked_add(N)
            .ok_or(Error::CatalogCorrupt("catalog snapshot length overflow"))?;
        let bytes = self
            .bytes
            .get(self.pos..end)
            .ok_or(Error::CatalogCorrupt("catalog snapshot truncated"))?;
        self.pos = end;
        let mut out = [0u8; N];
        out.copy_from_slice(bytes);
        Ok(out)
    }

    fn string(&mut self) -> Result<String> {
        let len = self.u32()? as usize;
        let end = self
            .pos
            .checked_add(len)
            .ok_or(Error::CatalogCorrupt("catalog snapshot length overflow"))?;
        let bytes = self
            .bytes
            .get(self.pos..end)
            .ok_or(Error::CatalogCorrupt("catalog snapshot truncated"))?;
        self.pos = end;
        let text = std::str::from_utf8(bytes).map_err(|_| Error::CatalogCorrupt("invalid utf8"))?;
        Ok(text.to_owned())
    }

    fn box_str(&mut self) -> Result<Box<str>> {
        Ok(self.string()?.into_boxed_str())
    }

    fn opt_box_str(&mut self) -> Result<Option<Box<str>>> {
        if self.bool()? {
            Ok(Some(self.box_str()?))
        } else {
            Ok(None)
        }
    }

    fn opt_u16(&mut self) -> Result<Option<u16>> {
        if self.bool()? {
            Ok(Some(self.u16()?))
        } else {
            Ok(None)
        }
    }

    fn opt_u64(&mut self) -> Result<Option<u64>> {
        if self.bool()? {
            Ok(Some(self.u64()?))
        } else {
            Ok(None)
        }
    }

    fn opt_value(&mut self) -> Result<Option<OwnedValue>> {
        if !self.bool()? {
            return Ok(None);
        }
        Ok(Some(self.value()?))
    }

    fn value(&mut self) -> Result<OwnedValue> {
        Ok(match self.u8()? {
            0 => OwnedValue::Null,
            1 => OwnedValue::Integer(i64::from_le_bytes(self.take_array()?)),
            2 => OwnedValue::Real(f64::from_bits(self.u64()?)),
            3 => OwnedValue::Text(Arc::from(self.string()?)),
            4 => {
                let len = self.u32()? as usize;
                let end = self
                    .pos
                    .checked_add(len)
                    .ok_or(Error::CatalogCorrupt("catalog snapshot length overflow"))?;
                let bytes = self
                    .bytes
                    .get(self.pos..end)
                    .ok_or(Error::CatalogCorrupt("catalog snapshot truncated"))?;
                self.pos = end;
                OwnedValue::Blob(Arc::from(bytes))
            }
            _ => return Err(Error::CatalogCorrupt("invalid value tag")),
        })
    }

    fn opt_expr(&mut self) -> Result<Option<Arc<CompiledExpr>>> {
        if !self.bool()? {
            return Ok(None);
        }
        let len = self.u32()? as usize;
        let end = self
            .pos
            .checked_add(len)
            .ok_or(Error::CatalogCorrupt("catalog snapshot length overflow"))?;
        let bytes = self
            .bytes
            .get(self.pos..end)
            .ok_or(Error::CatalogCorrupt("catalog snapshot truncated"))?;
        self.pos = end;
        Ok(Some(Arc::new(decode_expr_from_bytes(bytes)?)))
    }
}
