use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::format::{Lsn, TimelineId, WalSegmentNo};

pub const WAL_SEGMENT_EXT: &str = ".wal";
pub const WAL_SEAL_EXT: &str = ".seal";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SealedWalSegment {
    pub timeline: TimelineId,
    pub segment_no: WalSegmentNo,
    pub first_lsn: Lsn,
    pub last_lsn: Lsn,
    pub byte_len: u64,
    pub crc32: u32,
}

impl SealedWalSegment {
    pub fn seal_name(&self) -> String {
        format!("{:020}{}", self.segment_no.0, WAL_SEAL_EXT)
    }

    pub fn encode_text(&self) -> String {
        format!(
            "timeline={}\nsegment_no={}\nfirst_lsn={}\nlast_lsn={}\nbyte_len={}\ncrc32={:08x}\n",
            self.timeline.0,
            self.segment_no.0,
            self.first_lsn.0,
            self.last_lsn.0,
            self.byte_len,
            self.crc32
        )
    }

    pub fn decode_text(text: &str) -> Option<Self> {
        let mut timeline = None;
        let mut segment_no = None;
        let mut first_lsn = None;
        let mut last_lsn = None;
        let mut byte_len = None;
        let mut crc32 = None;
        for line in text.lines() {
            let (key, value) = line.split_once('=')?;
            match key {
                "timeline" => timeline = value.parse::<u64>().ok().map(TimelineId),
                "segment_no" => segment_no = value.parse::<u64>().ok().map(WalSegmentNo),
                "first_lsn" => first_lsn = value.parse::<u64>().ok().map(Lsn),
                "last_lsn" => last_lsn = value.parse::<u64>().ok().map(Lsn),
                "byte_len" => byte_len = value.parse::<u64>().ok(),
                "crc32" => crc32 = u32::from_str_radix(value.trim(), 16).ok(),
                _ => {}
            }
        }
        Some(Self {
            timeline: timeline?,
            segment_no: segment_no?,
            first_lsn: first_lsn?,
            last_lsn: last_lsn?,
            byte_len: byte_len?,
            crc32: crc32?,
        })
    }
}

pub fn segment_for_lsn(lsn: Lsn, segment_bytes: u64) -> WalSegmentNo {
    WalSegmentNo(lsn.0 / segment_bytes + 1)
}

pub fn offset_for_lsn(lsn: Lsn, segment_bytes: u64) -> u64 {
    lsn.0 % segment_bytes
}

pub fn segment_path(dir: &Path, segment: WalSegmentNo) -> PathBuf {
    dir.join(format!("{:020}{}", segment.0, WAL_SEGMENT_EXT))
}

pub fn seal_path(dir: &Path, segment: WalSegmentNo) -> PathBuf {
    dir.join(format!("{:020}{}", segment.0, WAL_SEAL_EXT))
}

pub fn parse_segment_name(name: &str) -> Option<WalSegmentNo> {
    let number = name.strip_suffix(WAL_SEGMENT_EXT)?;
    if number.len() != 20 || !number.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    number.parse().ok().map(WalSegmentNo)
}

pub fn parse_seal_name(name: &str) -> Option<WalSegmentNo> {
    let number = name.strip_suffix(WAL_SEAL_EXT)?;
    if number.len() != 20 || !number.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    number.parse().ok().map(WalSegmentNo)
}

pub fn format_segment_label(segment: WalSegmentNo) -> String {
    let mut label = String::new();
    let _ = write!(&mut label, "{:020}", segment.0);
    label
}
