use std::collections::BTreeSet;
use std::ffi::{CStr, CString};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};
use std::sync::MutexGuard;
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
const MAX_SLOT_FILE_BYTES: u64 = 64 * 1024;
const MAX_BACKUP_FILES: usize = 1_000_000;
const MAX_BACKUP_FILE_BYTES: u64 = 1 << 40;
const MAX_BACKUP_TOTAL_BYTES: u64 = 16 << 40;

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
    pub file_entries: Vec<VerifiedBackupFile>,
    pub total_bytes: u64,
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

struct OpenedDestinationFile {
    verified: VerifiedBackupFile,
    file: File,
    dev: u64,
    ino: u64,
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

    let source_root = NativeRoot::open(src)?;
    let backup_root = NativeRoot::open(dst)?;
    let files = collect_files(src, &|path| should_copy_path(path, dst))?;
    let mut bytes_copied = 0_u64;
    for rel in &files {
        bytes_copied = bytes_copied
            .checked_add(copy_file_exclusive(&source_root, &backup_root, rel)?)
            .ok_or_else(|| Error::new(ErrorCode::TooBig, "backup byte total overflow"))?;
        if bytes_copied > MAX_BACKUP_TOTAL_BYTES {
            return Err(Error::new(
                ErrorCode::TooBig,
                "backup exceeds the closed aggregate limit",
            ));
        }
    }

    source_root.revalidate_path(src)?;
    backup_root.revalidate_path(dst)?;
    verify_closed_inventory(&backup_root, &files)?;
    let (file_entries, total_bytes, tree_hash) = verify_files_for_manifest(&backup_root, &files)?;
    backup_root.revalidate_path(dst)?;
    if total_bytes != bytes_copied {
        return Err(Error::new(
            ErrorCode::Corrupt,
            "physical backup byte count changed before manifest publication",
        ));
    }
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
        file_entries,
        total_bytes,
        tree_hash,
    };
    let manifest_path = phase8_path(dst).join(PHYSICAL_BACKUP_MANIFEST_FILE);
    write_json_atomic(&manifest_path, &manifest)?;
    write_text_atomic(&phase8_path(dst).join(COMPLETE_FILE), "ok\n")?;
    let mut final_inventory = files.clone();
    final_inventory.push(Path::new(PHASE8_DIR).join(PHYSICAL_BACKUP_MANIFEST_FILE));
    final_inventory.push(Path::new(PHASE8_DIR).join(COMPLETE_FILE));
    backup_root.revalidate_path(dst)?;
    verify_closed_inventory(&backup_root, &final_inventory)?;

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
    let mut destination_files = Vec::with_capacity(backup.files.len());
    for file in &mut backup.files {
        backup.root.revalidate_file(file)?;
        let copied = copy_opened_file(file, &destination)?;
        bytes_copied = bytes_copied
            .checked_add(copied.verified.byte_len)
            .ok_or_else(|| Error::new(ErrorCode::TooBig, "restore byte total overflow"))?;
        destination_files.push(copied);
    }
    let mut destination_inventory = manifest_file_list(src, &manifest)?;
    verify_destination_files(&destination, &mut destination_files, &destination_inventory)?;
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
    refresh_destination_file(
        &destination,
        &mut destination_files,
        &Path::new(PHASE8_DIR).join(IDENTITY_FILE),
    )?;
    verify_destination_files(&destination, &mut destination_files, &destination_inventory)?;

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
    match retain_destination_file(&destination, Path::new("owner.lock")) {
        Ok(owner_lock) => {
            destination_inventory.push(PathBuf::from("owner.lock"));
            destination_files.push(owner_lock);
        }
        Err(error) if error.code() == ErrorCode::NotFound => {}
        Err(error) => return Err(error),
    }
    verify_destination_files(&destination, &mut destination_files, &destination_inventory)?;
    write_text_atomic(
        &phase8_path(&retained_dst).join(RESTORE_COMPLETE_FILE),
        "ok\n",
    )?;
    let restore_marker = Path::new(PHASE8_DIR).join(RESTORE_COMPLETE_FILE);
    destination_files.push(retain_destination_file(&destination, &restore_marker)?);
    destination_inventory.push(restore_marker);
    verify_destination_files(&destination, &mut destination_files, &destination_inventory)?;
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
        || manifest.files.len() > MAX_BACKUP_FILES
        || manifest.file_entries.len() != manifest.files.len()
        || manifest.total_bytes > MAX_BACKUP_TOTAL_BYTES
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
    let mut declared_total = 0_u64;
    for (path, entry) in manifest.files.iter().zip(&manifest.file_entries) {
        if path != &entry.relative_path
            || entry.byte_len > MAX_BACKUP_FILE_BYTES
            || entry.sha256.len() != 64
            || !entry
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(Error::new(
                ErrorCode::Corrupt,
                "invalid physical backup file authority",
            ));
        }
        declared_total = declared_total
            .checked_add(entry.byte_len)
            .ok_or_else(|| Error::new(ErrorCode::TooBig, "backup byte total overflow"))?;
    }
    if declared_total != manifest.total_bytes {
        return Err(Error::new(
            ErrorCode::Corrupt,
            "physical backup byte total mismatch",
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
    let mut expected_inventory = paths.clone();
    expected_inventory.push(Path::new(PHASE8_DIR).join(PHYSICAL_BACKUP_MANIFEST_FILE));
    expected_inventory.push(Path::new(PHASE8_DIR).join(COMPLETE_FILE));
    verify_closed_inventory(&root, &expected_inventory)?;
    let mut files = Vec::with_capacity(paths.len());
    let mut tree_hasher = Sha256::new();
    let mut total_bytes = 0_u64;
    for (path, expected) in paths.into_iter().zip(&manifest.file_entries) {
        let relative_path = rel_to_string(&path);
        tree_hasher.update(relative_path.as_bytes());
        let mut file = root.open_regular_file(&path)?;
        let metadata = file.metadata()?;
        if metadata.len() != expected.byte_len || expected.relative_path != relative_path {
            return Err(Error::new(
                ErrorCode::Corrupt,
                "backup file size or path differs from manifest",
            ));
        }
        let mut file_hasher = Sha256::new();
        let mut remaining = expected.byte_len;
        let mut buf = [0_u8; 8192];
        while remaining > 0 {
            let take = remaining.min(buf.len() as u64) as usize;
            let read = file.read(&mut buf[..take])?;
            if read == 0 {
                return Err(Error::new(
                    ErrorCode::Corrupt,
                    "backup file ended before its declared size",
                ));
            }
            file_hasher.update(&buf[..read]);
            tree_hasher.update(&buf[..read]);
            remaining -= read as u64;
        }
        let mut extra = [0_u8; 1];
        if file.read(&mut extra)? != 0 {
            return Err(Error::new(
                ErrorCode::Corrupt,
                "backup file exceeds its declared size",
            ));
        }
        let digest = format!("{:x}", file_hasher.finalize());
        if digest != expected.sha256 {
            return Err(Error::new(
                ErrorCode::Corrupt,
                "backup file digest differs from manifest",
            ));
        }
        total_bytes = total_bytes
            .checked_add(expected.byte_len)
            .ok_or_else(|| Error::new(ErrorCode::TooBig, "backup byte total overflow"))?;
        files.push(OpenedBackupFile {
            verified: expected.clone(),
            file,
            dev: metadata.dev(),
            ino: metadata.ino(),
        });
    }
    if total_bytes != manifest.total_bytes {
        return Err(Error::new(
            ErrorCode::Corrupt,
            "verified backup byte total mismatch",
        ));
    }
    if format!("{:x}", tree_hasher.finalize()) != manifest.tree_hash {
        return Err(Error::new(ErrorCode::Corrupt, "backup tree hash mismatch"));
    }
    for file in &files {
        root.revalidate_file(file)?;
    }
    root.revalidate_path(src)?;
    verify_closed_inventory(&root, &expected_inventory)?;
    Ok(OpenedBackup {
        root,
        manifest,
        files,
    })
}

pub fn create_physical_slot(db: &Database, name: &str, active: bool) -> Result<ReplicationSlot> {
    let _retention_guard = lock_retention_authority(db)?;
    validate_slot_name(name)?;
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
    let _retention_guard = lock_retention_authority(db)?;
    validate_slot_name(name)?;
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
    let _retention_guard = lock_retention_authority(db)?;
    validate_slot_name(name)?;
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
    let identity = load_identity_or_init(db.path())?;
    let slots_root = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&dir)?;
    let root_metadata = slots_root.metadata()?;
    if !root_metadata.file_type().is_dir() {
        return Err(Error::new(
            ErrorCode::Corrupt,
            "replication slot root is not a native directory",
        ));
    }
    let retained_dir = PathBuf::from(format!("/proc/self/fd/{}", slots_root.as_raw_fd()));
    for entry in fs::read_dir(&retained_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            return Err(Error::new(
                ErrorCode::Corrupt,
                "replication slot directory contains an undeclared entry",
            ));
        }
        let file_name = entry
            .file_name()
            .into_string()
            .map_err(|_| Error::new(ErrorCode::Corrupt, "replication slot name is not UTF-8"))?;
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(entry.path())?;
        let metadata = file.metadata()?;
        if !metadata.file_type().is_file() || metadata.nlink() != 1 {
            return Err(Error::new(
                ErrorCode::Corrupt,
                "replication slot is not a native single-link file",
            ));
        }
        let slot: SlotFile = read_bounded_json(&mut file, MAX_SLOT_FILE_BYTES)?;
        validate_slot_file(&file_name, &slot, &identity)?;
        let final_metadata = file.metadata()?;
        let reopened = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(entry.path())?;
        let reopened_metadata = reopened.metadata()?;
        if final_metadata.dev() != metadata.dev()
            || final_metadata.ino() != metadata.ino()
            || final_metadata.len() != metadata.len()
            || final_metadata.nlink() != 1
            || reopened_metadata.dev() != metadata.dev()
            || reopened_metadata.ino() != metadata.ino()
            || reopened_metadata.len() != metadata.len()
            || reopened_metadata.nlink() != 1
        {
            return Err(Error::new(
                ErrorCode::Corrupt,
                "replication slot changed while reading",
            ));
        }
        slots.push(slot.stats);
    }
    let final_root = fs::symlink_metadata(&dir)?;
    if !final_root.file_type().is_dir()
        || final_root.dev() != root_metadata.dev()
        || final_root.ino() != root_metadata.ino()
    {
        return Err(Error::new(
            ErrorCode::Corrupt,
            "replication slot root identity changed",
        ));
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
    let _retention_guard = lock_retention_authority(db)?;
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

pub(crate) fn lock_retention_authority(db: &Database) -> Result<MutexGuard<'_, ()>> {
    db.inner
        .retention_lock
        .lock()
        .map_err(|_| Error::new(ErrorCode::Error, "retention authority lock poisoned"))
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
            if out.len() > MAX_BACKUP_FILES {
                return Err(Error::new(
                    ErrorCode::TooBig,
                    "backup file count exceeds the closed limit",
                ));
            }
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

fn copy_file_exclusive(source: &NativeRoot, destination: &NativeRoot, rel: &Path) -> Result<u64> {
    let mut input = source.open_regular_file(rel)?;
    let metadata = input.metadata()?;
    if metadata.len() > MAX_BACKUP_FILE_BYTES {
        return Err(Error::new(
            ErrorCode::TooBig,
            "backup source file exceeds the closed per-file limit",
        ));
    }
    let mut output = destination.create_regular_file(rel)?;
    let mut remaining = metadata.len();
    let mut buffer = [0_u8; 8192];
    while remaining > 0 {
        let take = remaining.min(buffer.len() as u64) as usize;
        let read = input.read(&mut buffer[..take])?;
        if read == 0 {
            return Err(Error::new(
                ErrorCode::Corrupt,
                "backup source file ended while copying",
            ));
        }
        output.write_all(&buffer[..read])?;
        remaining -= read as u64;
    }
    let mut extra = [0_u8; 1];
    if input.read(&mut extra)? != 0 {
        return Err(Error::new(
            ErrorCode::Corrupt,
            "backup source file grew beyond its bounded size",
        ));
    }
    output.sync_all()?;
    let final_source = input.metadata()?;
    let final_output = output.metadata()?;
    if final_source.dev() != metadata.dev()
        || final_source.ino() != metadata.ino()
        || final_source.len() != metadata.len()
        || final_source.nlink() != 1
        || final_output.len() != metadata.len()
        || final_output.nlink() != 1
    {
        return Err(Error::new(
            ErrorCode::Corrupt,
            "backup source or destination changed while copying",
        ));
    }
    Ok(metadata.len())
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
            libc::O_RDWR | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )?;
        parent.sync_all()?;
        Ok(output)
    }

    fn revalidate_file(&self, file: &OpenedBackupFile) -> Result<()> {
        self.revalidate_opened_file(
            Path::new(&file.verified.relative_path),
            &file.file,
            file.dev,
            file.ino,
        )
    }

    fn revalidate_opened_file(
        &self,
        relative_path: &Path,
        file: &File,
        dev: u64,
        ino: u64,
    ) -> Result<()> {
        let reopened = self.open_regular_file(relative_path)?;
        let metadata = reopened.metadata()?;
        let retained_metadata = file.metadata()?;
        if metadata.dev() != dev
            || metadata.ino() != ino
            || retained_metadata.dev() != dev
            || retained_metadata.ino() != ino
            || metadata.nlink() != 1
            || retained_metadata.nlink() != 1
        {
            return Err(Error::new(
                ErrorCode::Corrupt,
                "retained file identity changed after verification",
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

fn copy_opened_file(
    file: &mut OpenedBackupFile,
    destination: &NativeRoot,
) -> Result<OpenedDestinationFile> {
    file.file.seek(SeekFrom::Start(0))?;
    let mut output = destination.create_regular_file(Path::new(&file.verified.relative_path))?;
    let mut hasher = Sha256::new();
    let mut remaining = file.verified.byte_len;
    let mut buffer = [0_u8; 8192];
    while remaining > 0 {
        let take = remaining.min(buffer.len() as u64) as usize;
        let read = file.file.read(&mut buffer[..take])?;
        if read == 0 {
            return Err(Error::new(
                ErrorCode::Corrupt,
                "backup file ended while restoring",
            ));
        }
        output.write_all(&buffer[..read])?;
        hasher.update(&buffer[..read]);
        remaining -= read as u64;
    }
    let mut extra = [0_u8; 1];
    if file.file.read(&mut extra)? != 0 {
        return Err(Error::new(
            ErrorCode::Corrupt,
            "backup file grew beyond its verified size",
        ));
    }
    if format!("{:x}", hasher.finalize()) != file.verified.sha256 {
        return Err(Error::new(
            ErrorCode::Corrupt,
            "backup file changed while restoring",
        ));
    }
    output.sync_all()?;
    let metadata = output.metadata()?;
    if metadata.len() != file.verified.byte_len || metadata.nlink() != 1 {
        return Err(Error::new(
            ErrorCode::Corrupt,
            "restored file size or custody is invalid",
        ));
    }
    Ok(OpenedDestinationFile {
        verified: file.verified.clone(),
        file: output,
        dev: metadata.dev(),
        ino: metadata.ino(),
    })
}

fn verify_destination_files(
    destination: &NativeRoot,
    files: &mut [OpenedDestinationFile],
    expected_inventory: &[PathBuf],
) -> Result<()> {
    for expected in files {
        destination.revalidate_opened_file(
            Path::new(&expected.verified.relative_path),
            &expected.file,
            expected.dev,
            expected.ino,
        )?;
        expected.file.seek(SeekFrom::Start(0))?;
        let mut hasher = Sha256::new();
        let mut remaining = expected.verified.byte_len;
        let mut buffer = [0_u8; 8192];
        while remaining > 0 {
            let take = remaining.min(buffer.len() as u64) as usize;
            let read = expected.file.read(&mut buffer[..take])?;
            if read == 0 {
                return Err(Error::new(
                    ErrorCode::Corrupt,
                    "restored file ended before its retained size",
                ));
            }
            hasher.update(&buffer[..read]);
            remaining -= read as u64;
        }
        let mut extra = [0_u8; 1];
        if expected.file.read(&mut extra)? != 0
            || format!("{:x}", hasher.finalize()) != expected.verified.sha256
        {
            return Err(Error::new(
                ErrorCode::Corrupt,
                "restored file does not match verified backup bytes",
            ));
        }
        destination.revalidate_opened_file(
            Path::new(&expected.verified.relative_path),
            &expected.file,
            expected.dev,
            expected.ino,
        )?;
    }
    verify_closed_inventory(destination, expected_inventory)?;
    destination.dir.sync_all()?;
    Ok(())
}

fn refresh_destination_file(
    destination: &NativeRoot,
    files: &mut [OpenedDestinationFile],
    relative_path: &Path,
) -> Result<()> {
    let target = files
        .iter_mut()
        .find(|file| Path::new(&file.verified.relative_path) == relative_path)
        .ok_or_else(|| {
            Error::new(
                ErrorCode::Corrupt,
                "restore transformation targets an undeclared file",
            )
        })?;
    let mut file = destination.open_regular_file(relative_path)?;
    let metadata = file.metadata()?;
    if metadata.len() > MAX_BACKUP_FILE_BYTES {
        return Err(Error::new(
            ErrorCode::TooBig,
            "transformed restore file exceeds the closed limit",
        ));
    }
    let mut hasher = Sha256::new();
    let mut remaining = metadata.len();
    let mut buffer = [0_u8; 8192];
    while remaining > 0 {
        let take = remaining.min(buffer.len() as u64) as usize;
        let read = file.read(&mut buffer[..take])?;
        if read == 0 {
            return Err(Error::new(
                ErrorCode::Corrupt,
                "transformed restore file ended early",
            ));
        }
        hasher.update(&buffer[..read]);
        remaining -= read as u64;
    }
    let mut extra = [0_u8; 1];
    if file.read(&mut extra)? != 0 {
        return Err(Error::new(
            ErrorCode::Corrupt,
            "transformed restore file grew while retaining custody",
        ));
    }
    target.verified.byte_len = metadata.len();
    target.verified.sha256 = format!("{:x}", hasher.finalize());
    target.file = file;
    target.dev = metadata.dev();
    target.ino = metadata.ino();
    Ok(())
}

fn retain_destination_file(
    destination: &NativeRoot,
    relative_path: &Path,
) -> Result<OpenedDestinationFile> {
    let mut file = destination.open_regular_file(relative_path)?;
    let metadata = file.metadata()?;
    if metadata.len() > MAX_BACKUP_FILE_BYTES {
        return Err(Error::new(
            ErrorCode::TooBig,
            "runtime restore file exceeds the closed limit",
        ));
    }
    let mut hasher = Sha256::new();
    let mut remaining = metadata.len();
    let mut buffer = [0_u8; 8192];
    while remaining > 0 {
        let take = remaining.min(buffer.len() as u64) as usize;
        let read = file.read(&mut buffer[..take])?;
        if read == 0 {
            return Err(Error::new(
                ErrorCode::Corrupt,
                "runtime restore file ended early",
            ));
        }
        hasher.update(&buffer[..read]);
        remaining -= read as u64;
    }
    let mut extra = [0_u8; 1];
    if file.read(&mut extra)? != 0 {
        return Err(Error::new(
            ErrorCode::Corrupt,
            "runtime restore file grew while retaining custody",
        ));
    }
    destination.revalidate_opened_file(relative_path, &file, metadata.dev(), metadata.ino())?;
    Ok(OpenedDestinationFile {
        verified: VerifiedBackupFile {
            relative_path: rel_to_string(relative_path),
            byte_len: metadata.len(),
            sha256: format!("{:x}", hasher.finalize()),
        },
        file,
        dev: metadata.dev(),
        ino: metadata.ino(),
    })
}

fn verify_files_for_manifest(
    root: &NativeRoot,
    files: &[PathBuf],
) -> Result<(Vec<VerifiedBackupFile>, u64, String)> {
    if files.is_empty() || files.len() > MAX_BACKUP_FILES {
        return Err(Error::new(
            ErrorCode::TooBig,
            "backup file count is outside the closed limit",
        ));
    }
    let mut tree_hasher = Sha256::new();
    let mut entries = Vec::with_capacity(files.len());
    let mut total_bytes = 0_u64;
    for rel in files {
        let relative_path = rel_to_string(rel);
        tree_hasher.update(relative_path.as_bytes());
        let mut file = root.open_regular_file(rel)?;
        let metadata = file.metadata()?;
        if metadata.len() > MAX_BACKUP_FILE_BYTES {
            return Err(Error::new(
                ErrorCode::TooBig,
                "backup file exceeds the closed per-file limit",
            ));
        }
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
                .ok_or_else(|| Error::new(ErrorCode::TooBig, "backup file size overflow"))?;
            if byte_len > MAX_BACKUP_FILE_BYTES {
                return Err(Error::new(
                    ErrorCode::TooBig,
                    "backup file exceeds the closed per-file limit",
                ));
            }
            file_hasher.update(&buf[..read]);
            tree_hasher.update(&buf[..read]);
        }
        if byte_len != metadata.len() {
            return Err(Error::new(
                ErrorCode::Corrupt,
                "backup file changed while building the manifest",
            ));
        }
        total_bytes = total_bytes
            .checked_add(byte_len)
            .ok_or_else(|| Error::new(ErrorCode::TooBig, "backup byte total overflow"))?;
        if total_bytes > MAX_BACKUP_TOTAL_BYTES {
            return Err(Error::new(
                ErrorCode::TooBig,
                "backup exceeds the closed aggregate limit",
            ));
        }
        entries.push(VerifiedBackupFile {
            relative_path,
            byte_len,
            sha256: format!("{:x}", file_hasher.finalize()),
        });
    }
    Ok((
        entries,
        total_bytes,
        format!("{:x}", tree_hasher.finalize()),
    ))
}

fn manifest_file_list(_root: &Path, manifest: &PhysicalBackupManifest) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::with_capacity(manifest.files.len());
    let mut previous = None;
    for (raw, entry) in manifest.files.iter().zip(&manifest.file_entries) {
        if raw != &entry.relative_path {
            return Err(Error::new(
                ErrorCode::Corrupt,
                "backup manifest file authorities disagree",
            ));
        }
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

fn verify_closed_inventory(root: &NativeRoot, expected_files: &[PathBuf]) -> Result<()> {
    let expected_files = expected_files
        .iter()
        .map(|path| rel_to_string(path))
        .collect::<BTreeSet<_>>();
    let mut expected_dirs = BTreeSet::new();
    for path in expected_files.iter().map(Path::new) {
        let mut parent = path.parent();
        while let Some(value) = parent {
            if value.as_os_str().is_empty() {
                break;
            }
            expected_dirs.insert(rel_to_string(value));
            parent = value.parent();
        }
    }
    let mut actual_files = BTreeSet::new();
    collect_native_inventory(
        &root.proc_path(),
        Path::new(""),
        &expected_dirs,
        &mut actual_files,
    )?;
    if actual_files != expected_files {
        return Err(Error::new(
            ErrorCode::Corrupt,
            "native file inventory differs from the closed manifest",
        ));
    }
    Ok(())
}

fn collect_native_inventory(
    current: &Path,
    relative: &Path,
    expected_dirs: &BTreeSet<String>,
    files: &mut BTreeSet<String>,
) -> Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let child_relative = relative.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            let key = rel_to_string(&child_relative);
            if !expected_dirs.contains(&key) {
                return Err(Error::new(
                    ErrorCode::Corrupt,
                    "native inventory contains an undeclared directory",
                ));
            }
            collect_native_inventory(&entry.path(), &child_relative, expected_dirs, files)?;
        } else if file_type.is_file() {
            let metadata = entry.metadata()?;
            if metadata.nlink() != 1 || !files.insert(rel_to_string(&child_relative)) {
                return Err(Error::new(
                    ErrorCode::Corrupt,
                    "native inventory contains a non-native or duplicate file",
                ));
            }
        } else {
            return Err(Error::new(
                ErrorCode::Corrupt,
                "native inventory contains an undeclared entry type",
            ));
        }
    }
    Ok(())
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

fn validate_slot_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 128
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(Error::new(
            ErrorCode::Misuse,
            "invalid replication slot name",
        ));
    }
    Ok(())
}

fn validate_slot_file(file_name: &str, slot: &SlotFile, identity: &IdentityFile) -> Result<()> {
    validate_slot_name(&slot.stats.name).map_err(|_| {
        Error::new(
            ErrorCode::Corrupt,
            "invalid persisted replication slot name",
        )
    })?;
    if slot.format_version != FORMAT_VERSION
        || slot.created_unix_nanos == 0
        || file_name != format!("{}.json", slot.stats.name)
        || slot.stats.slot_id == 0
        || slot.stats.database_id != identity.db_id
        || slot.stats.timeline != identity.timeline
        || slot.stats.confirmed_flush_lsn < slot.stats.restart_lsn
        || slot.stats.confirmed_flush_csn < slot.stats.restart_csn
    {
        return Err(Error::new(
            ErrorCode::Corrupt,
            "replication slot identity is invalid",
        ));
    }
    Ok(())
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
    validate_slot_name(&slot.name)?;
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

fn read_bounded_json<T: for<'de> Deserialize<'de>>(file: &mut File, max_bytes: u64) -> Result<T> {
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::new();
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max_bytes {
        return Err(Error::new(
            ErrorCode::TooBig,
            "JSON authority file is too large",
        ));
    }
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
    fn growing_source_is_rejected_before_excess_reaches_destination() {
        let temp = tempfile::tempdir().expect("tempdir");
        let src = temp.path().join("src");
        let dst = temp.path().join("dst");
        fs::create_dir(&src).expect("src");
        fs::create_dir(&dst).expect("dst");
        fs::write(src.join("data"), b"verified").expect("verified");
        let source = NativeRoot::open(&src).expect("source root");
        let mut opened = opened_file(&source, Path::new("data"));
        OpenOptions::new()
            .append(true)
            .open(src.join("data"))
            .expect("append handle")
            .write_all(b"-excess")
            .expect("grow source");

        let destination = NativeRoot::open(&dst).expect("destination root");
        assert_eq!(
            copy_opened_file(&mut opened, &destination)
                .err()
                .expect("growth must fail")
                .code(),
            ErrorCode::Corrupt
        );
        assert_eq!(
            fs::read(dst.join("data")).expect("bounded output"),
            b"verified"
        );
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

    #[test]
    fn retained_destination_rejects_mutation_replacement_and_extra_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let src = temp.path().join("src");
        let dst = temp.path().join("dst");
        fs::create_dir(&src).expect("src");
        fs::create_dir(&dst).expect("dst");
        fs::write(src.join("data"), b"verified").expect("verified");
        let source = NativeRoot::open(&src).expect("source root");
        let mut source_file = opened_file(&source, Path::new("data"));
        let destination = NativeRoot::open(&dst).expect("destination root");
        let mut copied =
            vec![copy_opened_file(&mut source_file, &destination).expect("copy retained")];
        let inventory = vec![PathBuf::from("data")];
        verify_destination_files(&destination, &mut copied, &inventory).expect("verify");

        fs::write(dst.join("extra"), b"undeclared").expect("extra");
        assert_eq!(
            verify_destination_files(&destination, &mut copied, &inventory)
                .expect_err("extra file must fail")
                .code(),
            ErrorCode::Corrupt
        );
        fs::remove_file(dst.join("extra")).expect("remove extra");

        fs::write(dst.join("data"), b"mutated!").expect("mutate in place");
        assert_eq!(
            verify_destination_files(&destination, &mut copied, &inventory)
                .expect_err("mutation must fail")
                .code(),
            ErrorCode::Corrupt
        );

        fs::rename(dst.join("data"), dst.join("data.held")).expect("hold inode");
        fs::write(dst.join("data"), b"verified").expect("replacement");
        assert_eq!(
            verify_destination_files(&destination, &mut copied, &inventory)
                .expect_err("replacement must fail")
                .code(),
            ErrorCode::Corrupt
        );
    }

    #[test]
    fn retention_policy_mutators_and_checkpoint_share_a_fail_closed_lock() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db = Database::create(temp.path().join("retention")).expect("database");
        let poison = db.clone();
        std::thread::spawn(move || {
            let _guard = poison.inner.retention_lock.lock().expect("retention lock");
            panic!("poison retention authority");
        })
        .join()
        .expect_err("thread must poison lock");

        assert_eq!(
            db.set_archive_mode(ArchiveMode::RequiredLocal)
                .expect_err("archive mutator must acquire lock")
                .code(),
            ErrorCode::Error
        );
        assert_eq!(
            db.create_physical_slot("blocked")
                .expect_err("slot mutator must acquire lock")
                .code(),
            ErrorCode::Error
        );
        assert_eq!(
            db.checkpoint()
                .expect_err("checkpoint must acquire the same lock")
                .code(),
            ErrorCode::Error
        );
    }
}
