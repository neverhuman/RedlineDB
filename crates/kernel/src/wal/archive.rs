use std::ffi::{CStr, CString};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Component, Path};

use sha2::{Digest, Sha256};

use crate::format::{Lsn, TimelineId};
use crate::{Error, Result};

use super::{offset_for_lsn, segment_for_lsn, segment_path};

pub const ARCHIVE_SEAL_BYTES: u64 = 16 * 1024 * 1024;
pub const ARCHIVE_SEAL_AFTER_MS: u64 = 5_000;
pub const ARCHIVE_LAG_ALERT_AFTER_MS: u64 = 30_000;

const WATERMARK_FILE: &str = "archive.watermark";
const WATERMARK_PENDING_FILE: &str = "archive.watermark.next";
const WATERMARK_LOCK_FILE: &str = "archive.watermark.lock";
const WATERMARK_VERSION: u16 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArchiveSealPolicy {
    pub max_unarchived_bytes: u64,
    pub max_unarchived_age_ms: u64,
}

impl Default for ArchiveSealPolicy {
    fn default() -> Self {
        Self {
            max_unarchived_bytes: ARCHIVE_SEAL_BYTES,
            max_unarchived_age_ms: ARCHIVE_SEAL_AFTER_MS,
        }
    }
}

impl ArchiveSealPolicy {
    pub fn should_seal(
        self,
        unarchived_bytes: u64,
        first_unarchived_at_ms: Option<u64>,
        now_ms: u64,
    ) -> bool {
        if unarchived_bytes == 0 {
            return false;
        }
        if unarchived_bytes >= self.max_unarchived_bytes {
            return true;
        }
        first_unarchived_at_ms
            .is_some_and(|first| now_ms.saturating_sub(first) >= self.max_unarchived_age_ms)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArchiveLag {
    pub lag_ms: u64,
    pub alert: bool,
}

pub fn archive_lag(first_unarchived_at_ms: Option<u64>, now_ms: u64) -> ArchiveLag {
    let lag_ms = first_unarchived_at_ms
        .map(|first| now_ms.saturating_sub(first))
        .unwrap_or(0);
    ArchiveLag {
        lag_ms,
        alert: lag_ms > ARCHIVE_LAG_ALERT_AFTER_MS,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SealedWalRange {
    pub timeline: TimelineId,
    pub start_lsn: Lsn,
    pub end_lsn: Lsn,
    pub byte_len: u64,
    pub sha256: [u8; 32],
}

impl SealedWalRange {
    pub fn validate(&self) -> Result<()> {
        if self.timeline == TimelineId::ZERO
            || self.start_lsn >= self.end_lsn
            || self.byte_len == 0
            || self.byte_len != self.end_lsn.0.saturating_sub(self.start_lsn.0)
            || self.sha256.iter().all(|byte| *byte == 0)
        {
            return Err(Error::CorruptWal("invalid sealed wal range"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DurableWalPrefix {
    pub range: SealedWalRange,
    pub bytes: Vec<u8>,
}

pub fn read_durable_prefix(
    wal_dir: impl AsRef<Path>,
    segment_bytes: u64,
    timeline: TimelineId,
    start_lsn: Lsn,
    durable_lsn: Lsn,
    max_bytes: usize,
) -> Result<Option<DurableWalPrefix>> {
    if segment_bytes == 0 || max_bytes == 0 || start_lsn > durable_lsn {
        return Err(Error::CorruptWal("invalid durable wal prefix bounds"));
    }
    if start_lsn == durable_lsn {
        return Ok(None);
    }

    let wal_dir = NativeDirectory::open(wal_dir.as_ref(), false)?;
    wal_dir.revalidate()?;
    let mut cursor = start_lsn;
    let address_bytes =
        usize::try_from(durable_lsn.0.saturating_sub(start_lsn.0)).unwrap_or(usize::MAX);
    let mut bytes = Vec::with_capacity(max_bytes.min(address_bytes));
    while cursor < durable_lsn && bytes.len() < max_bytes {
        let segment = segment_for_lsn(cursor, segment_bytes);
        let path = segment_path(Path::new(""), segment);
        let name = path
            .file_name()
            .ok_or(Error::CorruptWal("invalid wal segment name"))?;
        let name = CString::new(name.as_bytes())
            .map_err(|_| Error::CorruptWal("invalid wal segment name"))?;
        let mut file = openat(
            &wal_dir.dir,
            &name,
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0,
        )?;
        let metadata = file.metadata()?;
        if !metadata.file_type().is_file() || metadata.nlink() != 1 {
            return Err(Error::CorruptWal(
                "wal segment is not a native regular file",
            ));
        }
        let file_len = metadata.len();
        if file_len > segment_bytes
            || (file_len > 0 && metadata.blocks().saturating_mul(512) < file_len)
        {
            return Err(Error::CorruptWal("wal segment has invalid physical extent"));
        }
        let segment_start = segment
            .0
            .checked_sub(1)
            .and_then(|value| value.checked_mul(segment_bytes))
            .ok_or(Error::CorruptWal("wal segment address overflow"))?;
        let segment_end = segment_start
            .checked_add(segment_bytes)
            .ok_or(Error::CorruptWal("wal segment address overflow"))?;
        let required_end = durable_lsn.0.min(segment_end);
        let required_len = required_end.saturating_sub(segment_start);
        if file_len < required_len {
            return Err(Error::CorruptWal("wal segment contains an address gap"));
        }
        let offset = offset_for_lsn(cursor, segment_bytes);
        if offset >= required_len {
            return Err(Error::CorruptWal("wal range starts beyond segment bytes"));
        }

        let address_remaining = required_end.saturating_sub(cursor.0);
        let capacity = (max_bytes - bytes.len()) as u64;
        let take = address_remaining.min(capacity);
        file.seek(SeekFrom::Start(offset))?;
        let start_len = bytes.len();
        bytes.resize(start_len + take as usize, 0);
        file.read_exact(&mut bytes[start_len..])?;
        cursor = Lsn(cursor.0.saturating_add(take));

        revalidate_named_file(&wal_dir, &name, &file, &metadata)?;
        let final_metadata = file.metadata()?;
        if final_metadata.len() != file_len {
            return Err(Error::CorruptWal(
                "wal segment identity changed while reading",
            ));
        }
    }

    if bytes.is_empty() {
        return Err(Error::CorruptWal("durable wal prefix contains no bytes"));
    }
    let sha256: [u8; 32] = Sha256::digest(&bytes).into();
    let range = SealedWalRange {
        timeline,
        start_lsn,
        end_lsn: cursor,
        byte_len: bytes.len() as u64,
        sha256,
    };
    range.validate()?;
    Ok(Some(DurableWalPrefix { range, bytes }))
}

fn revalidate_named_file(
    directory: &NativeDirectory,
    name: &CStr,
    file: &File,
    initial_metadata: &fs::Metadata,
) -> Result<()> {
    directory.revalidate()?;
    let path_metadata = statat(&directory.dir, name)?;
    let final_metadata = file.metadata()?;
    if path_metadata.st_mode & libc::S_IFMT != libc::S_IFREG
        || path_metadata.st_dev as u64 != initial_metadata.dev()
        || path_metadata.st_ino as u64 != initial_metadata.ino()
        || final_metadata.dev() != initial_metadata.dev()
        || final_metadata.ino() != initial_metadata.ino()
        || final_metadata.nlink() != 1
    {
        return Err(Error::CorruptWal(
            "wal segment identity changed while reading",
        ));
    }
    directory.revalidate()?;
    Ok(())
}

pub trait ArchiveReceiptVerifier {
    fn verify(&self, range: &SealedWalRange, receipt: &[u8]) -> Result<()>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedArchiveReceipt {
    range: SealedWalRange,
    receipt_sha256: [u8; 32],
}

impl VerifiedArchiveReceipt {
    pub fn verify(
        range: SealedWalRange,
        receipt: &[u8],
        verifier: &dyn ArchiveReceiptVerifier,
    ) -> Result<Self> {
        range.validate()?;
        if receipt.is_empty() {
            return Err(Error::CorruptWal("archive receipt is empty"));
        }
        verifier.verify(&range, receipt)?;
        Ok(Self {
            range,
            receipt_sha256: Sha256::digest(receipt).into(),
        })
    }

    pub fn range(&self) -> &SealedWalRange {
        &self.range
    }

    pub fn receipt_sha256(&self) -> [u8; 32] {
        self.receipt_sha256
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArchiveWatermark {
    pub timeline: TimelineId,
    pub archived_lsn: Lsn,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WatermarkState {
    watermark: ArchiveWatermark,
    previous_lsn: Lsn,
    receipt_sha256: [u8; 32],
}

struct DirectoryBinding {
    parent: File,
    name: CString,
    dev: u64,
    ino: u64,
}

struct NativeDirectory {
    dir: File,
    dev: u64,
    ino: u64,
    bindings: Vec<DirectoryBinding>,
}

pub fn archive_watermark(
    state_dir: impl AsRef<Path>,
    timeline: TimelineId,
) -> Result<ArchiveWatermark> {
    if timeline == TimelineId::ZERO {
        return Err(Error::CorruptWal("archive timeline must be nonzero"));
    }
    validate_state_dir(state_dir.as_ref())?;
    let state = match open_state_dir(state_dir.as_ref(), false) {
        Ok(state) => state,
        Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(empty_watermark(timeline).watermark);
        }
        Err(error) => return Err(error),
    };
    Ok(read_watermark(&state, timeline)?.watermark)
}

pub fn advance_archive_watermark(
    state_dir: impl AsRef<Path>,
    receipt: &VerifiedArchiveReceipt,
) -> Result<ArchiveWatermark> {
    let state_dir = state_dir.as_ref();
    validate_state_dir(state_dir)?;
    let state = open_state_dir(state_dir, true)?;
    let _lock = lock_watermark(&state)?;
    let current = recover_pending_watermark(&state, receipt.range.timeline)?;
    if current.watermark.generation > 0
        && current.previous_lsn == receipt.range.start_lsn
        && current.watermark.archived_lsn == receipt.range.end_lsn
        && current.receipt_sha256 == receipt.receipt_sha256
    {
        return Ok(current.watermark);
    }
    if current.watermark.archived_lsn != receipt.range.start_lsn {
        return Err(Error::CorruptWal("archive receipt is not contiguous"));
    }
    let next = WatermarkState {
        watermark: ArchiveWatermark {
            timeline: current.watermark.timeline,
            archived_lsn: receipt.range.end_lsn,
            generation: current
                .watermark
                .generation
                .checked_add(1)
                .ok_or(Error::CorruptWal("archive watermark generation overflow"))?,
        },
        previous_lsn: current.watermark.archived_lsn,
        receipt_sha256: receipt.receipt_sha256,
    };
    write_watermark(&state, next)?;
    Ok(next.watermark)
}

fn validate_state_dir(state_dir: &Path) -> Result<()> {
    if state_dir.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::CurDir | Component::Prefix(_)
        )
    }) {
        return Err(Error::CorruptWal("invalid archive state directory"));
    }
    Ok(())
}

fn empty_watermark(timeline: TimelineId) -> WatermarkState {
    WatermarkState {
        watermark: ArchiveWatermark {
            timeline,
            archived_lsn: Lsn::ZERO,
            generation: 0,
        },
        previous_lsn: Lsn::ZERO,
        receipt_sha256: [0; 32],
    }
}

impl NativeDirectory {
    fn open(path: &Path, create: bool) -> Result<Self> {
        if path.as_os_str().is_empty() {
            return Err(Error::CorruptWal("invalid native directory path"));
        }
        let start = if path.is_absolute() { "/" } else { "." };
        let mut directory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(start)?;
        let mut bindings = Vec::new();
        for component in path.components() {
            let component = match component {
                Component::RootDir | Component::CurDir => continue,
                Component::Normal(component) => component,
                Component::ParentDir | Component::Prefix(_) => {
                    return Err(Error::CorruptWal("invalid native directory path"));
                }
            };
            let name = CString::new(component.as_bytes())
                .map_err(|_| Error::CorruptWal("invalid native directory path"))?;
            let flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
            let opened = match openat(&directory, &name, flags, 0) {
                Ok(opened) => opened,
                Err(error) if create && error.kind() == std::io::ErrorKind::NotFound => {
                    mkdirat(&directory, &name)?;
                    directory.sync_all()?;
                    openat(&directory, &name, flags, 0)?
                }
                Err(error) => return Err(error.into()),
            };
            let metadata = opened.metadata()?;
            let observed = statat(&directory, &name)?;
            if !metadata.file_type().is_dir()
                || observed.st_mode & libc::S_IFMT != libc::S_IFDIR
                || observed.st_dev as u64 != metadata.dev()
                || observed.st_ino as u64 != metadata.ino()
            {
                return Err(Error::CorruptWal("native directory ancestry is not stable"));
            }
            bindings.push(DirectoryBinding {
                parent: directory,
                name,
                dev: metadata.dev(),
                ino: metadata.ino(),
            });
            directory = opened;
        }
        let metadata = directory.metadata()?;
        if !metadata.file_type().is_dir() {
            return Err(Error::CorruptWal(
                "native directory path is not a directory",
            ));
        }
        let native = Self {
            dir: directory,
            dev: metadata.dev(),
            ino: metadata.ino(),
            bindings,
        };
        native.revalidate()?;
        Ok(native)
    }

    fn revalidate(&self) -> Result<()> {
        for binding in &self.bindings {
            let observed = statat(&binding.parent, &binding.name)?;
            if observed.st_mode & libc::S_IFMT != libc::S_IFDIR
                || observed.st_dev as u64 != binding.dev
                || observed.st_ino as u64 != binding.ino
            {
                return Err(Error::CorruptWal("native directory ancestry changed"));
            }
        }
        let metadata = self.dir.metadata()?;
        if !metadata.file_type().is_dir()
            || metadata.dev() != self.dev
            || metadata.ino() != self.ino
        {
            return Err(Error::CorruptWal("native directory ancestry changed"));
        }
        Ok(())
    }
}

fn open_state_dir(state_dir: &Path, create: bool) -> Result<NativeDirectory> {
    NativeDirectory::open(state_dir, create)
}

fn c_name(name: &str) -> Result<CString> {
    CString::new(name.as_bytes()).map_err(|_| Error::CorruptWal("invalid archive state filename"))
}

fn openat(
    directory: &File,
    name: &CStr,
    flags: libc::c_int,
    mode: libc::mode_t,
) -> std::io::Result<File> {
    // SAFETY: `directory` is an open directory descriptor and `name` is a
    // NUL-terminated single component retained for the duration of the call.
    let fd = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags, mode) };
    if fd < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        // SAFETY: a successful `openat` returns a new owned descriptor.
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}

fn mkdirat(directory: &File, name: &CStr) -> std::io::Result<()> {
    // SAFETY: `directory` remains open and `name` is a retained,
    // NUL-terminated single component for the duration of the call.
    if unsafe { libc::mkdirat(directory.as_raw_fd(), name.as_ptr(), 0o700) } != 0 {
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::AlreadyExists {
            return Err(error);
        }
    }
    Ok(())
}

fn statat(directory: &File, name: &CStr) -> std::io::Result<libc::stat> {
    // SAFETY: `libc::stat` is a C data record whose all-zero bit pattern is
    // valid; `fstatat` overwrites it on success.
    let mut stat = unsafe { std::mem::zeroed::<libc::stat>() };
    // SAFETY: `directory` remains open, `name` is NUL-terminated, and
    // `stat` points to writable storage initialized by successful `fstatat`.
    if unsafe {
        libc::fstatat(
            directory.as_raw_fd(),
            name.as_ptr(),
            &mut stat,
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } != 0
    {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(stat)
    }
}

fn read_named_file(state: &NativeDirectory, name: &str) -> Result<Option<File>> {
    let name = c_name(name)?;
    let file = match openat(
        &state.dir,
        &name,
        libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        0,
    ) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let metadata = file.metadata()?;
    if !metadata.file_type().is_file() || metadata.nlink() != 1 {
        return Err(Error::CorruptWal("archive watermark is not a regular file"));
    }
    revalidate_named_file(state, &name, &file, &metadata)?;
    Ok(Some(file))
}

fn read_watermark(state: &NativeDirectory, timeline: TimelineId) -> Result<WatermarkState> {
    let Some(mut file) = read_named_file(state, WATERMARK_FILE)? else {
        return Ok(empty_watermark(timeline));
    };
    let mut text = String::new();
    file.read_to_string(&mut text)?;
    decode_watermark(&text, timeline)
}

fn lock_watermark(state: &NativeDirectory) -> Result<File> {
    let name = c_name(WATERMARK_LOCK_FILE)?;
    let lock = openat(
        &state.dir,
        &name,
        libc::O_RDWR | libc::O_CREAT | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        0o600,
    )?;
    let metadata = lock.metadata()?;
    if !metadata.file_type().is_file() || metadata.nlink() != 1 {
        return Err(Error::CorruptWal(
            "archive watermark lock is not a regular file",
        ));
    }
    revalidate_named_file(state, &name, &lock, &metadata)?;
    // SAFETY: `lock` owns a valid descriptor. `flock` does not retain the
    // pointer state and the lock is released when this descriptor is dropped.
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(lock)
}

fn renameat(state: &NativeDirectory, old: &str, new: &str) -> Result<()> {
    state.revalidate()?;
    let old = c_name(old)?;
    let new = c_name(new)?;
    // SAFETY: both names are NUL-terminated single components and both
    // directory descriptors remain open for the complete operation.
    if unsafe {
        libc::renameat(
            state.dir.as_raw_fd(),
            old.as_ptr(),
            state.dir.as_raw_fd(),
            new.as_ptr(),
        )
    } != 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    state.revalidate()?;
    Ok(())
}

fn unlinkat(state: &NativeDirectory, name: &str) -> Result<()> {
    state.revalidate()?;
    let name = c_name(name)?;
    // SAFETY: `name` is a retained NUL-terminated component and `state`
    // remains an open directory descriptor for the complete call.
    if unsafe { libc::unlinkat(state.dir.as_raw_fd(), name.as_ptr(), 0) } != 0 {
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::NotFound {
            return Err(error.into());
        }
    }
    state.revalidate()?;
    Ok(())
}

fn recover_pending_watermark(
    state: &NativeDirectory,
    timeline: TimelineId,
) -> Result<WatermarkState> {
    let current = read_watermark(state, timeline)?;
    let Some(mut pending_file) = read_named_file(state, WATERMARK_PENDING_FILE)? else {
        return Ok(current);
    };
    let mut text = String::new();
    pending_file.read_to_string(&mut text)?;
    let pending = match decode_watermark(&text, timeline) {
        Ok(pending) => pending,
        Err(Error::CorruptWal(_)) => {
            // A crash can leave a created-but-not-fsynced intent containing
            // only a prefix. It has never become authority and is safe to
            // discard while holding the cross-process lock.
            drop(pending_file);
            unlinkat(state, WATERMARK_PENDING_FILE)?;
            state.dir.sync_all()?;
            return Ok(current);
        }
        Err(error) => return Err(error),
    };
    if pending.watermark.generation
        != current
            .watermark
            .generation
            .checked_add(1)
            .ok_or(Error::CorruptWal("archive watermark generation overflow"))?
        || pending.previous_lsn != current.watermark.archived_lsn
        || pending.watermark.archived_lsn <= pending.previous_lsn
    {
        return Err(Error::CorruptWal(
            "archive watermark intent conflicts with native state",
        ));
    }
    renameat(state, WATERMARK_PENDING_FILE, WATERMARK_FILE)?;
    state.dir.sync_all()?;
    Ok(pending)
}

fn write_watermark(state: &NativeDirectory, watermark: WatermarkState) -> Result<()> {
    let text = format!(
        "version={WATERMARK_VERSION}\ntimeline={}\nprevious_lsn={}\narchived_lsn={}\ngeneration={}\nreceipt_sha256={}\n",
        watermark.watermark.timeline.0,
        watermark.previous_lsn.0,
        watermark.watermark.archived_lsn.0,
        watermark.watermark.generation,
        encode_hex(&watermark.receipt_sha256)
    );
    {
        let name = c_name(WATERMARK_PENDING_FILE)?;
        let mut file = openat(
            &state.dir,
            &name,
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
    }
    state.revalidate()?;
    state.dir.sync_all()?;
    renameat(state, WATERMARK_PENDING_FILE, WATERMARK_FILE)?;
    state.dir.sync_all()?;
    state.revalidate()?;
    Ok(())
}

fn decode_watermark(text: &str, expected_timeline: TimelineId) -> Result<WatermarkState> {
    let mut version = None;
    let mut timeline = None;
    let mut previous_lsn = None;
    let mut archived_lsn = None;
    let mut generation = None;
    let mut receipt_sha256 = None;
    for line in text.lines() {
        let (key, value) = line
            .split_once('=')
            .ok_or(Error::CorruptWal("invalid archive watermark"))?;
        match key {
            "version" => version = value.parse::<u16>().ok(),
            "timeline" => timeline = value.parse::<u64>().ok().map(TimelineId),
            "previous_lsn" => previous_lsn = value.parse::<u64>().ok().map(Lsn),
            "archived_lsn" => archived_lsn = value.parse::<u64>().ok().map(Lsn),
            "generation" => generation = value.parse::<u64>().ok(),
            "receipt_sha256" => receipt_sha256 = decode_hex_32(value),
            _ => return Err(Error::CorruptWal("unknown archive watermark field")),
        }
    }
    if version != Some(WATERMARK_VERSION)
        || timeline != Some(expected_timeline)
        || receipt_sha256.is_none()
    {
        return Err(Error::CorruptWal("invalid archive watermark"));
    }
    Ok(WatermarkState {
        watermark: ArchiveWatermark {
            timeline: expected_timeline,
            archived_lsn: archived_lsn.ok_or(Error::CorruptWal("missing archived lsn"))?,
            generation: generation.ok_or(Error::CorruptWal("missing archive generation"))?,
        },
        previous_lsn: previous_lsn.ok_or(Error::CorruptWal("missing previous archived lsn"))?,
        receipt_sha256: receipt_sha256
            .ok_or(Error::CorruptWal("missing archive receipt digest"))?,
    })
}

fn encode_hex(bytes: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn decode_hex_32(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0_u8; 32];
    for (index, slot) in out.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(out)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WalRetentionHorizons {
    pub checkpoint_lsn: Lsn,
    pub replication_slot_lsn: Lsn,
    pub required_archive_lsn: Lsn,
}

impl WalRetentionHorizons {
    pub fn recycle_lsn(self) -> Lsn {
        self.checkpoint_lsn
            .min(self.replication_slot_lsn)
            .min(self.required_archive_lsn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_segment_rejects_path_replacement() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("00000000000000000001.wal");
        fs::write(&path, [1_u8; 128]).expect("segment");
        let directory = NativeDirectory::open(temp.path(), false).expect("native directory");
        let name = c_name("00000000000000000001.wal").expect("name");
        let file = openat(
            &directory.dir,
            &name,
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0,
        )
        .expect("retained segment");
        let metadata = file.metadata().expect("metadata");

        fs::rename(&path, temp.path().join("held.wal")).expect("hold original");
        fs::write(&path, [2_u8; 128]).expect("replacement");
        assert_eq!(
            revalidate_named_file(&directory, &name, &file, &metadata)
                .expect_err("replacement must fail"),
            Error::CorruptWal("wal segment identity changed while reading")
        );
    }

    #[test]
    fn retained_directory_rejects_ancestor_replacement() {
        let temp = tempfile::tempdir().expect("tempdir");
        let parent = temp.path().join("parent");
        let directory_path = parent.join("wal");
        fs::create_dir_all(&directory_path).expect("directory");
        let directory = NativeDirectory::open(&directory_path, false).expect("native directory");

        let held = temp.path().join("held-parent");
        fs::rename(&parent, &held).expect("hold original ancestry");
        fs::create_dir_all(&directory_path).expect("replacement ancestry");

        assert_eq!(
            directory
                .revalidate()
                .expect_err("ancestor replacement must fail"),
            Error::CorruptWal("native directory ancestry changed")
        );
    }

    #[test]
    fn native_directory_retains_current_and_filesystem_roots() {
        NativeDirectory::open(Path::new("."), false)
            .expect("current directory")
            .revalidate()
            .expect("current directory binding");
        NativeDirectory::open(Path::new("/"), false)
            .expect("filesystem root")
            .revalidate()
            .expect("filesystem root binding");
    }
}
