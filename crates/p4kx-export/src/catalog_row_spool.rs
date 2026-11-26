// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Seek as _, Write};
use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::Mutex;

use p4kx_extraction::CancelToken;
use serde::ser::SerializeSeq as _;
use serde::{Serialize, Serializer};
use serde_json::value::RawValue;
use thiserror::Error;
use zstd::stream::{read, write};

use crate::CatalogError;

#[path = "catalog_indexed_row_spool.rs"]
mod indexed;

pub(crate) use self::indexed::IndexedRowSpoolWriter;

const BUFFER_BYTES: usize = 64 * 1024;
const SPOOL_ZSTD_LEVEL: i32 = 1;

#[derive(Debug, Error)]
pub(crate) enum RowSpoolError {
    #[error("catalog row spool I/O failed")]
    Io(#[from] io::Error),
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
pub(crate) struct RowSpoolWriter {
    encoder: Option<BufWriter<write::Encoder<'static, BufWriter<File>>>>,
    lengths: Vec<NonZeroUsize>,
}

impl RowSpoolWriter {
    pub(crate) fn new(scratch: &Path) -> Result<Self, RowSpoolError> {
        let file = tempfile::tempfile_in(scratch)?;
        let mut encoder = write::Encoder::new(
            BufWriter::with_capacity(BUFFER_BYTES, file),
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
    pub(crate) fn finish(
        mut self,
        cancel: Option<&CancelToken>,
    ) -> Result<RowSpool, RowSpoolError> {
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
        let file = buffered
            .into_inner()
            .map_err(io::IntoInnerError::into_error)?;
        let bytes = file.metadata()?.len();
        Ok(RowSpool {
            file: Mutex::new(file),
            bytes,
            index: RowIndex::Sequential(self.lengths),
        })
    }
}

/// An immutable ordered row source replayed with a fresh decoder.
#[derive(Debug)]
pub(crate) struct RowSpool {
    file: Mutex<File>,
    bytes: u64,
    index: RowIndex,
}

#[derive(Debug)]
enum RowIndex {
    Sequential(Vec<NonZeroUsize>),
    Indexed(Vec<indexed::Frame>),
}

impl RowSpool {
    pub(crate) const fn rows(&self) -> usize {
        match &self.index {
            RowIndex::Sequential(lengths) => lengths.len(),
            RowIndex::Indexed(frames) => frames.len(),
        }
    }

    pub(crate) const fn stored_bytes(&self) -> u64 {
        self.bytes
    }

    pub(crate) const fn serialize_rows<'a>(
        &'a self,
        cancel: Option<&'a CancelToken>,
    ) -> SpoolRows<'a> {
        SpoolRows {
            spool: self,
            cancel,
            failure: RefCell::new(None),
        }
    }

    fn replay<E>(
        &self,
        cancel: Option<&CancelToken>,
        mut visit: impl FnMut(&RawValue) -> Result<(), E>,
    ) -> Result<(), ReplayError<E>> {
        checkpoint(cancel)?;
        let mut file = self.file.lock().map_err(|_| {
            RowSpoolError::Io(io::Error::other("catalog row spool lock is poisoned"))
        })?;
        if file.metadata().map_err(RowSpoolError::Io)?.len() != self.bytes {
            return Err(RowSpoolError::Corrupt("compressed length changed").into());
        }
        let lengths = match &self.index {
            RowIndex::Sequential(lengths) => lengths,
            RowIndex::Indexed(frames) => return indexed::replay(&mut file, frames, cancel, visit),
        };
        file.rewind().map_err(RowSpoolError::Io)?;
        let input = CheckedIo::new(&mut *file, cancel);
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

enum ReplayError<E> {
    Spool(RowSpoolError),
    Output(E),
}

impl<E> From<RowSpoolError> for ReplayError<E> {
    fn from(error: RowSpoolError) -> Self {
        Self::Spool(error)
    }
}

/// Serializes the same ordered row sequence on each invocation.
#[derive(Debug)]
pub(crate) struct SpoolRows<'a> {
    spool: &'a RowSpool,
    cancel: Option<&'a CancelToken>,
    failure: RefCell<Option<RowSpoolError>>,
}

impl SpoolRows<'_> {
    /// Recovers the typed input error after a failed serialization.
    pub(crate) fn take_failure(&self) -> Option<RowSpoolError> {
        self.failure.take()
    }

    /// Streams verified rows into an existing JSON sequence.
    pub(crate) fn visit<E: serde::ser::Error>(
        &self,
        visit: impl FnMut(&RawValue) -> Result<(), E>,
    ) -> Result<(), E> {
        *self.failure.borrow_mut() = None;
        match self.spool.replay(self.cancel, visit) {
            Ok(()) => Ok(()),
            Err(ReplayError::Output(error)) => Err(error),
            Err(ReplayError::Spool(error)) => {
                let reported = E::custom(&error);
                *self.failure.borrow_mut() = Some(error);
                Err(reported)
            }
        }
    }
}

impl Serialize for SpoolRows<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        *self.failure.borrow_mut() = None;
        let mut sequence = serializer.serialize_seq(Some(self.spool.rows()))?;
        self.visit(|row| sequence.serialize_element(row))?;
        sequence.end()
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
