// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use crate::archive_metadata::ArchiveMetadata;
use crate::central_directory::parse_header;
use crate::format_error::FormatError;
use crate::issue::{Directory, IssueLog};
use crate::source::Source;
use crate::{Checksum, Entry, EntryIssue};

#[cfg(test)]
mod tests;

/// The most bytes from the end of the file that recovery scans for directory records.
pub const MAX_RECOVERY_SCAN: u64 = 4 << 30;

/// The most Entries recovery admits.
pub const MAX_RECOVERED_ENTRIES: usize = 1 << 24;

const MAX_CANDIDATES: u64 = 1 << 24;
const WORK_PER_WINDOW_BYTE: u64 = 2;
const CHUNK_LEN: usize = 4 << 20;
const CENTRAL_SIGNATURE: [u8; 4] = 0x0201_4B50_u32.to_le_bytes();
const CENTRAL_HEADER_LEN: usize = 46;
const MAX_HEADER_LEN: usize = CENTRAL_HEADER_LEN + 3 * u16::MAX as usize;
const NAME_LEN_AT: usize = 28;
const LOCAL_OFFSET_AT: usize = 42;
const LOCAL_SIGNATURE: [u8; 4] = 0x0403_4B50_u32.to_le_bytes();
const LOCAL_CIG_SIGNATURE: [u8; 4] = 0x1403_4B50_u32.to_le_bytes();
const LOCAL_HEADER_LEN: usize = 30;
const LOCAL_NAME_AT: usize = 26;

/// Rebuilds the Entries from the central-directory records found by scanning `source` once from
/// front to back, from `hint` (the directory offset a valid end record gave) when that lies in the
/// last [`MAX_RECOVERY_SCAN`] bytes, else from the start of those.
///
/// A record counts only if it parses, if the local header at its offset lies before it and
/// carries the same name, and if the Entry limit holds. Whatever else is in the window is skipped
/// byte by byte, so one damaged record costs only itself. The work is capped by a byte budget of
/// twice the window and by the number of signature hits; reaching a cap is listed as an issue.
/// Returns `None` when no record qualifies.
pub(crate) fn recover(
    source: &Source,
    hint: Option<u64>,
) -> Result<Option<Directory>, FormatError> {
    let floor = source.len().saturating_sub(MAX_RECOVERY_SCAN);
    if let Some(hint) = hint.filter(|&hint| hint > floor && hint < source.len())
        && let Some(found) = scan(source, hint)?.into_directory()
    {
        return Ok(Some(found));
    }
    Ok(scan(source, floor)?.into_directory())
}

struct Scan {
    source_len: u64,
    entries: Vec<Entry>,
    log: IssueLog,
    p4k_checksums: bool,
    cut: bool,
    reads: u64,
}

impl Scan {
    fn into_directory(self) -> Option<Directory> {
        let Self {
            mut entries,
            mut log,
            p4k_checksums,
            source_len,
            cut,
            ..
        } = self;
        if entries.is_empty() {
            return None;
        }
        if !p4k_checksums {
            for entry in &mut entries {
                if let Some(value) = entry.crc32c() {
                    entry.checksum = Checksum::Crc32(value);
                }
            }
        }
        if cut {
            log.record_first(EntryIssue::RecoveryLimit);
        }
        log.record_first(EntryIssue::Recovered);
        Some(Directory {
            entries,
            log,
            installation_flags: None,
            metadata: ArchiveMetadata::recovered(source_len),
        })
    }
}

fn scan(source: &Source, start: u64) -> Result<Scan, FormatError> {
    let end = source.len();
    let mut window = Window::new(source);
    let mut pos = start;
    let mut budget = (end - start).saturating_mul(WORK_PER_WINDOW_BYTE);
    let mut found = Scan {
        source_len: source.len(),
        entries: Vec::new(),
        log: IssueLog::default(),
        p4k_checksums: false,
        cut: false,
        reads: 0,
    };
    let mut candidates = 0_u64;
    while pos + CENTRAL_HEADER_LEN as u64 <= end {
        if found.entries.len() >= MAX_RECOVERED_ENTRIES {
            found.cut = true;
            break;
        }
        let chunk = window.get(pos, CENTRAL_HEADER_LEN)?;
        let Some(hit) = find_signature(chunk) else {
            pos += chunk
                .len()
                .saturating_sub(CENTRAL_SIGNATURE.len() - 1)
                .max(1) as u64;
            continue;
        };
        let candidate = pos + hit as u64;
        candidates += 1;
        if candidates > MAX_CANDIDATES {
            found.cut = true;
            break;
        }
        match accept(&mut window, candidate, found.entries.len(), &mut budget)? {
            Outcome::Found(record) => {
                pos = candidate + record.len;
                found.p4k_checksums |= record.p4k;
                found.log.absorb(record.log);
                found.entries.push(record.entry);
            }
            Outcome::Rejected => pos = candidate + 1,
            Outcome::OverBudget => {
                found.cut = true;
                break;
            }
        }
    }
    found.reads = window.reads;
    Ok(found)
}

struct Found {
    entry: Entry,
    len: u64,
    p4k: bool,
    log: IssueLog,
}

enum Outcome {
    Found(Box<Found>),
    Rejected,
    OverBudget,
}

struct LocalStart {
    data_start: u64,
    cig: bool,
}

fn charge(budget: &mut u64, cost: usize) -> bool {
    budget
        .checked_sub(cost as u64)
        .map(|left| *budget = left)
        .is_some()
}

fn accept(
    window: &mut Window<'_>,
    candidate: u64,
    index: usize,
    budget: &mut u64,
) -> Result<Outcome, FormatError> {
    let source = window.source;
    let bytes = window.get(candidate, MAX_HEADER_LEN)?;
    if !charge(budget, CENTRAL_HEADER_LEN) {
        return Ok(Outcome::OverBudget);
    }
    let Some(fixed) = bytes.first_chunk::<CENTRAL_HEADER_LEN>() else {
        return Ok(Outcome::Rejected);
    };
    let name_len = usize::from(u16_at(fixed, NAME_LEN_AT));
    let total = CENTRAL_HEADER_LEN
        + name_len
        + usize::from(u16_at(fixed, NAME_LEN_AT + 2))
        + usize::from(u16_at(fixed, NAME_LEN_AT + 4));
    let Some(record) = bytes.get(..total) else {
        return Ok(Outcome::Rejected);
    };
    let local_offset = u32_at(fixed, LOCAL_OFFSET_AT);
    let mut local = None;
    if local_offset != u32::MAX {
        let Some(name) = record.get(CENTRAL_HEADER_LEN..CENTRAL_HEADER_LEN + name_len) else {
            return Ok(Outcome::Rejected);
        };
        match check_local(source, u64::from(local_offset), name, candidate, budget)? {
            Checked::Found(start) => local = Some(start),
            Checked::Rejected => return Ok(Outcome::Rejected),
            Checked::OverBudget => return Ok(Outcome::OverBudget),
        }
    }
    finish(source, record, candidate, index, (local, budget))
}

fn finish(
    source: &Source,
    record: &[u8],
    candidate: u64,
    index: usize,
    (local, budget): (Option<LocalStart>, &mut u64),
) -> Result<Outcome, FormatError> {
    let total = record.len();
    if !charge(budget, total) {
        return Ok(Outcome::OverBudget);
    }
    let mut log = IssueLog::default();
    let Ok(header) = parse_header(record, candidate, index, &mut log, source.len()) else {
        return Ok(Outcome::Rejected);
    };
    let local = match local {
        Some(start) => start,
        None => match check_local(
            source,
            header.entry.data_offset,
            header.name,
            candidate,
            budget,
        )? {
            Checked::Found(start) => start,
            Checked::Rejected => return Ok(Outcome::Rejected),
            Checked::OverBudget => return Ok(Outcome::OverBudget),
        },
    };
    let (entry, len) = (header.entry, header.len as u64);
    if local
        .data_start
        .checked_add(entry.compressed_size)
        .is_none_or(|data_end| data_end > candidate)
    {
        log.record(index, EntryIssue::DataOutsideFile);
    }
    Ok(Outcome::Found(Box::new(Found {
        entry,
        len,
        p4k: header.zip64_block || local.cig,
        log,
    })))
}

enum Checked {
    Found(LocalStart),
    Rejected,
    OverBudget,
}

/// Checks that the local header at `offset` lies completely before `candidate`, has a local
/// signature and carries `name`.
fn check_local(
    source: &Source,
    offset: u64,
    name: &[u8],
    candidate: u64,
    budget: &mut u64,
) -> Result<Checked, FormatError> {
    let fits = offset
        .checked_add((LOCAL_HEADER_LEN + name.len()) as u64)
        .is_some_and(|local_end| local_end <= candidate);
    if !fits {
        return Ok(Checked::Rejected);
    }
    if !charge(budget, LOCAL_HEADER_LEN + name.len()) {
        return Ok(Checked::OverBudget);
    }
    Ok(local_data_start(source, offset, name)?.map_or(Checked::Rejected, Checked::Found))
}

fn u16_at(bytes: &[u8; CENTRAL_HEADER_LEN], at: usize) -> u16 {
    bytes
        .get(at..)
        .and_then(|rest| rest.first_chunk::<2>())
        .map_or(0, |&pair| u16::from_le_bytes(pair))
}

fn u32_at(bytes: &[u8; CENTRAL_HEADER_LEN], at: usize) -> u32 {
    bytes
        .get(at..)
        .and_then(|rest| rest.first_chunk::<4>())
        .map_or(0, |&quad| u32::from_le_bytes(quad))
}

/// Returns where the data begins after the local header at `offset`, or `None` when that is not
/// a local header carrying `name`. The caller has checked that header and name lie in the source.
fn local_data_start(
    source: &Source,
    offset: u64,
    name: &[u8],
) -> Result<Option<LocalStart>, FormatError> {
    let mut fixed = [0_u8; LOCAL_HEADER_LEN];
    if let Err(error) = source.read_into(offset, &mut fixed) {
        return unless_eof(error);
    }
    let signature = fixed.first_chunk::<4>();
    let cig = signature == Some(&LOCAL_CIG_SIGNATURE);
    if !cig && signature != Some(&LOCAL_SIGNATURE) {
        return Ok(None);
    }
    let Some((name_len, extra_len)) = local_lengths(&fixed) else {
        return Ok(None);
    };
    if usize::from(name_len) != name.len() {
        return Ok(None);
    }
    let mut stored = vec![0_u8; name.len()];
    if let Err(error) = source.read_into(offset + LOCAL_HEADER_LEN as u64, &mut stored) {
        return unless_eof(error);
    }
    Ok((stored == name).then(|| LocalStart {
        data_start: offset + LOCAL_HEADER_LEN as u64 + u64::from(name_len) + u64::from(extra_len),
        cig,
    }))
}

fn local_lengths(fixed: &[u8; LOCAL_HEADER_LEN]) -> Option<(u16, u16)> {
    let lengths = fixed.get(LOCAL_NAME_AT..)?.first_chunk::<4>()?;
    let [name_low, name_high, extra_low, extra_high] = *lengths;
    Some((
        u16::from_le_bytes([name_low, name_high]),
        u16::from_le_bytes([extra_low, extra_high]),
    ))
}

fn unless_eof<T>(error: io::Error) -> Result<Option<T>, FormatError> {
    if error.kind() == io::ErrorKind::UnexpectedEof {
        Ok(None)
    } else {
        Err(error.into())
    }
}

fn find_signature(chunk: &[u8]) -> Option<usize> {
    let mut from = 0;
    while let Some(found) = chunk.get(from..)?.iter().position(|&byte| byte == b'P') {
        let at = from + found;
        if chunk.get(at..at + CENTRAL_SIGNATURE.len()) == Some(CENTRAL_SIGNATURE.as_slice()) {
            return Some(at);
        }
        from = at + 1;
    }
    None
}

/// A reusable read buffer over a part of the source.
struct Window<'a> {
    source: &'a Source,
    buffer: Vec<u8>,
    start: u64,
    reads: u64,
}

impl<'a> Window<'a> {
    const fn new(source: &'a Source) -> Self {
        Self {
            source,
            buffer: Vec::new(),
            start: 0,
            reads: 0,
        }
    }

    /// Returns the bytes held from `offset` on, at least `min` of them unless the source ends
    /// sooner. Reads a new [`CHUNK_LEN`] block only when fewer are held.
    fn get(&mut self, offset: u64, min: usize) -> io::Result<&[u8]> {
        let remaining =
            usize::try_from(self.source.len().saturating_sub(offset)).unwrap_or(usize::MAX);
        let need = min.min(remaining);
        let held = offset
            .checked_sub(self.start)
            .and_then(|skip| usize::try_from(skip).ok())
            .and_then(|skip| self.buffer.get(skip..));
        if held.is_none_or(|held| held.len() < need) {
            self.buffer.resize(CHUNK_LEN.max(need).min(remaining), 0);
            self.source.read_into(offset, &mut self.buffer)?;
            self.start = offset;
            self.reads += 1;
        }
        let skip = usize::try_from(offset - self.start).unwrap_or(0);
        Ok(self.buffer.get(skip..).unwrap_or_default())
    }
}
