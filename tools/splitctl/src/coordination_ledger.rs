use serde_json::{json, Value as JsonValue};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
};

const CUTOFF: &str = "2026-07-19T00:00:00Z";
const RELEASE_PREFIX_LINES: usize = 358;
const RELEASE_PREFIX_SHA256: &str =
    "1c10d3147122261aa77e1eec42f302481644e1bd432f4e7f443e07dbc7c58374";
// First uniquely matching entry after the compacted historical prefixes.
// Pinning it prevents later divergence from selecting a newer anchor.
const SYNC_ANCHOR_SHA256: &str = "10f8063ec5fc55c1e17606c946cf5c8108230be63d577bf749fe9e3fb047a956";
const MAX_LEDGER_BYTES: u64 = 16 * 1024 * 1024;
const MAX_ENTRY_BYTES: u64 = 64 * 1024;
const LEDGERS: [&str; 3] = ["DATA_SHARD_CHAT.md", "RELEASE_V10.md", "UPGRADE_CHAT.md"];

#[derive(Clone, Debug)]
struct Entry {
    digest: String,
    timestamp: String,
}

#[derive(Clone, Debug)]
struct LedgerEntries {
    name: &'static str,
    entries: Vec<Entry>,
    pre_anchor_entries: usize,
}

#[derive(Clone, Debug)]
struct Inspection {
    report: JsonValue,
    tips_aligned: bool,
    latest_timestamps: BTreeMap<&'static str, Option<String>>,
}

#[derive(Debug)]
struct LockedLedger {
    name: &'static str,
    path: PathBuf,
    file: File,
    metadata: fs::Metadata,
}

pub(crate) fn command(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut root = None;
    let mut entry_file = None;
    let mut receipt = None;
    let mut apply = false;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--root" => root = Some(PathBuf::from(iter.next().ok_or("--root needs a path")?)),
            "--entry-file" => {
                entry_file = Some(PathBuf::from(
                    iter.next().ok_or("--entry-file needs a path")?,
                ))
            }
            "--receipt" => {
                receipt = Some(PathBuf::from(iter.next().ok_or("--receipt needs a path")?))
            }
            "--apply" => apply = true,
            value => return Err(format!("unknown coordination-ledger argument: {value}").into()),
        }
    }
    let root = root.ok_or("coordination-ledger requires --root")?;
    if apply && entry_file.is_none() {
        return Err("coordination-ledger --apply requires --entry-file".into());
    }

    let expected = PrefixExpectation {
        line_count: RELEASE_PREFIX_LINES,
        sha256: RELEASE_PREFIX_SHA256,
        anchor_sha256: Some(SYNC_ANCHOR_SHA256),
    };
    let mut action = "status";
    let inspection = if let Some(entry_path) = entry_file {
        let entry = read_regular_bounded(&entry_path, MAX_ENTRY_BYTES, "coordination entry")?;
        let entry_timestamp = validate_entry(&entry)?;
        if apply {
            action = append_entry(&root, &entry, &entry_timestamp, &expected)?;
        } else {
            let before = inspect_root(&root, &expected)?;
            if !before.tips_aligned {
                return Err("coordination ledger tips must align before planning an append".into());
            }
            validate_monotonic_timestamp(&before, &entry_timestamp)?;
            action = if ledgers_end_with(&root, &entry)? {
                "already-present"
            } else {
                "would-append"
            };
        }
        inspect_root(&root, &expected)?
    } else {
        inspect_root(&root, &expected)?
    };

    let mut report = inspection.report;
    report["action"] = json!(action);
    report["applied"] = json!(apply && action == "appended");
    if let Some(path) = receipt {
        write_receipt(&path, &report)?;
    }
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[derive(Clone, Copy)]
struct PrefixExpectation<'a> {
    line_count: usize,
    sha256: &'a str,
    anchor_sha256: Option<&'a str>,
}

fn inspect_root(
    root: &Path,
    expected: &PrefixExpectation<'_>,
) -> Result<Inspection, Box<dyn std::error::Error>> {
    let mut contents = BTreeMap::new();
    for name in LEDGERS {
        contents.insert(
            name,
            read_regular_bounded(&root.join(name), MAX_LEDGER_BYTES, "coordination ledger")?,
        );
    }
    inspect_contents(&contents, expected)
}

fn inspect_contents(
    contents: &BTreeMap<&'static str, Vec<u8>>,
    expected: &PrefixExpectation<'_>,
) -> Result<Inspection, Box<dyn std::error::Error>> {
    let release = contents
        .get("RELEASE_V10.md")
        .ok_or("release coordination ledger is missing")?;
    let prefix_end = nth_line_end(release, expected.line_count)
        .ok_or("RELEASE_V10.md has fewer than the frozen prefix line count")?;
    let prefix_digest = sha256(&release[..prefix_end]);
    let prefix_valid = prefix_digest == expected.sha256;

    let release_entries = parse_entries(&release[prefix_end..])?;
    let release_entries = release_entries
        .into_iter()
        .filter(|entry| entry.timestamp.as_str() >= CUTOFF)
        .collect::<Vec<_>>();

    let mut blockers = Vec::new();
    if !prefix_valid {
        blockers.push(format!(
            "frozen release prefix digest mismatch: expected {}, found {prefix_digest}",
            expected.sha256
        ));
    }
    if release_entries.is_empty() {
        blockers.push("RELEASE_V10.md has no timestamped live entries".to_owned());
    }

    let mut parsed_ledgers = BTreeMap::new();
    parsed_ledgers.insert("RELEASE_V10.md", release_entries);
    for name in ["DATA_SHARD_CHAT.md", "UPGRADE_CHAT.md"] {
        parsed_ledgers.insert(
            name,
            parse_entries(contents.get(name).unwrap())?
                .into_iter()
                .filter(|entry| entry.timestamp.as_str() >= CUTOFF)
                .collect::<Vec<_>>(),
        );
    }
    let anchor_digest = expected.anchor_sha256.map(str::to_owned).or_else(|| {
        parsed_ledgers["RELEASE_V10.md"]
            .iter()
            .find(|candidate| {
                let positions = parsed_ledgers
                    .values()
                    .map(|entries| {
                        entries
                            .iter()
                            .enumerate()
                            .filter_map(|(index, entry)| {
                                (entry.digest == candidate.digest).then_some(index)
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();
                if positions.iter().any(|positions| positions.len() != 1) {
                    return false;
                }
                let suffixes = parsed_ledgers
                    .values()
                    .zip(positions)
                    .map(|(entries, positions)| &entries[positions[0]..])
                    .collect::<Vec<_>>();
                let shared = suffixes
                    .iter()
                    .map(|entries| entries.len())
                    .min()
                    .unwrap_or(0);
                (0..shared).all(|index| {
                    suffixes
                        .iter()
                        .skip(1)
                        .all(|entries| entries[index].digest == suffixes[0][index].digest)
                })
            })
            .map(|entry| entry.digest.clone())
    });
    if anchor_digest.is_none() {
        blockers
            .push("no unique synchronization anchor exists across all three ledgers".to_owned());
    }
    let mut ledgers = Vec::new();
    for name in LEDGERS {
        let parsed = parsed_ledgers.get(name).unwrap();
        let (entries, pre_anchor_entries) = if let Some(anchor) = &anchor_digest {
            let positions = parsed
                .iter()
                .enumerate()
                .filter_map(|(index, entry)| (entry.digest == *anchor).then_some(index))
                .collect::<Vec<_>>();
            if positions.len() != 1 {
                blockers.push(format!(
                    "{name} contains the synchronization anchor {} times",
                    positions.len()
                ));
                (Vec::new(), parsed.len())
            } else {
                let position = positions[0];
                (parsed[position..].to_vec(), position)
            }
        } else {
            (Vec::new(), parsed.len())
        };
        ledgers.push(LedgerEntries {
            name,
            entries,
            pre_anchor_entries,
        });
    }

    let max_len = ledgers
        .iter()
        .map(|ledger| ledger.entries.len())
        .max()
        .unwrap_or(0);
    let mut synchronized_entries = 0;
    for index in 0..max_len {
        let present = ledgers
            .iter()
            .filter_map(|ledger| ledger.entries.get(index))
            .collect::<Vec<_>>();
        if present.len() < ledgers.len() {
            break;
        }
        if present
            .iter()
            .skip(1)
            .all(|entry| entry.digest == present[0].digest)
        {
            synchronized_entries += 1;
        } else {
            blockers.push(format!(
                "ordered live-entry digests diverge at synchronized index {index}"
            ));
            break;
        }
    }

    if blockers.is_empty() {
        for ledger in &ledgers {
            for (index, entry) in ledger.entries.iter().enumerate() {
                let Some(reference) = ledgers[0].entries.get(index) else {
                    continue;
                };
                if index < synchronized_entries && entry.digest != reference.digest {
                    blockers.push(format!(
                        "{} diverges at synchronized index {index}",
                        ledger.name
                    ));
                    break;
                }
            }
        }
    }

    let status = if !blockers.is_empty() {
        "divergent"
    } else if ledgers.iter().all(|ledger| ledger.entries.len() == max_len) {
        "in_sync"
    } else {
        "lagging"
    };

    let mut missing_entry_count = 0;
    let mut latest_timestamps = BTreeMap::new();
    let ledger_reports = ledgers
        .iter()
        .map(|ledger| {
            let missing = max_len.saturating_sub(ledger.entries.len());
            missing_entry_count += missing;
            let latest = ledger.entries.last().map(|entry| entry.timestamp.clone());
            latest_timestamps.insert(ledger.name, latest.clone());
            json!({
                "name": ledger.name,
                "entry_count": ledger.entries.len(),
                "pre_anchor_entries": ledger.pre_anchor_entries,
                "missing_entries": missing,
                "latest_timestamp": latest,
                "latest_entry_sha256": ledger.entries.last().map(|entry| entry.digest.clone()),
            })
        })
        .collect::<Vec<_>>();
    let latest_digests = ledgers
        .iter()
        .filter_map(|ledger| ledger.entries.last().map(|entry| entry.digest.as_str()))
        .collect::<Vec<_>>();
    let tips_aligned = latest_digests.len() == ledgers.len()
        && latest_digests
            .iter()
            .skip(1)
            .all(|digest| *digest == latest_digests[0]);

    let report = json!({
        "schema_version": "jain.split.coordination-ledger/v1",
        "status": status,
        "cutoff": CUTOFF,
        "frozen_prefix": {
            "line_count": expected.line_count,
            "expected_sha256": expected.sha256,
            "actual_sha256": prefix_digest,
            "valid": prefix_valid,
        },
        "anchor_entry_sha256": anchor_digest,
        "anchor_timestamp": ledgers
            .first()
            .and_then(|ledger| ledger.entries.first())
            .map(|entry| entry.timestamp.clone()),
        "synchronized_entries": synchronized_entries,
        "missing_entry_count": missing_entry_count,
        "tips_aligned": tips_aligned,
        "blockers": blockers,
        "ledgers": ledger_reports,
    });
    Ok(Inspection {
        report,
        tips_aligned,
        latest_timestamps,
    })
}

fn parse_entries(bytes: &[u8]) -> Result<Vec<Entry>, Box<dyn std::error::Error>> {
    let text = std::str::from_utf8(bytes)?;
    let mut starts = Vec::new();
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let without_newline = line.strip_suffix('\n').unwrap_or(line);
        if let Some(timestamp) = entry_timestamp(without_newline) {
            starts.push((offset, timestamp));
        }
        offset += line.len();
    }
    if offset < text.len() {
        return Err("coordination parser did not consume the full ledger".into());
    }
    let mut entries = Vec::new();
    for (index, (start, timestamp)) in starts.iter().enumerate() {
        let mut end = starts
            .get(index + 1)
            .map(|(next, _)| *next)
            .unwrap_or(bytes.len());
        while end > *start && matches!(bytes[end - 1], b'\n' | b'\r') {
            end -= 1;
        }
        entries.push(Entry {
            digest: sha256(&bytes[*start..end]),
            timestamp: timestamp.clone(),
        });
    }
    Ok(entries)
}

fn entry_timestamp(line: &str) -> Option<String> {
    let mut candidate = line.trim_start();
    while let Some(rest) = candidate.strip_prefix('#') {
        candidate = rest.trim_start();
    }
    if let Some(rest) = candidate.strip_prefix("- ") {
        candidate = rest.trim_start();
    }
    if let Some(rest) = candidate.strip_prefix('`') {
        candidate = rest;
    }
    normalize_timestamp(candidate)
}

fn normalize_timestamp(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    if bytes.len() >= 20
        && ascii_date(&bytes[..10])
        && bytes[10] == b'T'
        && ascii_time(&bytes[11..19])
        && bytes[19] == b'Z'
    {
        return Some(value[..20].to_owned());
    }
    if bytes.len() >= 20
        && ascii_date(&bytes[..10])
        && bytes[10] == b' '
        && ascii_minute(&bytes[11..16])
        && &bytes[16..20] == b" UTC"
    {
        return Some(format!("{}T{}:00Z", &value[..10], &value[11..16]));
    }
    None
}

fn ascii_date(value: &[u8]) -> bool {
    value.len() == 10
        && value[4] == b'-'
        && value[7] == b'-'
        && value
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
}

fn ascii_time(value: &[u8]) -> bool {
    value.len() == 8
        && value[2] == b':'
        && value[5] == b':'
        && value
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 2 | 5) || byte.is_ascii_digit())
}

fn ascii_minute(value: &[u8]) -> bool {
    value.len() == 5
        && value[2] == b':'
        && value
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 2 || byte.is_ascii_digit())
}

fn validate_entry(bytes: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
    if bytes.is_empty() || bytes.len() as u64 > MAX_ENTRY_BYTES {
        return Err(format!("coordination entry must contain 1..={MAX_ENTRY_BYTES} bytes").into());
    }
    if !bytes.ends_with(b"\n") {
        return Err("coordination entry must end with a newline".into());
    }
    if bytes
        .iter()
        .any(|byte| byte.is_ascii_control() && !matches!(byte, b'\n' | b'\r' | b'\t'))
    {
        return Err("coordination entry contains a forbidden control byte".into());
    }
    let entries = parse_entries(bytes)?;
    if entries.len() != 1 {
        return Err("coordination entry must contain exactly one timestamped entry".into());
    }
    let text = std::str::from_utf8(bytes)?;
    let first_line = text
        .lines()
        .position(|line| entry_timestamp(line).is_some());
    let first_line = first_line.ok_or("coordination entry has no timestamped first line")?;
    if first_line > 1 || text.lines().take(first_line).any(|line| !line.is_empty()) {
        return Err(
            "coordination entry may contain at most one leading blank line before its timestamp"
                .into(),
        );
    }
    let timestamp = entries[0].timestamp.clone();
    if timestamp.as_str() < CUTOFF {
        return Err("coordination entry timestamp predates the live-ledger cutoff".into());
    }
    Ok(timestamp)
}

fn append_entry(
    root: &Path,
    entry: &[u8],
    entry_timestamp: &str,
    expected: &PrefixExpectation<'_>,
) -> Result<&'static str, Box<dyn std::error::Error>> {
    let mut locked = lock_ledgers(root)?;
    let mut contents = BTreeMap::new();
    for ledger in &mut locked {
        ledger.file.seek(SeekFrom::Start(0))?;
        let mut bytes = Vec::with_capacity(ledger.metadata.len() as usize);
        (&mut ledger.file)
            .take(MAX_LEDGER_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_LEDGER_BYTES {
            return Err(format!("{} exceeds the ledger byte limit", ledger.name).into());
        }
        contents.insert(ledger.name, bytes);
    }
    let before = inspect_contents(&contents, expected)?;
    if !before.tips_aligned {
        return Err("coordination ledger tips must align before appending".into());
    }

    let tail_matches = locked
        .iter()
        .map(|ledger| {
            contents
                .get(ledger.name)
                .is_some_and(|bytes| bytes.ends_with(entry))
        })
        .collect::<Vec<_>>();
    if tail_matches.iter().all(|matches| *matches) {
        unlock_ledgers(&locked)?;
        return Ok("already-present");
    }
    if tail_matches.iter().any(|matches| *matches) {
        return Err("coordination entry is present in only some ledgers".into());
    }
    validate_monotonic_timestamp(&before, entry_timestamp)?;

    let original_lengths = locked
        .iter()
        .map(|ledger| ledger.metadata.len())
        .collect::<Vec<_>>();
    let write_result = (|| -> Result<(), Box<dyn std::error::Error>> {
        for ledger in &mut locked {
            ledger.file.seek(SeekFrom::End(0))?;
            ledger.file.write_all(entry)?;
            ledger.file.sync_all()?;
        }
        Ok(())
    })();
    if let Err(error) = write_result {
        for (ledger, length) in locked.iter_mut().zip(original_lengths) {
            let _ = ledger.file.set_len(length);
            let _ = ledger.file.sync_all();
        }
        return Err(error);
    }

    let mut after_contents = BTreeMap::new();
    for ledger in &mut locked {
        let path_metadata = fs::symlink_metadata(&ledger.path)?;
        let handle_metadata = ledger.file.metadata()?;
        if !same_inode(&ledger.metadata, &handle_metadata)
            || !same_inode(&handle_metadata, &path_metadata)
            || !handle_metadata.file_type().is_file()
        {
            return Err(format!("{} changed identity while appending", ledger.name).into());
        }
        ledger.file.seek(SeekFrom::Start(0))?;
        let mut bytes = Vec::with_capacity(handle_metadata.len() as usize);
        ledger.file.read_to_end(&mut bytes)?;
        if !bytes.ends_with(entry) {
            return Err(format!("{} append readback differs", ledger.name).into());
        }
        after_contents.insert(ledger.name, bytes);
    }
    let after = inspect_contents(&after_contents, expected)?;
    if !after.tips_aligned {
        return Err("coordination ledger tips differ after append readback".into());
    }
    unlock_ledgers(&locked)?;
    Ok("appended")
}

fn lock_ledgers(root: &Path) -> Result<Vec<LockedLedger>, Box<dyn std::error::Error>> {
    let mut locked = Vec::new();
    for name in LEDGERS {
        let path = root.join(name);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&path)?;
        let metadata = file.metadata()?;
        let path_metadata = fs::symlink_metadata(&path)?;
        if !metadata.file_type().is_file()
            || metadata.nlink() != 1
            || metadata.len() > MAX_LEDGER_BYTES
            || !same_inode(&metadata, &path_metadata)
        {
            return Err(format!("{name} is not a safe single-link regular ledger").into());
        }
        // SAFETY: `file` owns a valid descriptor for the lifetime of the lock.
        let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if result != 0 {
            return Err(format!("coordination ledger lock contention on {name}").into());
        }
        locked.push(LockedLedger {
            name,
            path,
            file,
            metadata,
        });
    }
    Ok(locked)
}

fn unlock_ledgers(locked: &[LockedLedger]) -> Result<(), Box<dyn std::error::Error>> {
    for ledger in locked.iter().rev() {
        // SAFETY: every ledger owns a live descriptor until this function returns.
        if unsafe { libc::flock(ledger.file.as_raw_fd(), libc::LOCK_UN) } != 0 {
            return Err(
                format!("cannot release coordination ledger lock on {}", ledger.name).into(),
            );
        }
    }
    Ok(())
}

fn validate_monotonic_timestamp(
    inspection: &Inspection,
    timestamp: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    for (ledger, latest) in &inspection.latest_timestamps {
        if latest.as_deref().is_some_and(|latest| timestamp < latest) {
            return Err(format!(
                "coordination entry timestamp {timestamp} predates {ledger} latest {latest:?}"
            )
            .into());
        }
    }
    Ok(())
}

fn ledgers_end_with(root: &Path, entry: &[u8]) -> Result<bool, Box<dyn std::error::Error>> {
    let matches = LEDGERS
        .iter()
        .map(|name| {
            read_regular_bounded(&root.join(name), MAX_LEDGER_BYTES, "coordination ledger")
                .map(|bytes| bytes.ends_with(entry))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if matches.iter().any(|value| *value) && !matches.iter().all(|value| *value) {
        return Err("coordination entry is present in only some ledgers".into());
    }
    Ok(matches.iter().all(|value| *value))
}

fn read_regular_bounded(
    path: &Path,
    limit: u64,
    label: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let before = file.metadata()?;
    let path_before = fs::symlink_metadata(path)?;
    if !before.file_type().is_file()
        || before.nlink() != 1
        || before.len() > limit
        || !same_inode(&before, &path_before)
    {
        return Err(format!("{label} is not a safe bounded single-link regular file").into());
    }
    let mut bytes = Vec::with_capacity(before.len() as usize);
    (&mut file).take(limit + 1).read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    let path_after = fs::symlink_metadata(path)?;
    if bytes.len() as u64 > limit
        || bytes.len() as u64 != before.len()
        || !same_metadata(&before, &after)
        || !same_metadata(&before, &path_after)
    {
        return Err(format!("{label} changed while being read").into());
    }
    Ok(bytes)
}

fn write_receipt(path: &Path, report: &JsonValue) -> Result<(), Box<dyn std::error::Error>> {
    if path.exists() || path.is_symlink() {
        return Err(format!("receipt already exists: {}", path.display()).into());
    }
    let parent = path.parent().ok_or("receipt path has no parent")?;
    let parent_metadata = fs::symlink_metadata(parent)?;
    if !parent_metadata.file_type().is_dir() {
        return Err("receipt parent is not a physical directory".into());
    }
    let bytes = serde_json::to_vec_pretty(report)?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o644)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    file.write_all(&bytes)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

fn nth_line_end(bytes: &[u8], lines: usize) -> Option<usize> {
    let mut seen = 0;
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' {
            seen += 1;
            if seen == lines {
                return Some(index + 1);
            }
        }
    }
    None
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn same_inode(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.dev() == right.dev() && left.ino() == right.ino()
}

fn same_metadata(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    same_inode(left, right)
        && left.file_type().is_file()
        && right.file_type().is_file()
        && left.mode() == right.mode()
        && left.nlink() == right.nlink()
        && left.uid() == right.uid()
        && left.gid() == right.gid()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        os::unix::fs::symlink,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT_ID: AtomicU64 = AtomicU64::new(1);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "splitctl-coordination-{label}-{}-{}",
                std::process::id(),
                NEXT_ID.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn fixture_prefix() -> Vec<u8> {
        (1..=RELEASE_PREFIX_LINES)
            .map(|line| format!("frozen {line}\n"))
            .collect::<String>()
            .into_bytes()
    }

    fn fixture(
        root: &Path,
        release_entries: &[&str],
        peer_entries: &[&str],
    ) -> PrefixExpectation<'static> {
        let prefix = fixture_prefix();
        let expected = PrefixExpectation {
            line_count: RELEASE_PREFIX_LINES,
            sha256: Box::leak(sha256(&prefix).into_boxed_str()),
            anchor_sha256: None,
        };
        let release = [prefix, release_entries.concat().into_bytes()].concat();
        fs::write(root.join("RELEASE_V10.md"), release).unwrap();
        for name in ["UPGRADE_CHAT.md", "DATA_SHARD_CHAT.md"] {
            fs::write(
                root.join(name),
                format!("historical peer preface\n{}", peer_entries.concat()),
            )
            .unwrap();
        }
        expected
    }

    const ENTRY_A: &str = "\n2026-07-23T17:49:10Z | claim a\n";
    const ENTRY_B: &str = "\n2026-07-25T21:00:00Z | claim b\n";
    const ENTRY_C: &str = "\n2026-07-25T22:00:00Z | claim c\n";

    #[test]
    fn synchronized_lagging_and_divergent_ledgers_are_distinguished() {
        let temp = TestDir::new("states");
        let expected = fixture(&temp.0, &[ENTRY_A, ENTRY_B], &[ENTRY_A, ENTRY_B]);
        let sync = inspect_root(&temp.0, &expected).unwrap();
        assert_eq!(sync.report["status"], "in_sync");

        fs::write(
            temp.0.join("RELEASE_V10.md"),
            [fixture_prefix(), ENTRY_A.as_bytes().to_vec()].concat(),
        )
        .unwrap();
        let lagging = inspect_root(&temp.0, &expected).unwrap();
        assert_eq!(lagging.report["status"], "lagging");
        assert_eq!(lagging.report["missing_entry_count"], 1);

        let divergent = "\n2026-07-25T21:00:00Z | conflicting b\n";
        fs::write(
            temp.0.join("RELEASE_V10.md"),
            [
                fixture_prefix(),
                ENTRY_A.as_bytes().to_vec(),
                divergent.as_bytes().to_vec(),
            ]
            .concat(),
        )
        .unwrap();
        assert_eq!(
            inspect_root(&temp.0, &expected).unwrap().report["status"],
            "divergent"
        );
    }

    #[test]
    fn frozen_prefix_tampering_is_divergent() {
        let temp = TestDir::new("prefix");
        let expected = fixture(&temp.0, &[ENTRY_A], &[ENTRY_A]);
        let path = temp.0.join("RELEASE_V10.md");
        let mut bytes = fs::read(&path).unwrap();
        bytes[0] ^= 1;
        fs::write(path, bytes).unwrap();
        let result = inspect_root(&temp.0, &expected).unwrap();
        assert_eq!(result.report["status"], "divergent");
        assert_eq!(result.report["frozen_prefix"]["valid"], false);
    }

    #[test]
    fn historical_gap_with_aligned_tips_remains_appendable_and_divergent() {
        let temp = TestDir::new("aligned-divergence");
        let mut expected = fixture(&temp.0, &[ENTRY_A, ENTRY_C], &[ENTRY_A, ENTRY_B, ENTRY_C]);
        let anchor = parse_entries(ENTRY_A.as_bytes()).unwrap()[0].digest.clone();
        expected.anchor_sha256 = Some(Box::leak(anchor.into_boxed_str()));
        let before = inspect_root(&temp.0, &expected).unwrap();
        assert_eq!(before.report["status"], "divergent");
        assert!(before.tips_aligned);

        let entry_d = b"\n2026-07-25T23:00:00Z | claim d\n";
        assert_eq!(
            append_entry(&temp.0, entry_d, "2026-07-25T23:00:00Z", &expected).unwrap(),
            "appended"
        );
        let after = inspect_root(&temp.0, &expected).unwrap();
        assert_eq!(after.report["status"], "divergent");
        assert!(after.tips_aligned);
    }

    #[test]
    fn entry_validation_rejects_malformed_and_oversized_inputs() {
        assert!(validate_entry(b"missing timestamp\n").is_err());
        assert!(validate_entry(b"2026-07-25T22:00:00Z | no newline").is_err());
        let oversized = vec![b'a'; MAX_ENTRY_BYTES as usize + 1];
        assert!(validate_entry(&oversized).is_err());
        assert!(validate_entry(ENTRY_C.as_bytes()).is_ok());
    }

    #[test]
    fn apply_is_exact_and_idempotent() {
        let temp = TestDir::new("append");
        let expected = fixture(&temp.0, &[ENTRY_A, ENTRY_B], &[ENTRY_A, ENTRY_B]);
        let before = LEDGERS
            .iter()
            .map(|name| (name, fs::read(temp.0.join(name)).unwrap()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            append_entry(
                &temp.0,
                ENTRY_C.as_bytes(),
                "2026-07-25T22:00:00Z",
                &expected
            )
            .unwrap(),
            "appended"
        );
        for name in LEDGERS {
            let bytes = fs::read(temp.0.join(name)).unwrap();
            assert_eq!(&bytes[before[&name].len()..], ENTRY_C.as_bytes(), "{name}");
        }
        assert_eq!(
            append_entry(
                &temp.0,
                ENTRY_C.as_bytes(),
                "2026-07-25T22:00:00Z",
                &expected
            )
            .unwrap(),
            "already-present"
        );
    }

    #[test]
    fn dry_run_checks_do_not_change_ledger_bytes() {
        let temp = TestDir::new("dry-run");
        let expected = fixture(&temp.0, &[ENTRY_A, ENTRY_B], &[ENTRY_A, ENTRY_B]);
        let before = LEDGERS
            .iter()
            .map(|name| (*name, fs::read(temp.0.join(name)).unwrap()))
            .collect::<BTreeMap<_, _>>();
        let inspection = inspect_root(&temp.0, &expected).unwrap();
        assert!(inspection.tips_aligned);
        validate_monotonic_timestamp(&inspection, "2026-07-25T22:00:00Z").unwrap();
        assert!(!ledgers_end_with(&temp.0, ENTRY_C.as_bytes()).unwrap());
        for name in LEDGERS {
            assert_eq!(fs::read(temp.0.join(name)).unwrap(), before[name]);
        }
    }

    #[test]
    fn symlink_and_lock_contention_are_rejected() {
        let temp = TestDir::new("guards");
        let expected = fixture(&temp.0, &[ENTRY_A], &[ENTRY_A]);
        let upgrade = temp.0.join("UPGRADE_CHAT.md");
        let target = temp.0.join("upgrade-target");
        fs::rename(&upgrade, &target).unwrap();
        symlink(&target, &upgrade).unwrap();
        assert!(inspect_root(&temp.0, &expected).is_err());
        fs::remove_file(&upgrade).unwrap();
        fs::rename(&target, &upgrade).unwrap();

        let held = OpenOptions::new()
            .read(true)
            .write(true)
            .open(temp.0.join(LEDGERS[0]))
            .unwrap();
        // SAFETY: `held` owns a live descriptor throughout the assertion.
        assert_eq!(unsafe { libc::flock(held.as_raw_fd(), libc::LOCK_EX) }, 0);
        let result = append_entry(
            &temp.0,
            ENTRY_B.as_bytes(),
            "2026-07-25T21:00:00Z",
            &expected,
        );
        assert!(result.unwrap_err().to_string().contains("lock contention"));
    }
}
