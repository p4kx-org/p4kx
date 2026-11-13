// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read, Seek, SeekFrom};
use std::ops::Range;

use crate::file::{self, ChunkHeader, HEADER_LEN, TABLE_ENTRY_LEN};
use crate::{Chunk, ChunkError, ChunkTableRow, Container};

/// Failure to read an original chunk directory through a seekable input.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ChunkDirectoryError {
    /// Source bytes do not satisfy the chunk directory format.
    #[error(transparent)]
    Format(#[from] ChunkError),
    /// The original row declares an invalid payload extent.
    #[error("{source}")]
    InvalidRow {
        /// Original format failure.
        source: ChunkError,
        /// Original row bytes in the decoded Entry.
        range: Range<u64>,
    },
    /// The underlying seek, read or caller-supplied admission failed.
    #[error(transparent)]
    Input(#[from] io::Error),
}

/// Validated original chunk metadata without a copy of the source payload.
#[derive(Debug, Clone)]
pub struct ChunkDirectory {
    container: Container,
    chunks: Vec<Chunk>,
    table: Range<u64>,
}

impl ChunkDirectory {
    /// Reads only the original header and table within the supplied input length.
    ///
    /// The caller admits the validated number of rows before allocation and may interrupt at
    /// each parsing checkpoint. Its reader stays responsible for I/O accounting.
    ///
    /// # Errors
    ///
    /// Returns the original format or input error, including admission and checkpoint errors.
    pub fn read(
        reader: &mut (impl Read + Seek),
        length: u64,
        mut admit: impl FnMut(u64) -> io::Result<()>,
        mut checkpoint: impl FnMut() -> io::Result<()>,
    ) -> Result<Self, ChunkDirectoryError> {
        reader.seek(SeekFrom::Start(0))?;
        let mut header = [0; HEADER_LEN];
        let available =
            usize::try_from(length.min(HEADER_LEN as u64)).map_err(|_| ChunkError::Allocation)?;
        let header_bytes = header.get_mut(..available).ok_or(ChunkError::Allocation)?;
        reader.read_exact(header_bytes)?;
        let header = ChunkHeader::read(header_bytes, length)?;
        let count = u64::from(header.count);
        admit(count)?;
        let table_bytes =
            usize::try_from(count * TABLE_ENTRY_LEN as u64).map_err(|_| ChunkError::Allocation)?;
        let mut raw = Vec::new();
        raw.try_reserve_exact(table_bytes)
            .map_err(|_| ChunkError::Allocation)?;
        raw.resize(table_bytes, 0);
        reader.seek(SeekFrom::Start(header.table.start))?;
        reader.read_exact(&mut raw)?;
        let chunks = file::read_table(&header, &raw, || -> Result<(), ChunkDirectoryError> {
            Ok(checkpoint()?)
        })
        .map_err(|cause| invalid_row(cause, &header))?;
        Ok(Self {
            container: header.container,
            chunks,
            table: header.table,
        })
    }

    /// Returns the original container type.
    #[must_use]
    pub const fn container(&self) -> Container {
        self.container
    }

    /// Returns the complete chunk directory in original ordinal order.
    #[must_use]
    pub fn chunks(&self) -> &[Chunk] {
        &self.chunks
    }

    /// Returns the original table extent in the decoded Entry.
    #[must_use]
    pub fn table_range(&self) -> Range<u64> {
        self.table.clone()
    }

    /// Returns the exact original fields of one table row.
    #[must_use]
    pub fn table_row(&self, index: usize) -> Option<ChunkTableRow> {
        self.chunks.get(index)?;
        let start = self.table.start + index as u64 * TABLE_ENTRY_LEN as u64;
        Some(ChunkTableRow::at(start, self.container))
    }
}

fn invalid_row(cause: ChunkDirectoryError, header: &ChunkHeader) -> ChunkDirectoryError {
    match cause {
        ChunkDirectoryError::Format(
            source @ (ChunkError::ChunkOutOfBounds { index, .. }
            | ChunkError::ChunkInHeaderOrTable { index, .. }),
        ) => {
            let row_size = TABLE_ENTRY_LEN as u64;
            let start = header.table.start + index as u64 * row_size;
            ChunkDirectoryError::InvalidRow {
                source,
                range: start..start + row_size,
            }
        }
        other => other,
    }
}
