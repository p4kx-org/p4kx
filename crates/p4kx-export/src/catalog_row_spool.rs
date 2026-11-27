// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Seek as _, SeekFrom, Write};
use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::{Arc, Mutex};

use p4kx_extraction::CancelToken;
use serde::Serialize;
use serde_json::value::RawValue;
use thiserror::Error;
use zstd::stream::{read, write};

use crate::CatalogError;

#[path = "catalog_indexed_row_spool.rs"]
mod indexed;
#[path = "catalog_row_spool_serialization.rs"]
mod serialization;
#[path = "catalog_row_spool_suffix.rs"]
mod suffix;

pub(crate) use self::indexed::IndexedRowSpoolWriter;
pub(crate) use self::suffix::RowSpoolSuffixWriter;

const BUFFER_BYTES: usize = 64 * 1024;
const SPOOL_ZSTD_LEVEL: i32 = 1;

#[derive(Debug, Error)]
pub(crate) enum RowSpoolError {
    #[error("catalog row spool I/O failed")]
    Io(#[from] io::Error),
    #[error("catalog row spool suffix could not be rolled back: {rollback}")]
    SuffixRollback {
        #[source]
        primary: Box<CatalogError>,
        rollback: io::Error,
    },
    #[error("catalog row could not be encoded")]
    Encode(#[source] serde_json::Error),
    #[error("catalog row spool contained invalid JSON")]
    InvalidRow(#[source] serde_json::Error),
    #[error("catalog row spool failed verification: {0}")]
    Corrupt(&'static str),
    #[error("catalog row spool buffer could not be allocated")]
    Allocation(#[from] std::collections::TryReserveError),
    #[error("the catalog row spool was cancelled")]
    Cancelled,
    #[error("the catalog row spool is closed after a failed append")]
    Closed,
    #[error("catalog row spool row count or size cannot be represented")]
    Overflow,
}

impl From<RowSpoolError> for CatalogError {
    fn from(error: RowSpoolError) -> Self {
        match error {
            RowSpoolError::Cancelled => Self::Cancelled,
            other => Self::Write(io::Error::other(other)),
        }
    }
}

/// Streams ordered JSON rows into one private compressed file and indexes their decoded lengths.
pub(crate) struct RowSpoolWriter<W: Write = File> {
    encoder: Option<BufWriter<write::Encoder<'static, BufWriter<W>>>>,
    lengths: Vec<NonZeroUsize>,
}

impl RowSpoolWriter {
    pub(crate) fn new(scratch: &Path) -> Result<Self, RowSpoolError> {
        Self::with_output(tempfile::tempfile_in(scratch)?)
    }

    pub(crate) fn finish(self, cancel: Option<&CancelToken>) -> Result<RowSpool, RowSpoolError> {
        let (file, lengths) = self.finish_output(cancel)?;
        let bytes = file.metadata()?.len();
        Ok(RowSpool::new(file, bytes, RowIndex::Sequential(lengths)))
    }
}

impl<W: Write> RowSpoolWriter<W> {
    fn with_output(output: W) -> Result<Self, RowSpoolError> {
        let mut encoder = write::Encoder::new(
            BufWriter::with_capacity(BUFFER_BYTES, output),
            SPOOL_ZSTD_LEVEL,
        )?;
        encoder.include_checksum(true)?;
        Ok(Self {
            encoder: Some(BufWriter::with_capacity(BUFFER_BYTES, encoder)),
            lengths: Vec::new(),
        })
    }

    /// Appends one row in source order. An error closes the writer.
    pub(crate) fn append(
        &mut self,
        row: &(impl Serialize + ?Sized),
        cancel: Option<&CancelToken>,
    ) -> Result<(), RowSpoolError> {
        let mut encoder = self.encoder.take().ok_or(RowSpoolError::Closed)?;
        checkpoint(cancel)?;
        self.lengths.try_reserve(1)?;
        let length = write_row(&mut encoder, row, cancel)?;
        self.lengths.push(length);
        self.encoder = Some(encoder);
        Ok(())
    }

    /// Completes the compressed frame and releases the append buffer.
    fn finish_output(
        mut self,
        cancel: Option<&CancelToken>,
    ) -> Result<(W, Vec<NonZeroUsize>), RowSpoolError> {
        checkpoint(cancel)?;
        let buffered = self.encoder.take().ok_or(RowSpoolError::Closed)?;
        let encoder = buffered
            .into_inner()
            .map_err(io::IntoInnerError::into_error);
        checkpoint(cancel)?;
        let completed = encoder?.finish();
        checkpoint(cancel)?;
        let mut buffered = completed?;
        let flushed = buffered.flush();
        checkpoint(cancel)?;
        flushed?;
        let output = buffered
            .into_inner()
            .map_err(io::IntoInnerError::into_error)?;
        Ok((output, self.lengths))
    }
}

/// An immutable ordered row source replayed with a fresh decoder.
#[derive(Debug)]
pub(crate) struct RowSpool {
    backing: Arc<SpoolBacking>,
    offset: u64,
    bytes: u64,
    index: RowIndex,
}

#[derive(Debug)]
struct SpoolBacking {
    file: Mutex<File>,
    bytes: u64,
}

#[derive(Debug)]
enum RowIndex {
    Sequential(Vec<NonZeroUsize>),
    Indexed(Vec<indexed::Frame>),
}

impl RowSpool {
    fn new(file: File, bytes: u64, index: RowIndex) -> Self {
        Self {
            backing: Arc::new(SpoolBacking {
                file: Mutex::new(file),
                bytes,
            }),
            offset: 0,
            bytes,
            index,
        }
    }

    pub(crate) const fn rows(&self) -> usize {
        match &self.index {
            RowIndex::Sequential(lengths) => lengths.len(),
            RowIndex::Indexed(frames) => frames.len(),
        }
    }

    pub(crate) const fn stored_bytes(&self) -> u64 {
        self.bytes
    }

    fn replay<E>(
        &self,
        cancel: Option<&CancelToken>,
        mut visit: impl FnMut(&RawValue) -> Result<(), E>,
    ) -> Result<(), ReplayError<E>> {
        checkpoint(cancel)?;
        let mut file = self.backing.file.lock().map_err(|_| poisoned_lock())?;
        check_backing_length(&file, self.backing.bytes)?;
        if self
            .offset
            .checked_add(self.bytes)
            .is_none_or(|end| end > self.backing.bytes)
        {
            return Err(RowSpoolError::Corrupt("compressed range exceeds backing").into());
        }
        let lengths = match &self.index {
            RowIndex::Sequential(lengths) => lengths,
            RowIndex::Indexed(frames) => return indexed::replay(&mut file, frames, cancel, visit),
        };
        file.seek(SeekFrom::Start(self.offset))
            .map_err(RowSpoolError::Io)?;
        let input = CheckedIo::new((&mut *file).take(self.bytes), cancel);
        let decoder = read::Decoder::with_buffer(BufReader::with_capacity(BUFFER_BYTES, input))
            .map_err(RowSpoolError::Io)?
            .single_frame();
        let mut input = CheckedIo::new(decoder, cancel);
        let mut bytes = Vec::new();
        for &length in lengths {
            let row = read_row(&mut input, &mut bytes, length, cancel)?;
            visit(row).map_err(ReplayError::Output)?;
        }
        let mut tail = [0];
        let final_read = input.read(&mut tail);
        checkpoint(cancel)?;
        if final_read.map_err(RowSpoolError::Io)? != 0 {
            return Err(RowSpoolError::Corrupt("extra rows after the declared count").into());
        }
        let mut compressed = input.inner.finish();
        let final_read = compressed.read(&mut tail);
        drop(compressed);
        drop(file);
        checkpoint(cancel)?;
        if final_read.map_err(RowSpoolError::Io)? != 0 {
            return Err(RowSpoolError::Corrupt("extra bytes after the compressed frame").into());
        }
        Ok(())
    }
}

fn poisoned_lock() -> RowSpoolError {
    RowSpoolError::Io(io::Error::other("catalog row spool lock is poisoned"))
}

fn check_backing_length(file: &File, expected: u64) -> Result<(), RowSpoolError> {
    if file.metadata()?.len() != expected {
        return Err(RowSpoolError::Corrupt("compressed length changed"));
    }
    Ok(())
}

enum ReplayError<E> {
    Spool(RowSpoolError),
    Output(E),
}

impl<E> From<RowSpoolError> for ReplayError<E> {
    fn from(error: RowSpoolError) -> Self {
        Self::Spool(error)
    }
}

fn read_row<'a>(
    input: &mut impl Read,
    bytes: &'a mut Vec<u8>,
    length: NonZeroUsize,
    cancel: Option<&CancelToken>,
) -> Result<&'a RawValue, RowSpoolError> {
    checkpoint(cancel)?;
    let size = length.get();
    bytes.clear();
    bytes.try_reserve(size)?;
    bytes.resize(size, 0);
    checked_read(input, bytes, cancel)?;
    let parsed = serde_json::from_slice(bytes);
    checkpoint(cancel)?;
    parsed.map_err(RowSpoolError::InvalidRow)
}

fn checked_read(
    input: &mut impl Read,
    bytes: &mut [u8],
    cancel: Option<&CancelToken>,
) -> Result<(), RowSpoolError> {
    let read = input.read_exact(bytes);
    checkpoint(cancel)?;
    match read {
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
            Err(RowSpoolError::Corrupt("a prepared row ended early"))
        }
        read => read.map_err(RowSpoolError::Io),
    }
}

fn write_row(
    writer: &mut impl Write,
    row: &(impl Serialize + ?Sized),
    cancel: Option<&CancelToken>,
) -> Result<NonZeroUsize, RowSpoolError> {
    let mut output = RowWriter::new(CheckedIo::new(writer, cancel));
    let serialized = serde_json::to_writer(&mut output, row);
    checkpoint(cancel)?;
    serialized.map_err(|error| {
        if error.is_io() {
            RowSpoolError::Io(error.into())
        } else {
            RowSpoolError::Encode(error)
        }
    })?;
    NonZeroUsize::new(output.length).ok_or(RowSpoolError::Corrupt("encoded row is empty"))
}

struct RowWriter<W> {
    inner: W,
    length: usize,
}

impl<W> RowWriter<W> {
    const fn new(inner: W) -> Self {
        Self { inner, length: 0 }
    }
}

impl<W: Write> Write for RowWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(bytes)?;
        self.length = self
            .length
            .checked_add(written)
            .ok_or_else(|| io::Error::other(RowSpoolError::Overflow))?;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

struct CheckedIo<'a, T> {
    inner: T,
    cancel: Option<&'a CancelToken>,
}

impl<'a, T> CheckedIo<'a, T> {
    const fn new(inner: T, cancel: Option<&'a CancelToken>) -> Self {
        Self { inner, cancel }
    }
}

impl<T: Read> Read for CheckedIo<'_, T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        checkpoint(self.cancel).map_err(io::Error::other)?;
        let count = buf.len().min(BUFFER_BYTES);
        let Some(chunk) = buf.get_mut(..count) else {
            return Err(io::Error::other("invalid catalog row spool read buffer"));
        };
        self.inner.read(chunk)
    }
}

impl<T: Write> Write for CheckedIo<'_, T> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        checkpoint(self.cancel).map_err(io::Error::other)?;
        let Some(chunk) = bytes.get(..bytes.len().min(BUFFER_BYTES)) else {
            return Err(io::Error::other("invalid catalog row spool write buffer"));
        };
        self.inner.write(chunk)
    }

    fn flush(&mut self) -> io::Result<()> {
        checkpoint(self.cancel).map_err(io::Error::other)?;
        self.inner.flush()
    }
}

pub(crate) fn checkpoint(cancel: Option<&CancelToken>) -> Result<(), RowSpoolError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(RowSpoolError::Cancelled);
    }
    Ok(())
}

#[cfg(test)]
#[path = "catalog_row_spool_tests.rs"]
mod tests;
