// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Seek as _, Write};
use std::path::Path;
use std::sync::Mutex;

use p4kx_extraction::CancelToken;
use serde::ser::{Error as _, SerializeSeq as _};
use serde::{Serialize, Serializer};
use serde_json::value::RawValue;
use thiserror::Error;
use zstd::stream::{read, write};

use crate::CatalogError;

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

/// Appends length-framed JSON rows to one private compressed temporary file.
pub(crate) struct RowSpoolWriter {
    encoder: Option<write::Encoder<'static, BufWriter<File>>>,
    row: RowBuffer,
    rows: usize,
    largest_row: usize,
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
            encoder: Some(encoder),
            row: RowBuffer::default(),
            rows: 0,
            largest_row: 0,
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
        let rows = self.rows.checked_add(1).ok_or(RowSpoolError::Overflow)?;
        self.row.bytes.clear();
        let serialized = serde_json::to_writer(CheckedIo::new(&mut self.row, cancel), row);
        checkpoint(cancel)?;
        serialized.map_err(RowSpoolError::Encode)?;
        let length = u64::try_from(self.row.bytes.len()).map_err(|_| RowSpoolError::Overflow)?;
        let mut output = CheckedIo::new(&mut encoder, cancel);
        let written = output
            .write_all(&length.to_le_bytes())
            .and_then(|()| output.write_all(&self.row.bytes));
        checkpoint(cancel)?;
        written?;
        self.rows = rows;
        self.largest_row = self.largest_row.max(self.row.bytes.len());
        self.encoder = Some(encoder);
        Ok(())
    }

    /// Completes the compressed frame and releases the append buffer.
    pub(crate) fn finish(
        mut self,
        cancel: Option<&CancelToken>,
    ) -> Result<RowSpool, RowSpoolError> {
        checkpoint(cancel)?;
        let encoder = self.encoder.take().ok_or(RowSpoolError::Closed)?;
        let completed = encoder.finish();
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
            rows: self.rows,
            largest_row: self.largest_row,
            bytes,
        })
    }
}

/// An immutable ordered row source replayed with a fresh decoder.
#[derive(Debug)]
pub(crate) struct RowSpool {
    file: Mutex<File>,
    rows: usize,
    largest_row: usize,
    bytes: u64,
}

impl RowSpool {
    pub(crate) const fn rows(&self) -> usize {
        self.rows
    }

    #[cfg(test)]
    const fn stored_bytes(&self) -> u64 {
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
        file.rewind().map_err(RowSpoolError::Io)?;
        let input = CheckedIo::new(&mut *file, cancel);
        let decoder = read::Decoder::with_buffer(BufReader::with_capacity(BUFFER_BYTES, input))
            .map_err(RowSpoolError::Io)?
            .single_frame();
        let mut input = CheckedIo::new(decoder, cancel);
        let mut bytes = Vec::new();
        for _ in 0..self.rows {
            let row = read_row(&mut input, &mut bytes, self.largest_row, cancel)?;
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
}

impl Serialize for SpoolRows<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        *self.failure.borrow_mut() = None;
        let mut sequence = serializer.serialize_seq(Some(self.spool.rows()))?;
        match self
            .spool
            .replay(self.cancel, |row| sequence.serialize_element(row))
        {
            Ok(()) => sequence.end(),
            Err(ReplayError::Output(error)) => Err(error),
            Err(ReplayError::Spool(error)) => {
                let reported = S::Error::custom(&error);
                *self.failure.borrow_mut() = Some(error);
                Err(reported)
            }
        }
    }
}

fn read_row<'a>(
    input: &mut impl Read,
    bytes: &'a mut Vec<u8>,
    largest_row: usize,
    cancel: Option<&CancelToken>,
) -> Result<&'a RawValue, RowSpoolError> {
    let mut length = [0; size_of::<u64>()];
    checked_read(input, &mut length, cancel)?;
    let size = usize::try_from(u64::from_le_bytes(length)).map_err(|_| RowSpoolError::Overflow)?;
    if size == 0 || size > largest_row {
        return Err(RowSpoolError::Corrupt(
            "row length differs from the prepared bounds",
        ));
    }
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

#[derive(Default)]
struct RowBuffer {
    bytes: Vec<u8>,
}

impl Write for RowBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes
            .try_reserve(bytes.len())
            .map_err(io::Error::other)?;
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
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

fn checkpoint(cancel: Option<&CancelToken>) -> Result<(), RowSpoolError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(RowSpoolError::Cancelled);
    }
    Ok(())
}

#[cfg(test)]
#[path = "catalog_row_spool_tests.rs"]
mod tests;
