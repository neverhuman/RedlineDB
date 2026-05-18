use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use redlinedb_kernel::engine::RecoveryTarget;
use redlinedb_kernel::format::{BackupId, Csn, DbId, Lsn, TimelineId};
use redlinedb_sql::RecoveryTarget as SqlRecoveryTarget;

use crate::Database;
use crate::error::{Error, ErrorCode, Result};

pub const PHASE8_DIR: &str = "phase8";
pub const IDENTITY_FILE: &str = "identity.json";
pub const BACKUP_DIR: &str = "backups";
pub const SLOT_DIR: &str = "replication_slots";
pub const ARCHIVE_DIR: &str = "archive";
pub const RETENTION_FILE: &str = "retention.json";
const MANIFEST_FILE: &str = "backup-manifest.json";
const COMPLETE_FILE: &str = "complete.marker";
const RESTORE_COMPLETE_FILE: &str = "restore.complete";
const FORMAT_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum ArchiveMode {
    #[default]
    Off,
    Local,
    RequiredLocal,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum SlotKind {
    #[default]
    Physical,
    Logical,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum WalLevel {
    #[default]
    Physical,
    Logical,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhysicalBackupOptions {
    pub include_wal: bool,
    pub archive_mode: ArchiveMode,
}

impl Default for PhysicalBackupOptions {
    fn default() -> Self {
        Self {
            include_wal: true,
            archive_mode: ArchiveMode::Off,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RestoreOptions {
    pub target: SqlRecoveryTarget,
    pub preserve_timeline: bool,
}

impl Default for RestoreOptions {
    fn default() -> Self {
        Self {
            target: SqlRecoveryTarget::Latest,
            preserve_timeline: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhysicalBackupStats {
    pub files_copied: u64,
    pub bytes_copied: u64,
    pub elapsed_ms: u128,
    pub backup_id: u128,
    pub stop_lsn: u64,
    pub stop_csn: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreStats {
    pub files_copied: u64,
    pub bytes_copied: u64,
    pub elapsed_ms: u128,
    pub target_lsn: u64,
    pub target_csn: u64,
    pub new_timeline: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveStats {
    pub archive_mode: ArchiveMode,
    pub pending_segments: u64,
    pub archived_segments: u64,
    pub failed_segments: u64,
    pub last_archived_lsn: u64,
    pub archived_bytes: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionHorizon {
    pub wal_recycle_lsn: u64,
    pub vacuum_csn: u64,
    pub catalog_csn: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationSlotStats {
    pub name: String,
    pub kind: SlotKind,
    pub slot_id: u64,
    pub database_id: u128,
    pub timeline: u64,
    pub restart_lsn: u64,
    pub restart_csn: u64,
    pub confirmed_flush_lsn: u64,
    pub confirmed_flush_csn: u64,
    pub active: bool,
}

pub type ReplicationSlot = ReplicationSlotStats;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct IdentityFile {
    format_version: u32,
    db_id: u128,
    timeline: u64,
    parent_timeline: Option<u64>,
    fork_lsn: Option<u64>,
    wal_level: WalLevel,
    archive_mode: ArchiveMode,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PhysicalBackupManifest {
    format_version: u32,
    backup_id: u128,
    db_id: u128,
    timeline: u64,
    parent_timeline: Option<u64>,
    fork_lsn: Option<u64>,
    page_size: usize,
    wal_segment_bytes: u64,
    required_wal_start: u64,
    stop_lsn: u64,
    stop_csn: u64,
    included_wal: bool,
    archive_mode: ArchiveMode,
    created_unix_nanos: u128,
    files: Vec<String>,
    tree_hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SlotFile {
    format_version: u32,
    stats: ReplicationSlotStats,
    created_unix_nanos: u128,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RetentionFile {
    format_version: u32,
    horizon: RetentionHorizon,
    updated_unix_nanos: u128,
}

pub fn backup_physical_to_path(
    db: &Database,
    dst: impl AsRef<Path>,
    options: PhysicalBackupOptions,
) -> Result<PhysicalBackupStats> {
    let start = Instant::now();
    db.checkpoint()?;
    let src = db.path();
    let identity = load_identity_or_init(src)?;
    let stats = db.inner.db.stats().map_err(Error::from)?;
    let tx_stats = db.inner.db.tx_status_stats();
    let backup_id = next_backup_id();
    let dst = dst.as_ref();

    if dst.exists() {
        fs::remove_dir_all(dst)?;
    }
    fs::create_dir_all(dst)?;

    let files = collect_files(src, &|path| should_copy_path(path, dst))?;
    let mut bytes_copied = 0_u64;
    for rel in &files {
        bytes_copied += copy_file(src, dst, rel)?;
    }

    let tree_hash = hash_tree(dst, &files)?;
    let manifest = PhysicalBackupManifest {
        format_version: FORMAT_VERSION,
        backup_id: backup_id.0,
        db_id: identity.db_id,
        timeline: identity.timeline,
        parent_timeline: identity.parent_timeline,
        fork_lsn: identity.fork_lsn,
        page_size: db.inner.db.engine_config().page_size,
        wal_segment_bytes: db.inner.db.engine_config().wal.segment_bytes,
        required_wal_start: stats.wal_durable_lsn.0,
        stop_lsn: stats.wal_durable_lsn.0,
        stop_csn: tx_stats.published_csn.0,
        included_wal: options.include_wal,
        archive_mode: options.archive_mode,
        created_unix_nanos: unix_nanos(),
        files: files
            .iter()
            .map(|path| rel_to_string(path))
            .collect::<Vec<_>>(),
        tree_hash,
    };
    let manifest_path = phase8_path(dst).join(MANIFEST_FILE);
    write_json_atomic(&manifest_path, &manifest)?;
    write_text_atomic(&phase8_path(dst).join(COMPLETE_FILE), "ok\n")?;

    Ok(PhysicalBackupStats {
        files_copied: files.len() as u64,
        bytes_copied,
        elapsed_ms: start.elapsed().as_millis(),
        backup_id: backup_id.0,
        stop_lsn: manifest.stop_lsn,
        stop_csn: manifest.stop_csn,
    })
}

pub fn restore_from_backup(
    src: impl AsRef<Path>,
    dst: impl AsRef<Path>,
    options: RestoreOptions,
) -> Result<RestoreStats> {
    let start = Instant::now();
    let src = src.as_ref();
    let dst = dst.as_ref();
    let manifest: PhysicalBackupManifest = read_json(phase8_path(src).join(MANIFEST_FILE))?;
    if manifest.format_version != FORMAT_VERSION {
        return Err(Error::new(
            ErrorCode::Unsupported,
            "unsupported backup format",
        ));
    }
    if hash_tree(src, &manifest_file_list(src, &manifest)?)? != manifest.tree_hash {
        return Err(Error::new(ErrorCode::Corrupt, "backup tree hash mismatch"));
    }
    if dst.exists() {
        fs::remove_dir_all(dst)?;
    }
    fs::create_dir_all(dst)?;

    let mut bytes_copied = 0_u64;
    for rel in &manifest.files {
        bytes_copied += copy_file(src, dst, &PathBuf::from(rel))?;
    }
    let mut restored_identity = load_identity_or_init(dst)?;
    restored_identity.db_id = next_db_id();
    if !options.preserve_timeline {
        restored_identity.parent_timeline = Some(manifest.timeline);
        restored_identity.timeline = next_timeline_id(manifest.timeline).0;
        restored_identity.fork_lsn = match options.target {
            SqlRecoveryTarget::Latest => Some(manifest.stop_lsn),
            SqlRecoveryTarget::Lsn(lsn) => Some(lsn.0),
            SqlRecoveryTarget::Csn(csn) => Some(csn.0),
        };
    }
    write_json_atomic(&identity_path(dst), &restored_identity)?;

    let recovery_target = match options.target {
        SqlRecoveryTarget::Latest => RecoveryTarget::Latest,
        SqlRecoveryTarget::Lsn(lsn) => RecoveryTarget::Lsn(Lsn(lsn.0)),
        SqlRecoveryTarget::Csn(csn) => RecoveryTarget::Csn(Csn(csn.0)),
    };
    let mut sql_opts = redlinedb_sql::DbOptions::default();
    sql_opts.engine.page_size = manifest.page_size;
    sql_opts.engine.wal.segment_bytes = manifest.wal_segment_bytes;
    sql_opts.engine.data_file_name = "data.redline".to_owned();
    let _reopened =
        redlinedb_sql::Database::open_with_recovery_target(dst, sql_opts, recovery_target)?;

    write_text_atomic(&phase8_path(dst).join(RESTORE_COMPLETE_FILE), "ok\n")?;
    Ok(RestoreStats {
        files_copied: manifest.files.len() as u64,
        bytes_copied,
        elapsed_ms: start.elapsed().as_millis(),
        target_lsn: match options.target {
            SqlRecoveryTarget::Latest => manifest.stop_lsn,
            SqlRecoveryTarget::Lsn(lsn) => lsn.0,
            SqlRecoveryTarget::Csn(csn) => csn.0,
        },
        target_csn: match options.target {
            SqlRecoveryTarget::Latest => manifest.stop_csn,
            SqlRecoveryTarget::Lsn(_) => manifest.stop_csn,
            SqlRecoveryTarget::Csn(csn) => csn.0,
        },
        new_timeline: restored_identity.timeline,
    })
}

pub fn create_physical_slot(db: &Database, name: &str, active: bool) -> Result<ReplicationSlot> {
    let identity = load_identity_or_init(db.path())?;
    let stats = db.inner.db.stats().map_err(Error::from)?;
    let tx_stats = db.inner.db.tx_status_stats();
    let slot = ReplicationSlotStats {
        name: name.to_owned(),
        kind: SlotKind::Physical,
        slot_id: next_slot_id(),
        database_id: identity.db_id,
        timeline: identity.timeline,
        restart_lsn: stats.wal_durable_lsn.0,
        restart_csn: tx_stats.published_csn.0,
        confirmed_flush_lsn: stats.wal_durable_lsn.0,
        confirmed_flush_csn: tx_stats.published_csn.0,
        active,
    };
    persist_slot(db.path(), &slot)?;
    persist_retention(db.path(), compute_retention_horizon(db, &slot)?)?;
    Ok(slot)
}

pub fn create_logical_slot(db: &Database, name: &str, active: bool) -> Result<ReplicationSlot> {
    let identity = load_identity_or_init(db.path())?;
    let stats = db.inner.db.stats().map_err(Error::from)?;
    let tx_stats = db.inner.db.tx_status_stats();
    let slot = ReplicationSlotStats {
        name: name.to_owned(),
        kind: SlotKind::Logical,
        slot_id: next_slot_id(),
        database_id: identity.db_id,
        timeline: identity.timeline,
        restart_lsn: stats.wal_durable_lsn.0,
        restart_csn: tx_stats.published_csn.0,
        confirmed_flush_lsn: stats.wal_durable_lsn.0,
        confirmed_flush_csn: tx_stats.published_csn.0,
        active,
    };
    persist_slot(db.path(), &slot)?;
    persist_retention(db.path(), compute_retention_horizon(db, &slot)?)?;
    Ok(slot)
}

pub fn drop_replication_slot(db: &Database, name: &str) -> Result<()> {
    let path = slot_path(db.path(), name);
    if path.exists() {
        fs::remove_file(path)?;
    }
    persist_retention(db.path(), retention_from_state(db)?)?;
    Ok(())
}

pub fn replication_slots(db: &Database) -> Result<Vec<ReplicationSlotStats>> {
    let mut slots = Vec::new();
    let dir = slots_dir(db.path());
    if !dir.exists() {
        return Ok(slots);
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            let slot: SlotFile = read_json(entry.path())?;
            slots.push(slot.stats);
        }
    }
    slots.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(slots)
}

pub fn archive_stats(db: &Database) -> Result<ArchiveStats> {
    let _identity = load_identity_or_init(db.path())?;
    let mut pending_segments = 0_u64;
    let mut archived_segments = 0_u64;
    let mut archived_bytes = 0_u64;
    let archive_dir = archive_dir(db.path());
    if archive_dir.exists() {
        for entry in fs::read_dir(&archive_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                archived_segments += 1;
                archived_bytes += entry.metadata()?.len();
            }
        }
    }

    let wal_dir = db.path().join("wal");
    if wal_dir.exists() {
        for entry in fs::read_dir(&wal_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext == "wal")
            {
                pending_segments += 1;
            }
        }
    }

    let horizon = retention_from_state(db)?;
    Ok(ArchiveStats {
        archive_mode: load_identity_or_init(db.path())?.archive_mode,
        pending_segments,
        archived_segments,
        failed_segments: 0,
        last_archived_lsn: horizon.wal_recycle_lsn,
        archived_bytes,
    })
}

pub fn retention_horizon(db: &Database) -> Result<RetentionHorizon> {
    retention_from_state(db)
}

pub fn update_retention(db: &Database) -> Result<RetentionHorizon> {
    let horizon = retention_from_state(db)?;
    persist_retention(db.path(), horizon)?;
    Ok(horizon)
}

pub fn current_database_id(db: &Database) -> Result<DbId> {
    Ok(DbId(load_identity_or_init(db.path())?.db_id as u64))
}

fn retention_from_state(db: &Database) -> Result<RetentionHorizon> {
    let stats = db.inner.db.stats().map_err(Error::from)?;
    let tx_stats = db.inner.db.tx_status_stats();
    let slots = replication_slots(db)?;
    let mut wal_recycle_lsn = stats.wal_durable_lsn.0;
    for slot in slots {
        wal_recycle_lsn = wal_recycle_lsn.min(slot.restart_lsn);
    }
    Ok(RetentionHorizon {
        wal_recycle_lsn,
        vacuum_csn: stats.vacuum_horizon_csn.0,
        catalog_csn: tx_stats.published_csn.0,
    })
}

fn compute_retention_horizon(
    db: &Database,
    slot: &ReplicationSlotStats,
) -> Result<RetentionHorizon> {
    let mut horizon = retention_from_state(db)?;
    horizon.wal_recycle_lsn = horizon.wal_recycle_lsn.min(slot.restart_lsn);
    horizon.vacuum_csn = horizon.vacuum_csn.min(slot.restart_csn);
    Ok(horizon)
}

fn collect_files(root: &Path, allow: &dyn Fn(&Path) -> bool) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_files_recursive(root, root, allow, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_files_recursive(
    root: &Path,
    current: &Path,
    allow: &dyn Fn(&Path) -> bool,
    out: &mut Vec<PathBuf>,
) -> Result<()> {
    let backup_prefix = Path::new(PHASE8_DIR).join(BACKUP_DIR);
    if !current.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if !allow(&path) {
            continue;
        }
        if entry.file_type()?.is_dir() {
            if let Ok(rel) = path.strip_prefix(root)
                && rel.starts_with(&backup_prefix)
            {
                continue;
            }
            collect_files_recursive(root, &path, allow, out)?;
        } else if entry.file_type()?.is_file() {
            let rel = path
                .strip_prefix(root)
                .map_err(|_| Error::new(ErrorCode::Corrupt, "path outside backup root"))?
                .to_path_buf();
            if rel.starts_with(&backup_prefix) {
                continue;
            }
            out.push(rel);
        }
    }
    Ok(())
}

fn should_copy_path(path: &Path, backup_root: &Path) -> bool {
    if path.file_name().and_then(|name| name.to_str()) == Some("owner.lock") {
        return false;
    }
    if path.starts_with(backup_root) {
        return false;
    }
    true
}

fn copy_file(src_root: &Path, dst_root: &Path, rel: &Path) -> Result<u64> {
    let src = src_root.join(rel);
    let dst = dst_root.join(rel);
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = fs::copy(&src, &dst)?;
    Ok(bytes)
}

fn hash_tree(root: &Path, files: &[PathBuf]) -> Result<String> {
    let mut hasher = Sha256::new();
    for rel in files {
        hasher.update(rel_to_string(rel).as_bytes());
        let mut file = File::open(root.join(rel))?;
        let mut buf = [0_u8; 8192];
        loop {
            let read = file.read(&mut buf)?;
            if read == 0 {
                break;
            }
            hasher.update(&buf[..read]);
        }
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn manifest_file_list(_root: &Path, manifest: &PhysicalBackupManifest) -> Result<Vec<PathBuf>> {
    Ok(manifest.files.iter().map(PathBuf::from).collect())
}

fn rel_to_string(rel: &Path) -> String {
    rel.to_string_lossy().replace('\\', "/")
}

fn next_db_id() -> u128 {
    unix_nanos()
}

fn next_timeline_id(parent: u64) -> TimelineId {
    TimelineId(parent.saturating_add(1).max(1))
}

fn next_slot_id() -> u64 {
    unix_nanos() as u64
}

fn next_backup_id() -> BackupId {
    BackupId(unix_nanos())
}

fn unix_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

fn phase8_path(root: &Path) -> PathBuf {
    root.join(PHASE8_DIR)
}

fn identity_path(root: &Path) -> PathBuf {
    phase8_path(root).join(IDENTITY_FILE)
}

fn archive_dir(root: &Path) -> PathBuf {
    phase8_path(root).join(ARCHIVE_DIR)
}

fn slots_dir(root: &Path) -> PathBuf {
    phase8_path(root).join(SLOT_DIR)
}

fn slot_path(root: &Path, name: &str) -> PathBuf {
    slots_dir(root).join(format!("{name}.json"))
}

fn retention_path(root: &Path) -> PathBuf {
    phase8_path(root).join(RETENTION_FILE)
}

fn load_identity_or_init(root: &Path) -> Result<IdentityFile> {
    fs::create_dir_all(phase8_path(root))?;
    let path = identity_path(root);
    if path.exists() {
        return read_json(path);
    }
    let identity = IdentityFile {
        format_version: FORMAT_VERSION,
        db_id: next_db_id(),
        timeline: 1,
        parent_timeline: None,
        fork_lsn: None,
        wal_level: WalLevel::Physical,
        archive_mode: ArchiveMode::Off,
    };
    write_json_atomic(&path, &identity)?;
    Ok(identity)
}

fn persist_slot(root: &Path, slot: &ReplicationSlotStats) -> Result<()> {
    fs::create_dir_all(slots_dir(root))?;
    let path = slot_path(root, &slot.name);
    let file = SlotFile {
        format_version: FORMAT_VERSION,
        stats: slot.clone(),
        created_unix_nanos: unix_nanos(),
    };
    write_json_atomic(&path, &file)
}

fn persist_retention(root: &Path, horizon: RetentionHorizon) -> Result<()> {
    fs::create_dir_all(phase8_path(root))?;
    let file = RetentionFile {
        format_version: FORMAT_VERSION,
        horizon,
        updated_unix_nanos: unix_nanos(),
    };
    write_json_atomic(&retention_path(root), &file)
}

fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp_path = path.with_extension("tmp");
    let bytes = serde_json::to_vec_pretty(value)?;
    {
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&tmp_path)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
    }
    fs::rename(&tmp_path, path)?;
    if let Some(parent) = path.parent() {
        sync_dir(parent)?;
    }
    Ok(())
}

fn write_text_atomic(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp_path = path.with_extension("tmp");
    {
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&tmp_path)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
    }
    fs::rename(&tmp_path, path)?;
    if let Some(parent) = path.parent() {
        sync_dir(parent)?;
    }
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>, P: AsRef<Path>>(path: P) -> Result<T> {
    let bytes = fs::read(path.as_ref())?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn sync_dir(path: &Path) -> Result<()> {
    let file = File::open(path)?;
    file.sync_all()?;
    Ok(())
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Error::with_source(ErrorCode::Corrupt, value.to_string(), value)
    }
}
