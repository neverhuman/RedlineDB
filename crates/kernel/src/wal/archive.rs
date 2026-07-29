use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::format::{Lsn, TimelineId};
use crate::{Error, Result};

use super::{offset_for_lsn, segment_for_lsn, segment_path};

pub const ARCHIVE_SEAL_BYTES: u64 = 16 * 1024 * 1024;
pub const ARCHIVE_SEAL_AFTER_MS: u64 = 5_000;
pub const ARCHIVE_LAG_ALERT_AFTER_MS: u64 = 30_000;

const WATERMARK_FILE: &str = "archive.watermark";
const WATERMARK_VERSION: u16 = 1;

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
            || self.byte_len > self.end_lsn.0.saturating_sub(self.start_lsn.0)
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

    let wal_dir = wal_dir.as_ref();
    let mut cursor = start_lsn;
    let address_bytes =
        usize::try_from(durable_lsn.0.saturating_sub(start_lsn.0)).unwrap_or(usize::MAX);
    let mut bytes = Vec::with_capacity(max_bytes.min(address_bytes));
    while cursor < durable_lsn && bytes.len() < max_bytes {
        let segment = segment_for_lsn(cursor, segment_bytes);
        let path = segment_path(wal_dir, segment);
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.file_type().is_file() {
            return Err(Error::CorruptWal("wal segment is not a regular file"));
        }
        let mut file = File::open(&path)?;
        let file_len = metadata.len();
        let offset = offset_for_lsn(cursor, segment_bytes);
        if offset > file_len {
            return Err(Error::CorruptWal("wal range starts beyond segment bytes"));
        }
        if offset == file_len {
            cursor = next_segment_lsn(segment.0, segment_bytes)?;
            continue;
        }

        let address_remaining = durable_lsn.0.saturating_sub(cursor.0);
        let available = file_len.saturating_sub(offset);
        let capacity = (max_bytes - bytes.len()) as u64;
        let take = available.min(address_remaining).min(capacity);
        file.seek(SeekFrom::Start(offset))?;
        let start_len = bytes.len();
        bytes.resize(start_len + take as usize, 0);
        file.read_exact(&mut bytes[start_len..])?;
        cursor = Lsn(cursor.0.saturating_add(take));

        if take == available && cursor < durable_lsn && bytes.len() < max_bytes {
            cursor = next_segment_lsn(segment.0, segment_bytes)?;
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

fn next_segment_lsn(segment: u64, segment_bytes: u64) -> Result<Lsn> {
    segment
        .checked_mul(segment_bytes)
        .map(Lsn)
        .ok_or(Error::CorruptWal("wal segment address overflow"))
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

pub fn archive_watermark(
    state_dir: impl AsRef<Path>,
    timeline: TimelineId,
) -> Result<ArchiveWatermark> {
    if timeline == TimelineId::ZERO {
        return Err(Error::CorruptWal("archive timeline must be nonzero"));
    }
    let path = watermark_path(state_dir.as_ref())?;
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ArchiveWatermark {
                timeline,
                archived_lsn: Lsn::ZERO,
                generation: 0,
            });
        }
        Err(error) => return Err(error.into()),
    };
    if !metadata.file_type().is_file() {
        return Err(Error::CorruptWal("archive watermark is not a regular file"));
    }
    decode_watermark(&fs::read_to_string(path)?, timeline)
}

pub fn advance_archive_watermark(
    state_dir: impl AsRef<Path>,
    receipt: &VerifiedArchiveReceipt,
) -> Result<ArchiveWatermark> {
    let state_dir = state_dir.as_ref();
    let current = archive_watermark(state_dir, receipt.range.timeline)?;
    if current.archived_lsn != receipt.range.start_lsn {
        return Err(Error::CorruptWal("archive receipt is not contiguous"));
    }
    let next = ArchiveWatermark {
        timeline: current.timeline,
        archived_lsn: receipt.range.end_lsn,
        generation: current
            .generation
            .checked_add(1)
            .ok_or(Error::CorruptWal("archive watermark generation overflow"))?,
    };
    write_watermark(state_dir, next, receipt.receipt_sha256)?;
    Ok(next)
}

fn watermark_path(state_dir: &Path) -> Result<PathBuf> {
    if state_dir.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::CurDir | Component::Prefix(_)
        )
    }) {
        return Err(Error::CorruptWal("invalid archive state directory"));
    }
    Ok(state_dir.join(WATERMARK_FILE))
}

fn write_watermark(
    state_dir: &Path,
    watermark: ArchiveWatermark,
    receipt_sha256: [u8; 32],
) -> Result<()> {
    fs::create_dir_all(state_dir)?;
    let state_metadata = fs::symlink_metadata(state_dir)?;
    if !state_metadata.file_type().is_dir() {
        return Err(Error::CorruptWal(
            "archive state root is not a native directory",
        ));
    }
    let path = watermark_path(state_dir)?;
    let tmp = state_dir.join(format!("{WATERMARK_FILE}.{}.tmp", watermark.generation));
    let text = format!(
        "version={WATERMARK_VERSION}\ntimeline={}\narchived_lsn={}\ngeneration={}\nreceipt_sha256={}\n",
        watermark.timeline.0,
        watermark.archived_lsn.0,
        watermark.generation,
        encode_hex(&receipt_sha256)
    );
    {
        let mut file = OpenOptions::new().create_new(true).write(true).open(&tmp)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
    }
    fs::rename(&tmp, &path)?;
    File::open(state_dir)?.sync_all()?;
    Ok(())
}

fn decode_watermark(text: &str, expected_timeline: TimelineId) -> Result<ArchiveWatermark> {
    let mut version = None;
    let mut timeline = None;
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
    Ok(ArchiveWatermark {
        timeline: expected_timeline,
        archived_lsn: archived_lsn.ok_or(Error::CorruptWal("missing archived lsn"))?,
        generation: generation.ok_or(Error::CorruptWal("missing archive generation"))?,
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
