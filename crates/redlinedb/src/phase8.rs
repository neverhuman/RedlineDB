use std::ffi::{CStr, CString};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use redlinedb_kernel::engine::RecoveryTarget;
use redlinedb_kernel::format::{BackupId, Csn, DbId, Lsn, TimelineId};
use redlinedb_kernel::wal::{WalRetentionHorizons, archive_watermark as kernel_archive_watermark};
use redlinedb_sql::RecoveryTarget as SqlRecoveryTarget;

use crate::Database;
use crate::error::{Error, ErrorCode, Result};

pub const PHASE8_DIR: &str = "phase8";
pub const IDENTITY_FILE: &str = "identity.json";
pub const BACKUP_DIR: &str = "backups";
pub const SLOT_DIR: &str = "replication_slots";
pub const ARCHIVE_DIR: &str = "archive";
pub const RETENTION_FILE: &str = "retention.json";
pub const PHYSICAL_BACKUP_MANIFEST_FILE: &str = "backup-manifest.json";
const COMPLETE_FILE: &str = "complete.marker";
const RESTORE_COMPLETE_FILE: &str = "restore.complete";
const ARCHIVE_STATE_DIR: &str = "state";
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhysicalBackupManifest {
    pub format_version: u32,
    pub backup_id: u128,
    pub db_id: u128,
    pub timeline: u64,
    pub parent_timeline: Option<u64>,
    pub fork_lsn: Option<u64>,
    pub page_size: usize,
    pub wal_segment_bytes: u64,
    pub required_wal_start: u64,
    pub stop_lsn: u64,
    pub stop_csn: u64,
    pub included_wal: bool,
    pub archive_mode: ArchiveMode,
    pub created_unix_nanos: u128,
    pub files: Vec<String>,
    pub tree_hash: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifiedBackupFile {
    pub relative_path: String,
    pub byte_len: u64,
    pub sha256: String,
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

struct NativeRoot {
    dir: File,
    dev: u64,
    ino: u64,
}

struct OpenedBackupFile {
    verified: VerifiedBackupFile,
    file: File,
    dev: u64,
    ino: u64,
}

struct OpenedBackup {
    root: NativeRoot,
    manifest: PhysicalBackupManifest,
    files: Vec<OpenedBackupFile>,
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

    prepare_empty_directory(dst, "physical backup destination is not empty")?;

    let files = collect_files(src, &|path| should_copy_path(path, dst))?;
    let mut bytes_copied = 0_u64;
    for rel in &files {
        bytes_copied += copy_file_exclusive(src, dst, rel)?;
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
    let manifest_path = phase8_path(dst).join(PHYSICAL_BACKUP_MANIFEST_FILE);
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
    let mut backup = open_verified_backup(src)?;
    let manifest = backup.manifest.clone();
    prepare_empty_directory(dst, "restore destination is not empty")?;
    let destination = NativeRoot::open(dst)?;

    let mut bytes_copied = 0_u64;
    for file in &mut backup.files {
        backup.root.revalidate_file(file)?;
        bytes_copied += copy_opened_file(file, &destination)?;
    }
    verify_copied_files(&destination, &backup.files)?;
    backup.root.revalidate_path(src)?;
    destination.revalidate_path(dst)?;

    // All subsequent restore operations are rooted at the retained directory
    // descriptor, not at a pathname which could be exchanged after
    // verification.
    let retained_dst = destination.proc_path();
    let mut restored_identity = load_identity_or_init(&retained_dst)?;
    restored_identity.db_id = next_db_id();
    if !options.preserve_timeline {
        restored_identity.parent_timeline = Some(manifest.timeline);
        restored_identity.timeline = next_timeline_id(manifest.timeline).0;
        restored_identity.fork_lsn = match options.target {
            SqlRecoveryTarget::Latest => Some(manifest.stop_lsn),
            SqlRecoveryTarget::Lsn(lsn) => Some(lsn.0),
            // CSNs and LSNs are different domains. A CSN-targeted restore
            // replays from this backup's durable WAL boundary, while the CSN
            // itself is carried by the recovery target below.
            SqlRecoveryTarget::Csn(_) => Some(manifest.stop_lsn),
        };
    }
    write_json_atomic(&identity_path(&retained_dst), &restored_identity)?;

    let recovery_target = match options.target {
        SqlRecoveryTarget::Latest => RecoveryTarget::Latest,
        SqlRecoveryTarget::Lsn(lsn) => RecoveryTarget::Lsn(Lsn(lsn.0)),
        SqlRecoveryTarget::Csn(csn) => RecoveryTarget::Csn(Csn(csn.0)),
    };
    let mut sql_opts = redlinedb_sql::DbOptions::default();
    sql_opts.engine.page_size = manifest.page_size;
    sql_opts.engine.wal.segment_bytes = manifest.wal_segment_bytes;
    sql_opts.engine.data_file_name = "data.redline".to_owned();
    let _reopened = redlinedb_sql::Database::open_with_recovery_target(
        &retained_dst,
        sql_opts,
        recovery_target,
    )?;

    destination.revalidate_path(dst)?;
    verify_native_tree(&retained_dst)?;
    write_text_atomic(
        &phase8_path(&retained_dst).join(RESTORE_COMPLETE_FILE),
        "ok\n",
    )?;
    destination.revalidate_path(dst)?;
    Ok(RestoreStats {
        files_copied: manifest.files.len() as u64,
        bytes_copied,
        elapsed_ms: start.elapsed().as_millis(),
        target_lsn: match options.target {
            SqlRecoveryTarget::Latest => manifest.stop_lsn,
            SqlRecoveryTarget::Lsn(lsn) => lsn.0,
            SqlRecoveryTarget::Csn(_) => manifest.stop_lsn,
        },
        target_csn: match options.target {
            SqlRecoveryTarget::Latest => manifest.stop_csn,
            SqlRecoveryTarget::Lsn(_) => manifest.stop_csn,
            SqlRecoveryTarget::Csn(csn) => csn.0,
        },
        new_timeline: restored_identity.timeline,
    })
}

pub fn physical_backup_manifest(src: impl AsRef<Path>) -> Result<PhysicalBackupManifest> {
    let root = NativeRoot::open(src.as_ref())?;
    physical_backup_manifest_from_root(&root)
}

fn physical_backup_manifest_from_root(root: &NativeRoot) -> Result<PhysicalBackupManifest> {
    let mut file =
        root.open_regular_file(&Path::new(PHASE8_DIR).join(PHYSICAL_BACKUP_MANIFEST_FILE))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let manifest: PhysicalBackupManifest = serde_json::from_slice(&bytes)?;
    if manifest.format_version != FORMAT_VERSION {
        return Err(Error::new(
            ErrorCode::Unsupported,
            "unsupported backup format",
        ));
    }
    if manifest.timeline == 0
        || manifest.page_size == 0
        || manifest.wal_segment_bytes == 0
        || manifest.required_wal_start > manifest.stop_lsn
        || manifest.files.is_empty()
        || manifest.tree_hash.len() != 64
        || !manifest
            .tree_hash
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(Error::new(
            ErrorCode::Corrupt,
            "invalid physical backup manifest",
        ));
    }
    Ok(manifest)
}

pub fn verify_physical_backup(src: impl AsRef<Path>) -> Result<Vec<VerifiedBackupFile>> {
    Ok(open_verified_backup(src.as_ref())?
        .files
        .into_iter()
        .map(|file| file.verified)
        .collect())
}

fn open_verified_backup(src: &Path) -> Result<OpenedBackup> {
    let root = NativeRoot::open(src)?;
    let manifest = physical_backup_manifest_from_root(&root)?;
    verify_complete_marker_from_root(&root)?;
    let paths = manifest_file_list(src, &manifest)?;
    let mut files = Vec::with_capacity(paths.len());
    let mut tree_hasher = Sha256::new();
    for path in paths {
        let relative_path = rel_to_string(&path);
        tree_hasher.update(relative_path.as_bytes());
        let mut file = root.open_regular_file(&path)?;
        let metadata = file.metadata()?;
        let mut file_hasher = Sha256::new();
        let mut byte_len = 0_u64;
        let mut buf = [0_u8; 8192];
        loop {
            let read = file.read(&mut buf)?;
            if read == 0 {
                break;
            }
            byte_len = byte_len
                .checked_add(read as u64)
                .ok_or_else(|| Error::new(ErrorCode::TooBig, "backup file is too large"))?;
            file_hasher.update(&buf[..read]);
            tree_hasher.update(&buf[..read]);
        }
        files.push(OpenedBackupFile {
            verified: VerifiedBackupFile {
                relative_path,
                byte_len,
                sha256: format!("{:x}", file_hasher.finalize()),
            },
            file,
            dev: metadata.dev(),
            ino: metadata.ino(),
        });
    }
    if format!("{:x}", tree_hasher.finalize()) != manifest.tree_hash {
        return Err(Error::new(ErrorCode::Corrupt, "backup tree hash mismatch"));
    }
    for file in &files {
        root.revalidate_file(file)?;
    }
    root.revalidate_path(src)?;
    Ok(OpenedBackup {
        root,
        manifest,
        files,
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

pub fn set_archive_mode(db: &Database, mode: ArchiveMode) -> Result<()> {
    let mut identity = load_identity_or_init(db.path())?;
    identity.archive_mode = mode;
    write_json_atomic(&identity_path(db.path()), &identity)
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

pub(crate) fn wal_retention_horizons(db: &Database) -> Result<WalRetentionHorizons> {
    let stats = db.inner.db.stats().map_err(Error::from)?;
    let identity = load_identity_or_init(db.path())?;
    let slots = replication_slots(db)?;
    let replication_slot_lsn = slots
        .iter()
        .map(|slot| slot.restart_lsn)
        .min()
        .unwrap_or(stats.wal_durable_lsn.0);
    let required_archive_lsn = if identity.archive_mode == ArchiveMode::RequiredLocal {
        kernel_archive_watermark(
            archive_dir(db.path()).join(ARCHIVE_STATE_DIR),
            TimelineId(identity.timeline),
        )
        .map_err(redlinedb_sql::Error::Kernel)
        .map_err(Error::from)?
        .archived_lsn
        .0
    } else {
        stats.wal_durable_lsn.0
    };
    Ok(WalRetentionHorizons {
        checkpoint_lsn: stats.wal_durable_lsn,
        replication_slot_lsn: Lsn(replication_slot_lsn),
        required_archive_lsn: Lsn(required_archive_lsn),
    })
}

fn retention_from_state(db: &Database) -> Result<RetentionHorizon> {
    let stats = db.inner.db.stats().map_err(Error::from)?;
    let tx_stats = db.inner.db.tx_status_stats();
    let wal_recycle_lsn = wal_retention_horizons(db)?.recycle_lsn().0;
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

fn copy_file_exclusive(src_root: &Path, dst_root: &Path, rel: &Path) -> Result<u64> {
    verify_regular_path(src_root, rel)?;
    let src = src_root.join(rel);
    let dst = dst_root.join(rel);
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut input = File::open(src)?;
    let mut output = OpenOptions::new().create_new(true).write(true).open(dst)?;
    let bytes = std::io::copy(&mut input, &mut output)?;
    output.sync_all()?;
    Ok(bytes)
}

impl NativeRoot {
    fn open(path: &Path) -> Result<Self> {
        let dir = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)?;
        let metadata = dir.metadata()?;
        if !metadata.file_type().is_dir() {
            return Err(Error::new(
                ErrorCode::Corrupt,
                "native root is not a directory",
            ));
        }
        Ok(Self {
            dir,
            dev: metadata.dev(),
            ino: metadata.ino(),
        })
    }

    fn proc_path(&self) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{}", self.dir.as_raw_fd()))
    }

    fn revalidate_path(&self, path: &Path) -> Result<()> {
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.file_type().is_dir()
            || metadata.dev() != self.dev
            || metadata.ino() != self.ino
        {
            return Err(Error::new(
                ErrorCode::Corrupt,
                "native root identity changed",
            ));
        }
        Ok(())
    }

    fn open_regular_file(&self, rel: &Path) -> Result<File> {
        let components = normal_components(rel)?;
        let mut parent = self.dir.try_clone()?;
        for (index, component) in components.iter().enumerate() {
            let last = index + 1 == components.len();
            let flags = if last {
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC
            } else {
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC
            };
            let opened = openat_component(&parent, component, flags, 0)?;
            let metadata = opened.metadata()?;
            if (last && (!metadata.file_type().is_file() || metadata.nlink() != 1))
                || (!last && !metadata.file_type().is_dir())
            {
                return Err(Error::new(
                    ErrorCode::Corrupt,
                    "backup path is not a native regular file",
                ));
            }
            parent = opened;
        }
        Ok(parent)
    }

    fn create_regular_file(&self, rel: &Path) -> Result<File> {
        let components = normal_components(rel)?;
        let (file_name, parents) = components
            .split_last()
            .ok_or_else(|| Error::new(ErrorCode::Corrupt, "empty backup path"))?;
        let mut parent = self.dir.try_clone()?;
        for component in parents {
            let flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
            let opened = match openat_component(&parent, component, flags, 0) {
                Ok(opened) => opened,
                Err(error) if error.code() == ErrorCode::NotFound => {
                    mkdirat_component(&parent, component)?;
                    parent.sync_all()?;
                    openat_component(&parent, component, flags, 0)?
                }
                Err(error) => return Err(error),
            };
            if !opened.metadata()?.file_type().is_dir() {
                return Err(Error::new(
                    ErrorCode::Corrupt,
                    "restore ancestor is not a native directory",
                ));
            }
            parent = opened;
        }
        let output = openat_component(
            &parent,
            file_name,
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )?;
        parent.sync_all()?;
        Ok(output)
    }

    fn revalidate_file(&self, file: &OpenedBackupFile) -> Result<()> {
        let reopened = self.open_regular_file(Path::new(&file.verified.relative_path))?;
        let metadata = reopened.metadata()?;
        if metadata.dev() != file.dev || metadata.ino() != file.ino {
            return Err(Error::new(
                ErrorCode::Corrupt,
                "backup file identity changed after verification",
            ));
        }
        Ok(())
    }
}

fn normal_components(path: &Path) -> Result<Vec<&std::ffi::OsStr>> {
    let mut components = Vec::new();
    for component in path.components() {
        let Component::Normal(component) = component else {
            return Err(Error::new(
                ErrorCode::Corrupt,
                "invalid backup manifest path",
            ));
        };
        components.push(component);
    }
    if components.is_empty() {
        return Err(Error::new(ErrorCode::Corrupt, "empty backup manifest path"));
    }
    Ok(components)
}

fn component_name(component: &std::ffi::OsStr) -> Result<CString> {
    CString::new(component.as_bytes())
        .map_err(|_| Error::new(ErrorCode::Corrupt, "backup path contains NUL"))
}

fn openat_component(
    parent: &File,
    component: &std::ffi::OsStr,
    flags: libc::c_int,
    mode: libc::mode_t,
) -> Result<File> {
    let component = component_name(component)?;
    openat_name(parent, &component, flags, mode).map_err(Error::from)
}

fn openat_name(
    parent: &File,
    name: &CStr,
    flags: libc::c_int,
    mode: libc::mode_t,
) -> std::io::Result<File> {
    // SAFETY: `parent` is retained for the call, `name` is NUL-terminated,
    // and a successful call returns a new owned descriptor.
    let fd = unsafe { libc::openat(parent.as_raw_fd(), name.as_ptr(), flags, mode) };
    if fd < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        // SAFETY: the successful `openat` result is uniquely owned here.
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}

fn mkdirat_component(parent: &File, component: &std::ffi::OsStr) -> Result<()> {
    let component = component_name(component)?;
    // SAFETY: `parent` and `component` remain valid for the complete call.
    if unsafe { libc::mkdirat(parent.as_raw_fd(), component.as_ptr(), 0o700) } != 0 {
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::AlreadyExists {
            return Err(error.into());
        }
    }
    Ok(())
}

fn copy_opened_file(file: &mut OpenedBackupFile, destination: &NativeRoot) -> Result<u64> {
    file.file.seek(SeekFrom::Start(0))?;
    let mut output = destination.create_regular_file(Path::new(&file.verified.relative_path))?;
    let mut hasher = Sha256::new();
    let mut byte_len = 0_u64;
    let mut buffer = [0_u8; 8192];
    loop {
        let read = file.file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        output.write_all(&buffer[..read])?;
        hasher.update(&buffer[..read]);
        byte_len = byte_len
            .checked_add(read as u64)
            .ok_or_else(|| Error::new(ErrorCode::TooBig, "backup file is too large"))?;
    }
    if byte_len != file.verified.byte_len
        || format!("{:x}", hasher.finalize()) != file.verified.sha256
    {
        return Err(Error::new(
            ErrorCode::Corrupt,
            "backup file changed while restoring",
        ));
    }
    output.sync_all()?;
    Ok(byte_len)
}

fn verify_copied_files(destination: &NativeRoot, files: &[OpenedBackupFile]) -> Result<()> {
    for expected in files {
        let mut file =
            destination.open_regular_file(Path::new(&expected.verified.relative_path))?;
        let mut hasher = Sha256::new();
        let mut byte_len = 0_u64;
        let mut buffer = [0_u8; 8192];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            byte_len = byte_len
                .checked_add(read as u64)
                .ok_or_else(|| Error::new(ErrorCode::TooBig, "restore file is too large"))?;
        }
        if byte_len != expected.verified.byte_len
            || format!("{:x}", hasher.finalize()) != expected.verified.sha256
        {
            return Err(Error::new(
                ErrorCode::Corrupt,
                "restored file does not match verified backup bytes",
            ));
        }
    }
    destination.dir.sync_all()?;
    Ok(())
}

fn verify_native_tree(root: &Path) -> Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_dir() {
            verify_native_tree(&entry.path())?;
        } else if !metadata.file_type().is_file() || metadata.nlink() != 1 {
            return Err(Error::new(
                ErrorCode::Corrupt,
                "restore tree contains a non-native entry",
            ));
        }
    }
    Ok(())
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
    let mut paths = Vec::with_capacity(manifest.files.len());
    let mut previous = None;
    for raw in &manifest.files {
        let path = validate_backup_relative_path(raw)?;
        if previous.as_ref().is_some_and(|value| value >= &path) {
            return Err(Error::new(
                ErrorCode::Corrupt,
                "backup manifest paths are not canonical",
            ));
        }
        previous = Some(path.clone());
        paths.push(path);
    }
    Ok(paths)
}

fn rel_to_string(rel: &Path) -> String {
    rel.to_string_lossy().replace('\\', "/")
}

fn validate_backup_relative_path(raw: &str) -> Result<PathBuf> {
    if raw.is_empty() || raw.contains('\\') || raw.as_bytes().contains(&0) {
        return Err(Error::new(
            ErrorCode::Corrupt,
            "invalid backup manifest path",
        ));
    }
    let path = PathBuf::from(raw);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        || rel_to_string(&path) != raw
    {
        return Err(Error::new(
            ErrorCode::Corrupt,
            "invalid backup manifest path",
        ));
    }
    Ok(path)
}

fn verify_complete_marker_from_root(root: &NativeRoot) -> Result<()> {
    let mut marker = root.open_regular_file(&Path::new(PHASE8_DIR).join(COMPLETE_FILE))?;
    let mut text = String::new();
    marker.read_to_string(&mut text)?;
    if text != "ok\n" {
        return Err(Error::new(
            ErrorCode::Corrupt,
            "physical backup is incomplete",
        ));
    }
    Ok(())
}

fn verify_regular_path(root: &Path, rel: &Path) -> Result<()> {
    let root_metadata = fs::symlink_metadata(root)?;
    if !root_metadata.file_type().is_dir() {
        return Err(Error::new(
            ErrorCode::Corrupt,
            "backup root is not a native directory",
        ));
    }
    let mut current = root.to_path_buf();
    let component_count = rel.components().count();
    for (index, component) in rel.components().enumerate() {
        let Component::Normal(name) = component else {
            return Err(Error::new(
                ErrorCode::Corrupt,
                "invalid backup manifest path",
            ));
        };
        current.push(name);
        let metadata = fs::symlink_metadata(&current)?;
        let is_last = index + 1 == component_count;
        if (is_last && !metadata.file_type().is_file())
            || (!is_last && !metadata.file_type().is_dir())
        {
            return Err(Error::new(
                ErrorCode::Corrupt,
                "backup manifest entry is not a regular file",
            ));
        }
        #[cfg(unix)]
        if is_last {
            use std::os::unix::fs::MetadataExt as _;
            if metadata.nlink() != 1 {
                return Err(Error::new(
                    ErrorCode::Corrupt,
                    "backup manifest entry has multiple links",
                ));
            }
        }
    }
    Ok(())
}

fn prepare_empty_directory(path: &Path, message: &'static str) -> Result<()> {
    if path.exists() {
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.file_type().is_dir() || fs::read_dir(path)?.next().is_some() {
            return Err(Error::new(ErrorCode::Busy, message));
        }
        return Ok(());
    }
    fs::create_dir_all(path)?;
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn opened_file(root: &NativeRoot, rel: &Path) -> OpenedBackupFile {
        let mut file = root.open_regular_file(rel).expect("open retained file");
        let metadata = file.metadata().expect("metadata");
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).expect("read");
        file.seek(SeekFrom::Start(0)).expect("rewind");
        OpenedBackupFile {
            verified: VerifiedBackupFile {
                relative_path: rel_to_string(rel),
                byte_len: bytes.len() as u64,
                sha256: format!("{:x}", Sha256::digest(&bytes)),
            },
            file,
            dev: metadata.dev(),
            ino: metadata.ino(),
        }
    }

    #[test]
    fn retained_source_file_never_reopens_replacement_bytes() {
        let temp = tempfile::tempdir().expect("tempdir");
        let src = temp.path().join("src");
        let dst = temp.path().join("dst");
        fs::create_dir(&src).expect("src");
        fs::create_dir(&dst).expect("dst");
        fs::write(src.join("data"), b"verified").expect("verified");
        let source = NativeRoot::open(&src).expect("source root");
        let mut opened = opened_file(&source, Path::new("data"));

        fs::rename(src.join("data"), src.join("data.held")).expect("hold verified inode");
        fs::write(src.join("data"), b"replacement").expect("replacement");
        assert_eq!(
            source
                .revalidate_file(&opened)
                .expect_err("path swap must be rejected")
                .code(),
            ErrorCode::Corrupt
        );

        let destination = NativeRoot::open(&dst).expect("destination root");
        copy_opened_file(&mut opened, &destination).expect("copy retained descriptor");
        assert_eq!(fs::read(dst.join("data")).expect("restored"), b"verified");
    }

    #[test]
    fn source_ancestor_swap_is_rejected_before_restore_copy() {
        let temp = tempfile::tempdir().expect("tempdir");
        let src = temp.path().join("src");
        fs::create_dir_all(src.join("wal")).expect("source tree");
        fs::write(src.join("wal/segment"), b"verified").expect("verified");
        let source = NativeRoot::open(&src).expect("source root");
        let opened = opened_file(&source, Path::new("wal/segment"));

        fs::rename(src.join("wal"), src.join("wal.held")).expect("hold ancestor");
        fs::create_dir(src.join("wal")).expect("replacement ancestor");
        fs::write(src.join("wal/segment"), b"replacement").expect("replacement");

        assert_eq!(
            source
                .revalidate_file(&opened)
                .expect_err("ancestor swap must be rejected")
                .code(),
            ErrorCode::Corrupt
        );
    }

    #[test]
    fn destination_root_swap_is_rejected_before_publication() {
        let temp = tempfile::tempdir().expect("tempdir");
        let dst = temp.path().join("dst");
        fs::create_dir(&dst).expect("destination");
        let destination = NativeRoot::open(&dst).expect("destination root");
        fs::rename(&dst, temp.path().join("dst.held")).expect("hold destination");
        fs::create_dir(&dst).expect("replacement destination");

        assert_eq!(
            destination
                .revalidate_path(&dst)
                .expect_err("destination swap must be rejected")
                .code(),
            ErrorCode::Corrupt
        );
    }
}
