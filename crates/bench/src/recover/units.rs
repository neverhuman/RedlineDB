use anyhow::Result;

use crate::config::RecoveryScenarioKind;
use crate::engine::{self, CellValue};

pub fn commit_recovery_unit(
    engine: &dyn engine::BenchEngine,
    conn: &mut dyn engine::BenchConn,
    scenario: RecoveryScenarioKind,
    key: usize,
    total_rows: usize,
    checkpoint_every_rows: usize,
) -> Result<()> {
    conn.begin_immediate()?;
    match scenario {
        RecoveryScenarioKind::Wal => {
            recovery_wal_unit(conn, key)?;
        }
        RecoveryScenarioKind::Catalog => {
            recovery_catalog_unit(conn, key)?;
        }
        RecoveryScenarioKind::Checkpoint => {
            recovery_checkpoint_unit(conn, key, total_rows)?;
        }
    }
    conn.execute(
        "INSERT OR REPLACE INTO crash_progress(id, scenario, note) VALUES (?1, ?2, ?3)",
        &[
            CellValue::Integer(key as i64),
            CellValue::Text(scenario.as_str().to_owned()),
            CellValue::Text(format!("ack-{key}")),
        ],
    )?;
    conn.commit()?;
    if matches!(scenario, RecoveryScenarioKind::Checkpoint)
        && checkpoint_every_rows > 0
        && key > 0
        && key.is_multiple_of(checkpoint_every_rows)
    {
        engine.checkpoint()?;
    }
    Ok(())
}

pub fn recovery_wal_unit(conn: &mut dyn engine::BenchConn, key: usize) -> Result<()> {
    let params = [
        CellValue::Integer(key as i64),
        CellValue::Integer((key % 32) as i64),
        CellValue::Blob(format!("value-{key:08}").into_bytes()),
        CellValue::Integer(1),
    ];
    let _ = conn.execute(
        "INSERT OR REPLACE INTO kv(k, tenant, v, version) VALUES (?1, ?2, ?3, ?4)",
        &params,
    )?;
    Ok(())
}

pub fn recovery_catalog_unit(conn: &mut dyn engine::BenchConn, key: usize) -> Result<()> {
    let slot = key % 8;
    let table = format!("scratch_{slot}");
    conn.execute(
        &format!("CREATE TABLE IF NOT EXISTS {table}(id INTEGER PRIMARY KEY, note TEXT)"),
        &[],
    )?;
    conn.execute(
        &format!("CREATE INDEX IF NOT EXISTS {table}_note_idx ON {table}(note)"),
        &[],
    )?;
    conn.execute(
        &format!("INSERT INTO {table}(id, note) VALUES (?1, ?2)"),
        &[
            CellValue::Integer(key as i64),
            CellValue::Text(format!("catalog-{key}")),
        ],
    )?;
    if key.is_multiple_of(2) {
        let _ = conn.execute(&format!("DROP INDEX IF EXISTS {table}_note_idx"), &[])?;
        let _ = conn.execute(&format!("DROP TABLE IF EXISTS {table}"), &[])?;
    }
    Ok(())
}

pub fn recovery_checkpoint_unit(
    conn: &mut dyn engine::BenchConn,
    key: usize,
    total_rows: usize,
) -> Result<()> {
    let version = ((key % total_rows.max(1)) + 1) as i64;
    let params = [
        CellValue::Integer(key as i64),
        CellValue::Integer((key % 32) as i64),
        CellValue::Blob(format!("checkpoint-{key:08}").into_bytes()),
        CellValue::Integer(version),
    ];
    let _ = conn.execute(
        "INSERT OR REPLACE INTO kv(k, tenant, v, version) VALUES (?1, ?2, ?3, ?4)",
        &params,
    )?;
    Ok(())
}

pub fn ensure_crash_schema(conn: &mut dyn engine::BenchConn) -> Result<()> {
    let _ = conn.execute(
        "CREATE TABLE IF NOT EXISTS crash_progress(id INTEGER PRIMARY KEY, scenario TEXT, note TEXT)",
        &[],
    )?;
    Ok(())
}
