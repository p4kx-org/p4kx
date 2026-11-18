// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use crate::central_directory::{
    self, CENTRAL_HEADER_LEN, CENTRAL_HEADER_SIGNATURE, MAX_HEADER_LEN,
};
use crate::format_error::FormatError;
use crate::issue::IssueLog;
use crate::local_header_match::{self, LOCAL_HEADER_LEN};
use crate::source::Source;

/// Shared original bytes sufficient to contain any directory record overlapping the ZIP tail.
pub(crate) struct EndFrameProof<'source> {
    source: &'source Source,
    bytes: Cow<'source, [u8]>,
    start: u64,
    loaded: bool,
    remote: Option<RemoteWindow>,
}

struct RemoteWindow {
    point: u64,
    start: u64,
    bytes: Vec<u8>,
}

impl<'source> EndFrameProof<'source> {
    pub(crate) const fn new(source: &'source Source, tail: &'source [u8], start: u64) -> Self {
        Self {
            source,
            bytes: Cow::Borrowed(tail),
            start,
            loaded: false,
            remote: None,
        }
    }

    /// Refuses physically conflicting first and classic end frames from the original metadata.
    pub(crate) fn ensure_unowned(&mut self, first: u64, classic: u64) -> Result<(), FormatError> {
        self.load()?;
        check(self.source, &self.bytes, self.start, [first, classic])?;
        if first.saturating_sub(MAX_HEADER_LEN as u64) < self.start {
            let source = self.source;
            let remote = self.remote_window(first)?;
            check(source, &remote.bytes, remote.start, [first, first])?;
        }
        Ok(())
    }

    fn remote_window(&mut self, point: u64) -> Result<&RemoteWindow, FormatError> {
        if self
            .remote
            .as_ref()
            .is_none_or(|window| window.point != point)
        {
            self.remote = None;
            let start = point.saturating_sub(MAX_HEADER_LEN as u64);
            let end = self
                .source
                .len()
                .min(point.saturating_add(MAX_HEADER_LEN as u64));
            let bytes = self.remote_bytes(start, end)?;
            self.remote = Some(RemoteWindow {
                point,
                start,
                bytes,
            });
        }
        self.remote.as_ref().ok_or_else(|| {
            FormatError::corrupt(point, "original remote frame window is unavailable")
        })
    }

    fn remote_bytes(&self, start: u64, end: u64) -> Result<Vec<u8>, FormatError> {
        let capacity = usize::try_from(end - start).map_err(|_| {
            FormatError::corrupt(start, "remote frame window does not fit into memory")
        })?;
        let prefix = usize::try_from(end.min(self.start) - start).map_err(|_| {
            FormatError::corrupt(start, "remote frame prefix does not fit into memory")
        })?;
        let mut bytes = Vec::with_capacity(capacity);
        bytes.resize(prefix, 0);
        self.source.read_into(start, &mut bytes)?;
        if end > self.start {
            let length = usize::try_from(end - self.start).map_err(|_| {
                FormatError::corrupt(start, "remote frame overlap does not fit into memory")
            })?;
            let overlap = self.bytes.get(..length).ok_or_else(|| {
                FormatError::corrupt(
                    start,
                    "remote frame overlap is outside the held source bytes",
                )
            })?;
            bytes.extend_from_slice(overlap);
        }
        Ok(bytes)
    }

    fn load(&mut self) -> Result<(), FormatError> {
        if self.loaded {
            return Ok(());
        }
        let start = self.start.saturating_sub(MAX_HEADER_LEN as u64);
        let prefix = usize::try_from(self.start - start).map_err(|_| {
            FormatError::corrupt(start, "directory ownership window does not fit into memory")
        })?;
        if prefix > 0 {
            let mut bytes = Vec::with_capacity(prefix + self.bytes.len());
            bytes.resize(prefix, 0);
            self.source.read_into(start, &mut bytes)?;
            bytes.extend_from_slice(&self.bytes);
            self.bytes = Cow::Owned(bytes);
            self.start = start;
        }
        self.loaded = true;
        Ok(())
    }
}

fn check(
    source: &Source,
    tail: &[u8],
    tail_start: u64,
    end_offsets: [u64; 2],
) -> Result<(), FormatError> {
    let signature = CENTRAL_HEADER_SIGNATURE.to_le_bytes();
    for (index, _) in tail
        .windows(signature.len())
        .enumerate()
        .filter(|(_, bytes)| *bytes == signature.as_slice())
    {
        let bytes = tail.get(index..).unwrap_or_default();
        let offset = tail_start + index as u64;
        let Some(record) = complete_record(bytes, offset)? else {
            continue;
        };
        let mut log = IssueLog::default();
        let header =
            match central_directory::parse_header(record, offset, 0, &mut log, source.len()) {
                Ok(value) => value,
                Err(FormatError::Corrupt { .. } | FormatError::Truncated(_)) => continue,
                Err(error) => return Err(error),
            };
        let local_offset = header.entry.data_offset;
        let longest_header = LOCAL_HEADER_LEN as u64 + 2 * u64::from(u16::MAX);
        let latest_end = local_offset
            .saturating_add(longest_header)
            .saturating_add(header.entry.compressed_size);
        let owns_record = end_offsets
            .iter()
            .any(|&point| offset <= point && point < offset + header.len as u64);
        let possible_entry = end_offsets
            .iter()
            .any(|&point| local_offset <= point && point < latest_end);
        if !owns_record && !possible_entry {
            continue;
        }
        if local_offset
            .checked_add((LOCAL_HEADER_LEN + header.name.len()) as u64)
            .is_none_or(|local_end| local_end > offset)
        {
            continue;
        }
        let Some(local) = local_header_match::read(source, local_offset, header.name)? else {
            continue;
        };
        let Some(data_end) = local.data_start.checked_add(header.entry.compressed_size) else {
            continue;
        };
        if data_end > offset || data_end > source.len() {
            continue;
        }
        if let Some(point) = end_offsets.iter().copied().find(|&point| {
            (offset <= point && point < offset + header.len as u64)
                || (local_offset <= point && point < data_end)
        }) {
            return Err(FormatError::corrupt(
                point,
                "ZIP end frame is inside an original bound directory or Entry frame",
            ));
        }
    }
    Ok(())
}

fn complete_record(bytes: &[u8], offset: u64) -> Result<Option<&[u8]>, FormatError> {
    let Some(fixed) = bytes.get(..CENTRAL_HEADER_LEN) else {
        return Ok(None);
    };
    let lengths = match central_directory::record_lengths(fixed, offset) {
        Ok(value) => value,
        Err(FormatError::Corrupt { .. } | FormatError::Truncated(_)) => return Ok(None),
        Err(error) => return Err(error),
    };
    let total = CENTRAL_HEADER_LEN + lengths.name + lengths.extra + lengths.comment;
    Ok(bytes.get(..total))
}
