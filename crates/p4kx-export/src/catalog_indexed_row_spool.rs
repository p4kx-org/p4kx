// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{BufWriter, Seek as _, SeekFrom, Write as _};
use std::path::Path;
use std::sync::Mutex;

use p4kx_extraction::CancelToken;
use serde::Serialize;
use serde_json::value::RawValue;
use zstd::bulk::{Compressor, Decompressor};
use zstd::zstd_safe::{CParameter, compress_bound, find_frame_compressed_size};

use super::{
    BUFFER_BYTES, CheckedIo, ReplayError, RowBuffer, RowSpool, RowSpoolError, SPOOL_ZSTD_LEVEL,
    checked_read, checkpoint,
};

#[derive(Debug)]
pub(super) struct Frame {
    offset: u64,
    packed: usize,
    original: usize,
}

/// Writes rows once to independently verified frames and replays them by key.
pub(crate) struct IndexedRowSpoolWriter<Key> {
    writer: Option<BufWriter<File>>,
    compressor: Compressor<'static>,
    row: RowBuffer,
    packed: Vec<u8>,
    keys: Vec<(Key, Frame)>,
    bytes: u64,
}

impl<Key: Ord> IndexedRowSpoolWriter<Key> {
    pub(crate) fn new(directory: &Path) -> Result<Self, RowSpoolError> {
        let mut compressor = Compressor::new(SPOOL_ZSTD_LEVEL)?;
        compressor.set_parameter(CParameter::ChecksumFlag(true))?;
        Ok(Self {
            writer: Some(BufWriter::with_capacity(
                BUFFER_BYTES,
                tempfile::tempfile_in(directory)?,
            )),
            compressor,
            row: RowBuffer::default(),
            packed: Vec::new(),
            keys: Vec::new(),
            bytes: 0,
        })
    }

    pub(crate) fn append(
        &mut self,
        key: Key,
        row: &impl Serialize,
        cancel: Option<&CancelToken>,
    ) -> Result<(), RowSpoolError> {
        let mut writer = self.writer.take().ok_or(RowSpoolError::Closed)?;
        checkpoint(cancel)?;
        let bytes = self.row.encode(row, cancel)?;
        self.packed.clear();
        self.packed.try_reserve(compress_bound(bytes.len()))?;
        let packed = self.compressor.compress_to_buffer(bytes, &mut self.packed);
        checkpoint(cancel)?;
        let packed = packed?;
        self.keys.try_reserve(1)?;
        let frame = Frame {
            offset: self.bytes,
            packed,
            original: bytes.len(),
        };
        self.bytes = self
            .bytes
            .checked_add(u64::try_from(packed).map_err(|_| RowSpoolError::Overflow)?)
            .ok_or(RowSpoolError::Overflow)?;
        let written = CheckedIo::new(&mut writer, cancel).write_all(&self.packed);
        checkpoint(cancel)?;
        written?;
        self.keys.push((key, frame));
        self.writer = Some(writer);
        Ok(())
    }

    pub(crate) fn finish(
        mut self,
        cancel: Option<&CancelToken>,
    ) -> Result<RowSpool, RowSpoolError> {
        checkpoint(cancel)?;
        let mut writer = self.writer.take().ok_or(RowSpoolError::Closed)?;
        let flushed = writer.flush();
        checkpoint(cancel)?;
        flushed?;
        let file = writer
            .into_inner()
            .map_err(std::io::IntoInnerError::into_error)?;
        if file.metadata()?.len() != self.bytes {
            return Err(RowSpoolError::Corrupt("indexed compressed length changed"));
        }
        self.keys
            .sort_unstable_by(|(left_key, left), (right_key, right)| {
                left_key.cmp(right_key).then(left.offset.cmp(&right.offset))
            });
        checkpoint(cancel)?;
        let mut frames = Vec::new();
        frames.try_reserve_exact(self.keys.len())?;
        for (_, frame) in self.keys {
            checkpoint(cancel)?;
            frames.push(frame);
        }
        Ok(RowSpool {
            file: Mutex::new(file),
            rows: frames.len(),
            largest_row: 0,
            bytes: self.bytes,
            frames: Some(frames),
        })
    }
}

pub(super) fn replay<E>(
    file: &mut File,
    frames: &[Frame],
    cancel: Option<&CancelToken>,
    mut visit: impl FnMut(&RawValue) -> Result<(), E>,
) -> Result<(), ReplayError<E>> {
    let mut decoder = Decompressor::new().map_err(RowSpoolError::Io)?;
    let mut packed = Vec::new();
    let mut row = Vec::new();
    for frame in frames {
        checkpoint(cancel)?;
        file.seek(SeekFrom::Start(frame.offset))
            .map_err(RowSpoolError::Io)?;
        packed.clear();
        packed
            .try_reserve_exact(frame.packed)
            .map_err(RowSpoolError::Allocation)?;
        packed.resize(frame.packed, 0);
        checked_read(&mut CheckedIo::new(&mut *file, cancel), &mut packed, cancel)?;
        if find_frame_compressed_size(&packed) != Ok(frame.packed) {
            return Err(RowSpoolError::Corrupt("incorrect indexed frame length").into());
        }
        row.clear();
        row.try_reserve_exact(frame.original)
            .map_err(RowSpoolError::Allocation)?;
        row.resize(frame.original, 0);
        let read_size = decoder.decompress_to_buffer(&packed, row.as_mut_slice());
        checkpoint(cancel)?;
        if read_size.map_err(RowSpoolError::Io)? != frame.original {
            return Err(RowSpoolError::Corrupt("incorrect indexed row length").into());
        }
        let parsed = serde_json::from_slice(&row);
        checkpoint(cancel)?;
        visit(parsed.map_err(RowSpoolError::InvalidRow)?).map_err(ReplayError::Output)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "catalog_indexed_row_spool_tests.rs"]
mod tests;
