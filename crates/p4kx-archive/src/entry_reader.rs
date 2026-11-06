// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;
use std::io::{self, Read};

use p4kx_foundation::{ARCHIVE_IV, ArchiveKey, ByteReader};

use crate::decode::{ExactSize, Overlong};
use crate::decrypt::{AES_BLOCK_LEN, Decrypting};
use crate::entry::DataStart;
use crate::entry_decoder::{DataReader, EntryDecoder};
use crate::format_error::FormatError;
use crate::source::Source;
use crate::stored_hash::StoredCheck;
use crate::verify::Verifier;
use crate::{Compression, EffectiveEncoding, Entry};

const LOCAL_HEADER_SIGNATURE: u32 = 0x0403_4B50;
const LOCAL_HEADER_CIG_SIGNATURE: u32 = 0x1403_4B50;
const LOCAL_HEADER_LEN: usize = 30;
const LOCAL_HEADER_FIXED_FIELDS_LEN: usize = 22;
const DECODER_BUFFER_LEN: usize = 256 * 1024;

/// Streams the decrypted and decompressed data of one Entry from its Archive.
///
/// Decoding errors are reported as [`io::Error`]s of kind [`io::ErrorKind::InvalidData`] that
/// carry a [`DataError`](crate::DataError).
pub struct EntryReader<'a> {
    inner: ExactSize<EntryDecoder<'a>>,
    encrypted: bool,
    verifier: Option<Verifier>,
}

impl EntryReader<'_> {
    /// Returns the selected decoder without reading or consuming any bytes.
    /// CIG method 100 stays undetermined until its decrypted prefix has been read.
    #[must_use]
    pub fn effective_encoding(&self) -> EffectiveEncoding {
        self.inner.inner().effective_encoding()
    }

    /// Checks the decoded data against the checksum of `entry` as it streams by, like
    /// `open_verified` but without the SHA-256 of the stored bytes. Only for a reader that starts
    /// at the beginning of the Entry's data.
    #[must_use]
    pub(crate) fn crc_checked(mut self, entry: &Entry) -> Self {
        self.verifier = Some(Verifier::new(entry, None));
        self
    }

    /// Makes data that goes on after the Entry's uncompressed size an error, a
    /// [`DataError::TooLong`](crate::DataError::TooLong), instead of cutting it; the zero padding
    /// of an encrypted Entry below one AES block is still allowed.
    #[must_use]
    pub const fn reject_overlong(mut self) -> Self {
        self.inner.set_overlong(Overlong::Reject {
            padding: self.encrypted,
        });
        self
    }
}

impl fmt::Debug for EntryReader<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EntryReader").finish_non_exhaustive()
    }
}

impl Read for EntryReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let count = self.inner.read(buffer)?;
        if let Some(verifier) = &mut self.verifier {
            let data = buffer.get(..count).unwrap_or_default();
            let ended = verifier.feed(data);
            if !matches!(ended, Ok(false)) {
                self.verifier = None;
            }
            ended?;
        }
        Ok(count)
    }
}

/// Reads the stored bytes of an Entry by positional reads.
struct RawReader<'a> {
    source: &'a Source,
    offset: u64,
    remaining: u64,
}

impl Read for RawReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let count = usize::try_from(self.remaining)
            .unwrap_or(usize::MAX)
            .min(buffer.len());
        let Some(chunk) = buffer.get_mut(..count).filter(|chunk| !chunk.is_empty()) else {
            return Ok(0);
        };
        self.source.read_into(self.offset, chunk)?;
        self.offset += count as u64;
        self.remaining -= count as u64;
        Ok(count)
    }
}

/// Returns where the stored data of `entry` begins after checking its local header, if its layout
/// has one, and that the data lies inside `source`.
pub(crate) fn data_offset(source: &Source, entry: &Entry) -> Result<u64, FormatError> {
    let (offset, data_offset) = match entry.data() {
        DataStart::LocalHeader(offset) => (offset, read_local_header(source, offset)?),
        DataStart::Direct(offset) => (offset, offset),
    };
    if !fits(source, data_offset, entry.compressed_size) {
        return Err(FormatError::corrupt(
            offset,
            "Entry data lies outside the file",
        ));
    }
    Ok(data_offset)
}

/// Opens the data of `entry` after checking its local header, if its layout has one, decrypting
/// with `key` when the Entry is encrypted.
pub(crate) fn open<'a>(
    source: &'a Source,
    entry: &Entry,
    key: Option<&ArchiveKey>,
) -> Result<EntryReader<'a>, FormatError> {
    open_at(source, entry, 0, key).map(|(reader, _)| reader)
}

/// Opens the data of `entry` from decoded byte `offset`, at most its size; returns the reader
/// and how many decoded bytes it still has to skip to reach `offset`.
///
/// Stored data starts at `offset` directly: unencrypted at that byte, encrypted at the AES block
/// holding it, chained from the previous ciphertext block. Compressed data starts at the
/// beginning and the whole `offset` is left to skip.
pub(crate) fn open_at<'a>(
    source: &'a Source,
    entry: &Entry,
    offset: u64,
    key: Option<&ArchiveKey>,
) -> Result<(EntryReader<'a>, u64), FormatError> {
    open_with(source, entry, offset, key, false)
}

/// Opens the data of `entry` from its start like `open`, verified: the data is checked against
/// the checksums of `entry` as it streams by, and a mismatch is a
/// [`DataError::ChecksumMismatch`](crate::DataError::ChecksumMismatch) from the read that reaches
/// the end of the data, which then yields none of its bytes. The checksum of the decoded data and,
/// if the Entry states one, the SHA-256 of its stored bytes are checked.
pub(crate) fn open_verified<'a>(
    source: &'a Source,
    entry: &Entry,
    key: Option<&ArchiveKey>,
) -> Result<EntryReader<'a>, FormatError> {
    open_with(source, entry, 0, key, true).map(|(reader, _)| reader)
}

fn open_with<'a>(
    source: &'a Source,
    entry: &Entry,
    offset: u64,
    key: Option<&ArchiveKey>,
    verify: bool,
) -> Result<(EntryReader<'a>, u64), FormatError> {
    let key = match (entry.encrypted, key) {
        (true, Some(key)) => Some(key),
        (true, None) => {
            return Err(FormatError::KeyRequired {
                entry: entry.path.clone(),
            });
        }
        (false, _) => None,
    };
    let data_offset = data_offset(source, entry)?;
    if entry.encrypted && !entry.compressed_size.is_multiple_of(AES_BLOCK_LEN as u64) {
        return Err(FormatError::corrupt(
            entry.data_offset,
            "encrypted Entry data is not a whole number of AES blocks",
        ));
    }
    let start = match entry.compression {
        Compression::Store if entry.encrypted => {
            offset.min(entry.compressed_size) / AES_BLOCK_LEN as u64 * AES_BLOCK_LEN as u64
        }
        Compression::Store => offset.min(entry.compressed_size),
        _ => 0,
    };
    let iv = if entry.encrypted && start > 0 {
        let mut iv = [0; AES_BLOCK_LEN];
        source.read_into(data_offset + start - AES_BLOCK_LEN as u64, &mut iv)?;
        iv
    } else {
        ARCHIVE_IV
    };
    let stored_len = entry.compressed_size - start;
    let raw = RawReader {
        source,
        offset: data_offset + start,
        remaining: stored_len,
    };
    let check = if verify && start == 0 {
        stored_check(source, entry, data_offset)
    } else {
        None
    };
    let raw: DataReader<'a> = match &check {
        Some(check) => Box::new(check.tap(raw)),
        None => Box::new(raw),
    };
    let stored: DataReader<'a> = match key {
        Some(key) => Box::new(Decrypting::new(raw, buffer_len(stored_len), iv, key)),
        None => raw,
    };
    let decoded = EntryDecoder::new(stored, entry, buffer_len(entry.compressed_size))?;
    let reader = EntryReader {
        inner: ExactSize::new(decoded, entry.uncompressed_size.saturating_sub(start)),
        encrypted: entry.encrypted,
        verifier: verify.then(|| Verifier::new(entry, check)),
    };
    Ok((reader, offset.saturating_sub(start)))
}

/// Returns the check of the SHA-256 `entry` states for its stored bytes at `data_offset`.
fn stored_check(source: &Source, entry: &Entry, data_offset: u64) -> Option<Box<StoredCheck>> {
    let digest = entry.sha256.filter(|_| entry.sha256_checked)?;
    Some(Box::new(StoredCheck::new(
        source,
        data_offset,
        entry.compressed_size,
        digest,
    )))
}

fn read_local_header(source: &Source, offset: u64) -> Result<u64, FormatError> {
    if !fits(source, offset, LOCAL_HEADER_LEN as u64) {
        return Err(FormatError::corrupt(
            offset,
            "local header lies outside the file",
        ));
    }
    let header = source.read_at(offset, LOCAL_HEADER_LEN)?;
    let mut reader = ByteReader::with_base(&header, offset);
    let signature = reader.read_u32()?;
    if signature != LOCAL_HEADER_SIGNATURE && signature != LOCAL_HEADER_CIG_SIGNATURE {
        return Err(FormatError::corrupt(
            offset,
            "local header signature missing",
        ));
    }
    reader.skip(LOCAL_HEADER_FIXED_FIELDS_LEN)?;
    let name_len = u64::from(reader.read_u16()?);
    let extra_len = u64::from(reader.read_u16()?);
    Ok(offset + LOCAL_HEADER_LEN as u64 + name_len + extra_len)
}

fn buffer_len(compressed_size: u64) -> usize {
    usize::try_from(compressed_size)
        .unwrap_or(usize::MAX)
        .clamp(1, DECODER_BUFFER_LEN)
}

fn fits(source: &Source, offset: u64, len: u64) -> bool {
    offset
        .checked_add(len)
        .is_some_and(|end| end <= source.len())
}
