#![allow(dead_code)]

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::error::Result;
use crate::value::SqlValue;

#[derive(Debug, Clone)]
pub struct RowLayout {
    pub columns: Arc<[String]>,
}

#[derive(Debug, Clone)]
pub struct RowBatch {
    pub layout: Arc<RowLayout>,
    pub columns: Vec<Vec<SqlValue>>,
    pub len: usize,
}

impl RowBatch {
    pub fn new(layout: Arc<RowLayout>) -> Self {
        Self {
            layout,
            columns: Vec::new(),
            len: 0,
        }
    }

    pub fn clear(&mut self) {
        self.columns.clear();
        self.len = 0;
    }

    pub fn push_row(&mut self, row: Vec<SqlValue>) {
        if self.columns.is_empty() {
            self.columns = row.into_iter().map(|value| vec![value]).collect();
        } else {
            for (idx, value) in row.into_iter().enumerate() {
                if let Some(col) = self.columns.get_mut(idx) {
                    col.push(value);
                } else {
                    self.columns.push(vec![value]);
                }
            }
        }
        self.len += 1;
    }

    pub fn row(&self, index: usize) -> Option<Vec<SqlValue>> {
        if index >= self.len {
            return None;
        }
        Some(
            self.columns
                .iter()
                .map(|column| column[index].clone())
                .collect(),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecState {
    Yield,
    Done,
}

static QUERY_SPILL_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub struct QueryMemoryBroker {
    pub work_mem_bytes: usize,
    pub max_spill_bytes: usize,
    pub used_bytes: usize,
    pub spilled_bytes: usize,
    spill_root: PathBuf,
    spill_path: Option<PathBuf>,
}

impl QueryMemoryBroker {
    pub fn new(work_mem_bytes: usize, max_spill_bytes: usize, temp_dir: Option<PathBuf>) -> Self {
        let spill_root = match temp_dir {
            Some(dir) => dir,
            None => std::env::temp_dir(),
        };
        let _ = fs::create_dir_all(&spill_root);
        Self {
            work_mem_bytes,
            max_spill_bytes,
            used_bytes: 0,
            spilled_bytes: 0,
            spill_root,
            spill_path: None,
        }
    }

    pub fn request(&mut self, bytes: usize) -> Result<()> {
        self.used_bytes = self.used_bytes.saturating_add(bytes);
        if self.used_bytes > self.work_mem_bytes {
            let spill = self.used_bytes - self.work_mem_bytes;
            self.spilled_bytes = self.spilled_bytes.saturating_add(spill);
            if self.spilled_bytes > self.max_spill_bytes {
                return Err(crate::error::Error::ConstraintViolation(
                    "query memory spill limit exceeded".to_owned(),
                ));
            }
            self.ensure_spill_file()?;
            self.persist_spill_bytes()?;
        }
        Ok(())
    }

    pub fn release(&mut self, bytes: usize) {
        self.used_bytes = self.used_bytes.saturating_sub(bytes);
    }

    pub fn spill_root(&self) -> &Path {
        &self.spill_root
    }

    fn ensure_spill_file(&mut self) -> Result<()> {
        if self.spill_path.is_some() {
            return Ok(());
        }
        let stamp = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(d) => d,
            Err(_) => Duration::default(),
        }
        .as_nanos();
        let seq = QUERY_SPILL_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            crate::exec::vec::spill::spill_path(&self.spill_root, &format!("query-{stamp}-{seq}"));
        self.spill_path = Some(path);
        Ok(())
    }

    fn persist_spill_bytes(&self) -> Result<()> {
        let Some(path) = &self.spill_path else {
            return Ok(());
        };
        let file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)
            .map_err(|err| crate::error::Error::ConstraintViolation(err.to_string()))?;
        file.set_len(self.spilled_bytes as u64)
            .map_err(|err| crate::error::Error::ConstraintViolation(err.to_string()))?;
        Ok(())
    }
}

impl Drop for QueryMemoryBroker {
    fn drop(&mut self) {
        if let Some(path) = self.spill_path.take() {
            let _ = fs::remove_file(path);
        }
    }
}

#[derive(Debug)]
pub struct ExecContext {
    pub memory: QueryMemoryBroker,
}

impl ExecContext {
    pub fn new(work_mem_bytes: usize, max_spill_bytes: usize, temp_dir: Option<PathBuf>) -> Self {
        Self {
            memory: QueryMemoryBroker::new(work_mem_bytes, max_spill_bytes, temp_dir),
        }
    }
}

pub trait ExecNode {
    fn open(&mut self, _ctx: &mut ExecContext) -> Result<()> {
        Ok(())
    }
    fn next_batch(&mut self, _ctx: &mut ExecContext, _out: &mut RowBatch) -> Result<ExecState>;
    fn close(&mut self, _ctx: &mut ExecContext) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct MaterializeNode {
    rows: Vec<Vec<SqlValue>>,
    cursor: usize,
}

impl MaterializeNode {
    pub fn new(rows: Vec<Vec<SqlValue>>) -> Self {
        Self { rows, cursor: 0 }
    }
}

impl ExecNode for MaterializeNode {
    fn next_batch(&mut self, _ctx: &mut ExecContext, out: &mut RowBatch) -> Result<ExecState> {
        out.clear();
        while self.cursor < self.rows.len() && out.len < 1024 {
            out.push_row(self.rows[self.cursor].clone());
            self.cursor += 1;
        }
        Ok(if self.cursor >= self.rows.len() {
            ExecState::Done
        } else {
            ExecState::Yield
        })
    }
}
